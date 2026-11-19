use anyhow::{anyhow, bail, ensure, Result};
use hecs::{Entity, World as Ecs};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type Properties = serde_json::Map<String, Value>;
pub(crate) const MAX_ENTITIES: usize = 4096;
pub(crate) const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub id: u64,
    pub kind: String,
    pub props: Properties,
}

#[derive(Default)]
pub(crate) struct EntityStore {
    ecs: Ecs,
    index: BTreeMap<u64, Entity>,
    kinds: BTreeMap<String, BTreeSet<u64>>,
}

impl EntityStore {
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn get(&self, id: u64) -> Option<EntitySnapshot> {
        let handle = *self.index.get(&id)?;
        self.ecs
            .get::<&EntitySnapshot>(handle)
            .ok()
            .map(|entity| (*entity).clone())
    }

    pub fn query(&self, kind: &str) -> Vec<EntitySnapshot> {
        self.kinds
            .get(kind)
            .into_iter()
            .flatten()
            .filter_map(|id| self.get(*id))
            .collect()
    }

    pub fn all(&self) -> Vec<EntitySnapshot> {
        self.index.keys().filter_map(|id| self.get(*id)).collect()
    }

    pub fn upsert(&mut self, snapshot: EntitySnapshot) {
        debug_assert!(snapshot.id > 0);
        if let Some(handle) = self.index.get(&snapshot.id) {
            let mut current = self.ecs.get::<&mut EntitySnapshot>(*handle).unwrap();
            debug_assert_eq!(current.kind, snapshot.kind);
            *current = snapshot;
        } else {
            let id = snapshot.id;
            self.kinds
                .entry(snapshot.kind.clone())
                .or_default()
                .insert(id);
            self.index.insert(id, self.ecs.spawn((snapshot,)));
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let Some(snapshot) = self.get(id) else {
            return false;
        };
        let handle = self.index.remove(&id).unwrap();
        let ids = self.kinds.get_mut(&snapshot.kind).unwrap();
        ids.remove(&id);
        if ids.is_empty() {
            self.kinds.remove(&snapshot.kind);
        }
        self.ecs.despawn(handle).unwrap();
        true
    }
}

pub(crate) fn validate_label(label: &str, max_bytes: usize) -> Result<()> {
    ensure!(
        !label.trim().is_empty()
            && label.len() <= max_bytes
            && !label.chars().any(char::is_control),
        "label must contain 1 to {max_bytes} bytes of printable text"
    );
    Ok(())
}

pub(crate) fn validate_props(props: &Properties) -> Result<()> {
    validate_map(props, 0)?;
    ensure!(
        serde_json::to_vec(props)?.len() <= MAX_VALUE_BYTES,
        "property map exceeds {MAX_VALUE_BYTES} bytes"
    );
    Ok(())
}

fn validate_map(map: &Properties, depth: usize) -> Result<()> {
    ensure!(map.len() <= 256, "property map has too many keys");
    for (key, value) in map {
        validate_label(key, 128)?;
        validate_value(value, depth + 1)?;
    }
    Ok(())
}

fn validate_value(value: &Value, depth: usize) -> Result<()> {
    ensure!(depth <= 16, "property nesting is too deep");
    match value {
        Value::String(text) => ensure!(text.len() <= 4096, "property string is too large"),
        Value::Array(items) => {
            ensure!(items.len() <= 1024, "property array is too large");
            for item in items {
                validate_value(item, depth + 1)?;
            }
        }
        Value::Object(map) => validate_map(map, depth)?,
        Value::Number(number) if number.is_u64() && number.as_i64().is_none() => {
            bail!("property integer exceeds signed 64-bit range");
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn allocate_id(next_id: &mut u64) -> Result<u64> {
    let id = *next_id;
    ensure!(
        id > 0 && i64::try_from(id).is_ok(),
        "entity identifiers exhausted"
    );
    *next_id = id
        .checked_add(1)
        .ok_or_else(|| anyhow!("entity identifiers exhausted"))?;
    Ok(id)
}
