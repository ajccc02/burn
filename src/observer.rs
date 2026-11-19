use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::{
    budget::BudgetSnapshot,
    journal::{AgentState, Task},
};

#[derive(Clone, Debug)]
pub struct ContextLimits {
    pub max_context_tokens: usize,
    pub max_log_entries: usize,
    pub max_errors: usize,
    pub max_journal_entries: usize,
    pub max_source_bytes: usize,
    pub max_pending_tasks: usize,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            max_context_tokens: 24_000,
            max_log_entries: 24,
            max_errors: 16,
            max_journal_entries: 16,
            max_source_bytes: 8_192,
            max_pending_tasks: 32,
        }
    }
}

fn bytes(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |wire| wire.len())
}

fn append(value: &mut Value, path: &str, item: Value, limit: usize) -> bool {
    let Some(items) = value.pointer_mut(path).and_then(Value::as_array_mut) else {
        return false;
    };
    items.push(item);
    if bytes(value) <= limit {
        return true;
    }
    if let Some(items) = value.pointer_mut(path).and_then(Value::as_array_mut) {
        items.pop();
    }
    false
}

pub(crate) fn array<'a>(value: &'a Value, path: &str) -> &'a [Value] {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn recent<'a>(value: &'a Value, path: &str) -> impl DoubleEndedIterator<Item = &'a Value> {
    array(value, path).iter().rev()
}

pub(crate) fn tick(observation: &Value) -> Option<u64> {
    observation.pointer("/world/tick").and_then(Value::as_u64)
}

pub(crate) fn truncate(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

pub(crate) fn compact_result(result: &Value) -> Value {
    const MAX_BYTES: usize = 1_024;
    if bytes(result) <= MAX_BYTES {
        return result.clone();
    }
    let text = result.to_string();
    json!({"ok":result["ok"],"tool":result["tool"],"excerpt":truncate(&text,750),"truncated":true})
}

pub(crate) fn build(
    observation: &Value,
    state: &AgentState,
    task: &Task,
    budget: &BudgetSnapshot,
    settings: &ContextLimits,
    limit: usize,
) -> Result<Value> {
    let limit = limit.min(settings.max_context_tokens);
    let full_entities = array(observation, "/world/entities");
    let mut counts = BTreeMap::<&str, usize>::new();
    for entity in full_entities {
        if let Some(kind) = entity["kind"].as_str() {
            *counts.entry(kind).or_default() += 1;
        }
    }
    let mut payload = json!({
        "task":task.model_view(),"tasks":[],"recent_player_messages":[],"errors":[],"mods":[],
        "results":[],"world":{"tick":tick(observation),"entity_count":full_entities.len(),"counts":[],"entities":[],"events":[],"mods":[]},
        "logs":[],"journal":[],"budget":budget,
        "order":{"errors":"newest first","events":"newest first","logs":"newest first","journal":"newest first"},
    });
    ensure!(
        bytes(&payload) <= limit,
        "context allowance is too small for the current task and budget"
    );

    for pending in state
        .tasks
        .iter()
        .filter(|pending| !pending.finished() && pending.id != task.id)
        .take(settings.max_pending_tasks)
    {
        if !append(&mut payload, "/tasks", pending.model_view(), limit) {
            break;
        }
    }
    for error in array(observation, "/errors")
        .iter()
        .filter(|error| error["resolved"] != true)
        .take(settings.max_errors)
    {
        if !append(&mut payload, "/errors", error.clone(), limit) {
            let _ = append(
                &mut payload,
                "/errors",
                json!({"mod_name":error["mod_name"],"tick":error["tick"],
                "line":error["line"],"message":truncate(error["message"].as_str().unwrap_or_default(), 180),"truncated":true}),
                limit,
            );
        }
    }

    let instruction = task.instruction.to_ascii_lowercase();
    let mut mods: Vec<_> = array(observation, "/mods").iter().collect();
    mods.sort_by_key(|entry| {
        let name = entry["name"].as_str().unwrap_or_default();
        (
            task.target_mod.as_deref() != Some(name),
            !task.changed_mods.iter().any(|changed| changed == name),
            !instruction.contains(name),
        )
    });
    let mut source_remaining = settings.max_source_bytes;
    for entry in mods.into_iter().take(64) {
        let mut summary = entry.clone();
        if let Some(fields) = summary.as_object_mut() {
            for field in ["candidate", "source"] {
                if let Some(source) = fields.remove(field) {
                    if let Some(source) = source.as_str() {
                        let clipped = truncate(source, source_remaining);
                        source_remaining -= clipped.len();
                        if !clipped.is_empty() {
                            fields.insert(field.to_owned(), json!(clipped));
                        }
                        if source.len() > clipped.len() {
                            fields.insert(format!("{field}_truncated"), json!(true));
                        }
                    }
                }
            }
        }
        if !append(&mut payload, "/mods", summary, limit) {
            let _ = append(
                &mut payload,
                "/mods",
                json!({"name":entry["name"],"status":entry["status"],"source_truncated":true}),
                limit,
            );
        }
    }

    for result in task.results.iter().rev() {
        if !append(&mut payload, "/results", result.clone(), limit) {
            break;
        }
    }
    for (kind, count) in counts {
        if !append(
            &mut payload,
            "/world/counts",
            json!({"kind":kind,"count":count}),
            limit,
        ) {
            break;
        }
    }
    for event in recent(observation, "/world/events").take(40) {
        if !append(&mut payload, "/world/events", event.clone(), limit) {
            break;
        }
    }
    for entity in full_entities
        .iter()
        .filter(|entity| {
            entity["kind"]
                .as_str()
                .is_some_and(|kind| instruction.contains(kind))
        })
        .chain(full_entities.iter().filter(|entity| {
            entity["kind"]
                .as_str()
                .is_none_or(|kind| !instruction.contains(kind))
        }))
        .take(96)
    {
        if !append(&mut payload, "/world/entities", entity.clone(), limit) {
            let _ = append(
                &mut payload,
                "/world/entities",
                json!({"id":entity["id"],"kind":entity["kind"],"props_truncated":true}),
                limit,
            );
        }
    }
    for status in array(observation, "/world/mods").iter().take(64) {
        if !append(&mut payload, "/world/mods", status.clone(), limit) {
            break;
        }
    }
    for log in array(observation, "/logs")
        .iter()
        .take(settings.max_log_entries)
    {
        if !append(&mut payload, "/logs", log.clone(), limit) {
            break;
        }
    }
    for entry in state.recent.iter().rev().take(settings.max_journal_entries) {
        if !append(
            &mut payload,
            "/journal",
            serde_json::to_value(entry)?,
            limit,
        ) {
            break;
        }
    }
    for previous in state
        .tasks
        .iter()
        .rev()
        .filter(|previous| previous.finished() && previous.id != task.id)
        .take(settings.max_journal_entries)
    {
        if !append(
            &mut payload,
            "/recent_player_messages",
            previous.model_view(),
            limit,
        ) {
            break;
        }
    }
    Ok(payload)
}
