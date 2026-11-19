use crate::{
    entity::{allocate_id, validate_label, validate_props, EntityStore, Properties, MAX_ENTITIES},
    error_bus::{Diagnostic, ErrorBus, MAX_ERRORS},
    script_host::{
        Invocation, Program, ScriptFailure, ScriptHost, SharedEntities, Subscription, Timer,
        MAX_PENDING_EVENTS, MAX_SUBSCRIPTIONS, MAX_TIMERS,
    },
};
use anyhow::{anyhow, ensure, Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub use crate::{entity::EntitySnapshot, script_host::WorldEvent};

pub const MAX_MOD_SOURCE_BYTES: usize = 64 * 1024;
pub const FAILURE_THRESHOLD: u32 = 3;
const SCHEMA_VERSION: u32 = 1;
const MAX_MODS: usize = 16;
const MAX_LOGS: usize = 256;
const MAX_EVENTS: usize = 256;
const MAX_TIMER_WORK: usize = 64;
const MAX_EVENT_WORK: usize = 64;
const MAX_EVENTS_PER_TICK: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldLog {
    pub tick: u64,
    pub level: String,
    pub message: String,
    pub mod_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModInfo {
    pub name: String,
    pub status: String,
    pub active: bool,
    pub failures: u32,
    pub has_candidate: bool,
    pub has_previous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSource {
    pub name: String,
    pub source: Option<String>,
    pub candidate: Option<String>,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub entities: Vec<EntitySnapshot>,
    pub events: Vec<WorldEvent>,
    pub mods: Vec<ModInfo>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ModRecord {
    source: Option<String>,
    previous: Option<String>,
    candidate: Option<String>,
    active: bool,
    failures: u32,
    callback_failures: BTreeMap<String, u32>,
    revision: u64,
    subscriptions: Vec<Subscription>,
    timers: Vec<Timer>,
}

impl ModRecord {
    fn info(&self, name: &str) -> ModInfo {
        let status = match (self.active, self.source.is_some()) {
            (true, _) => "loaded",
            (false, true) => "disabled",
            (false, false) => "rejected",
        };
        ModInfo {
            name: name.to_owned(),
            status: status.to_owned(),
            active: self.active,
            failures: self.failures,
            has_candidate: self.candidate.is_some(),
            has_previous: self.previous.is_some(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct EventTarget {
    mod_name: String,
    callback: String,
    revision: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingEvent {
    event: WorldEvent,
    targets: Option<VecDeque<EventTarget>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorldState {
    pub version: u32,
    pub tick: u64,
    pub seed: u64,
    pub entities: Vec<EntitySnapshot>,
    next_id: u64,
    next_revision: u64,
    rng: ChaCha8Rng,
    mods: BTreeMap<String, ModRecord>,
    pending_events: VecDeque<PendingEvent>,
    events: VecDeque<WorldEvent>,
    logs: VecDeque<WorldLog>,
    errors: VecDeque<Diagnostic>,
}

pub struct World {
    tick: u64,
    seed: u64,
    next_id: u64,
    next_revision: u64,
    rng: ChaCha8Rng,
    entities: SharedEntities,
    host: ScriptHost,
    programs: BTreeMap<String, Program>,
    mods: BTreeMap<String, ModRecord>,
    pending_events: VecDeque<PendingEvent>,
    dispatching_event: bool,
    events: VecDeque<WorldEvent>,
    logs: VecDeque<WorldLog>,
    errors: ErrorBus,
}

impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            next_id: 1,
            next_revision: 1,
            rng: ChaCha8Rng::seed_from_u64(seed),
            entities: SharedEntities::default(),
            host: ScriptHost::new(),
            programs: BTreeMap::new(),
            mods: BTreeMap::new(),
            pending_events: VecDeque::new(),
            dispatching_event: false,
            events: VecDeque::new(),
            logs: VecDeque::new(),
            errors: ErrorBus::default(),
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            tick: self.tick,
            entities: self.entities().all(),
            events: self.events.iter().cloned().collect(),
            mods: self.list_mods(),
        }
    }

    pub fn spawn(&mut self, kind: &str, props: Properties) -> Result<u64> {
        validate_label(kind, 64)?;
        validate_props(&props)?;
        ensure!(
            self.entities().len() < MAX_ENTITIES,
            "world entity limit reached"
        );
        let id = allocate_id(&mut self.next_id)?;
        self.entities().upsert(EntitySnapshot {
            id,
            kind: kind.to_owned(),
            props,
        });
        self.record_log("info", format!("spawned {kind} #{id}"));
        Ok(id)
    }

    pub fn despawn(&mut self, id: u64) -> bool {
        let removed = self.entities().remove(id);
        if removed {
            self.record_log("info", format!("despawned entity #{id}"));
        }
        removed
    }

    pub fn get(&self, id: u64) -> Option<EntitySnapshot> {
        self.entities().get(id)
    }

    pub fn query(&self, kind: &str) -> Vec<EntitySnapshot> {
        self.entities().query(kind)
    }

    pub fn set(&mut self, id: u64, key: &str, value: serde_json::Value) -> Result<bool> {
        let Some(mut snapshot) = self.get(id) else {
            return Ok(false);
        };
        snapshot.props.insert(key.to_string(), value);
        validate_props(&snapshot.props)?;
        self.entities().upsert(snapshot);
        Ok(true)
    }

    pub fn recent_logs(&self, level: Option<&str>, count: usize) -> Vec<WorldLog> {
        self.logs
            .iter()
            .rev()
            .filter(|entry| level.is_none_or(|level| entry.level.eq_ignore_ascii_case(level)))
            .take(count)
            .cloned()
            .collect()
    }

    pub fn recent_errors(&self, count: usize) -> Vec<Diagnostic> {
        self.errors.recent(count)
    }

    pub fn record_log(&mut self, level: impl Into<String>, message: impl Into<String>) {
        self.log(level.into(), message.into(), None);
    }

    pub fn record_diagnostic(&mut self, mut diagnostic: Diagnostic) {
        bound_text(&mut diagnostic.message, 4096);
        if let Some(snippet) = diagnostic.repro_snippet.as_mut() {
            bound_text(snippet, 512);
        }
        tracing::warn!(
            mod_name = %diagnostic.mod_name,
            tick = diagnostic.tick,
            category = %diagnostic.category,
            failure_count = diagnostic.failure_count,
            disabled = diagnostic.disabled,
            message = %diagnostic.message,
            "mod diagnostic"
        );
        self.errors.record(diagnostic);
    }

    pub fn list_mods(&self) -> Vec<ModInfo> {
        self.mods
            .iter()
            .map(|(name, record)| record.info(name))
            .collect()
    }

    pub fn read_mod(&self, name: &str) -> Result<ModSource> {
        let name = validate_mod_name(name)?;
        let record = self
            .mods
            .get(name)
            .ok_or_else(|| anyhow!("mod {name:?} was not found"))?;
        Ok(ModSource {
            name: name.to_owned(),
            source: record.source.clone(),
            candidate: record.candidate.clone(),
            previous: record.previous.clone(),
        })
    }

    pub fn write_mod(&mut self, name: &str, source: &str) -> Result<ModInfo> {
        let name = validate_mod_name(name)?;
        ensure!(
            self.mods.contains_key(name) || self.mods.len() < MAX_MODS,
            "world mod limit reached"
        );
        if source.len() > MAX_MOD_SOURCE_BYTES {
            let message = format!("mod source exceeds {MAX_MOD_SOURCE_BYTES} bytes");
            self.script_error(
                name,
                "",
                None,
                ScriptFailure {
                    category: "source_limit",
                    line: None,
                    message: message.clone(),
                },
                0,
                false,
            );
            return Err(anyhow!(message));
        }

        if let Some(record) = self.mods.get_mut(name) {
            if record.active && record.source.as_deref() == Some(source) {
                record.candidate = None;
                self.errors.resolve(name, None);
                return Ok(record.info(name));
            }
        }

        let activate = || -> std::result::Result<_, ScriptFailure> {
            let program = self.host.compile(source)?;
            let invocation = self
                .host
                .initialize(&program, self.invocation(Vec::new(), Vec::new()))?;
            Ok((program, invocation))
        };
        let (program, invocation) = match activate() {
            Ok(activated) => activated,
            Err(error) => {
                let message = error.message.clone();
                let record = self.mods.entry(name.to_owned()).or_default();
                record.candidate = Some(source.to_owned());
                let failures = record.failures;
                let disabled = record.source.is_some() && !record.active;
                self.script_error(name, source, None, error, failures, disabled);
                return Err(anyhow!(message));
            }
        };

        let revision = self.next_revision;
        self.next_revision = revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("mod revisions exhausted"))?;
        let record = self.mods.entry(name.to_owned()).or_default();
        if record.active && record.failures == 0 {
            record.previous = record.source.take();
        }
        record.source = Some(source.to_owned());
        record.candidate = None;
        record.active = true;
        record.failures = 0;
        record.callback_failures.clear();
        record.revision = revision;
        self.errors
            .resolve_removed_callbacks(name, |callback| program.callback_arity(callback).is_some());
        self.programs.insert(name.to_owned(), program);
        self.commit(name, invocation);
        self.errors.resolve(name, None);
        self.log(
            "info".into(),
            format!("loaded mod {name}"),
            Some(name.to_owned()),
        );
        Ok(self.mods[name].info(name))
    }

    pub fn restore_mod(&mut self, name: &str) -> Result<ModInfo> {
        let source = self.read_mod(name)?;
        let previous = source
            .previous
            .ok_or_else(|| anyhow!("mod {name:?} has no previous version"))?;
        self.write_mod(name, &previous)
    }

    pub fn delete_mod(&mut self, name: &str) -> Result<bool> {
        let name = validate_mod_name(name)?;
        self.programs.remove(name);
        let removed = self.mods.remove(name).is_some();
        if removed {
            self.errors.resolve_removed_callbacks(name, |_| false);
            self.log(
                "info".into(),
                format!("deleted mod {name}"),
                Some(name.to_owned()),
            );
        }
        Ok(removed)
    }

    pub fn run_ticks(&mut self, count: u64) -> Result<()> {
        self.tick
            .checked_add(count)
            .ok_or_else(|| anyhow!("world tick overflow"))?;
        for _ in 0..count {
            self.tick += 1;
            tracing::trace!(tick = self.tick, "simulation tick");
            let args = Properties::from_iter([("tick".into(), json!(self.tick))]);
            for target in self.targets("tick") {
                self.invoke(&target, &args);
            }

            for _ in 0..MAX_TIMER_WORK {
                let Some(target) = self.take_timer() else {
                    break;
                };
                self.invoke(&target, &args);
            }

            if self.tick.is_multiple_of(100) {
                let roll = self.rng.gen_range(0..=999_i64);
                self.enqueue_event(WorldEvent {
                    tick: self.tick,
                    name: "ambient".to_owned(),
                    data: Properties::from_iter([("roll".into(), json!(roll))]),
                });
            }
            self.dispatch_events();
        }
        Ok(())
    }

    pub fn export(&self) -> WorldState {
        WorldState {
            version: SCHEMA_VERSION,
            tick: self.tick,
            seed: self.seed,
            entities: self.entities().all(),
            next_id: self.next_id,
            next_revision: self.next_revision,
            rng: self.rng.clone(),
            mods: self.mods.clone(),
            pending_events: self.pending_events.clone(),
            events: self.events.clone(),
            logs: self.logs.clone(),
            errors: self.errors.entries().clone(),
        }
    }

    pub fn restore(state: WorldState) -> Result<Self> {
        ensure!(
            state.version == SCHEMA_VERSION,
            "unsupported world schema version {}",
            state.version
        );
        ensure!(
            state.entities.len() <= MAX_ENTITIES,
            "saved world exceeds the entity limit"
        );
        ensure!(
            state.mods.len() <= MAX_MODS,
            "saved world exceeds the mod limit"
        );
        ensure!(
            state.pending_events.len() <= MAX_PENDING_EVENTS,
            "saved event queue exceeds its limit"
        );
        ensure!(
            state.events.len() <= MAX_EVENTS,
            "saved event history exceeds its limit"
        );
        ensure!(
            state.logs.len() <= MAX_LOGS,
            "saved log history exceeds its limit"
        );
        ensure!(
            state.errors.len() <= MAX_ERRORS,
            "saved error history exceeds its limit"
        );
        ensure!(state.next_revision > 0, "invalid next mod revision");
        ensure!(
            state.next_id > 0 && state.next_id <= i64::MAX as u64 + 1,
            "invalid next entity identifier"
        );
        let mut world = Self::new(state.seed);
        world.tick = state.tick;
        world.next_id = state.next_id;
        world.next_revision = state.next_revision;
        world.rng = state.rng;
        let mut ids = BTreeSet::new();
        for entity in state.entities {
            ensure!(
                entity.id > 0 && entity.id < world.next_id,
                "invalid saved entity ID"
            );
            ensure!(
                ids.insert(entity.id),
                "duplicate saved entity ID {}",
                entity.id
            );
            validate_label(&entity.kind, 64).context("invalid saved entity kind")?;
            validate_props(&entity.props).context("invalid saved entity properties")?;
            world.entities().upsert(entity);
        }
        for (name, record) in state.mods {
            world.restore_record(name, record)?;
        }
        for event in state
            .events
            .iter()
            .chain(state.pending_events.iter().map(|pending| &pending.event))
        {
            validate_label(&event.name, 64).context("invalid saved event name")?;
            ensure!(
                event.tick <= world.tick,
                "saved event is from a future tick"
            );
            validate_props(&event.data).context("invalid saved event data")?;
        }
        for pending in &state.pending_events {
            if let Some(targets) = &pending.targets {
                ensure!(
                    targets.len() <= MAX_MODS * MAX_SUBSCRIPTIONS,
                    "invalid saved event targets"
                );
                for target in targets {
                    validate_mod_name(&target.mod_name)?;
                    validate_label(&target.callback, MAX_MOD_SOURCE_BYTES)?;
                    ensure!(
                        target.revision > 0 && target.revision < world.next_revision,
                        "invalid saved event target revision"
                    );
                }
            }
        }
        world.pending_events = state.pending_events;
        world.events = state.events;
        world.logs = state.logs;
        world.errors = ErrorBus::from_entries(state.errors);
        Ok(world)
    }

    fn restore_record(&mut self, name: String, record: ModRecord) -> Result<()> {
        ensure!(validate_mod_name(&name)? == name, "invalid saved mod name");
        ensure!(
            record.subscriptions.len() <= MAX_SUBSCRIPTIONS,
            "invalid saved subscriptions"
        );
        ensure!(record.timers.len() <= MAX_TIMERS, "invalid saved timers");
        ensure!(
            record.failures <= FAILURE_THRESHOLD,
            "invalid saved mod failure count"
        );
        ensure!(
            record
                .callback_failures
                .values()
                .all(|streak| (1..=FAILURE_THRESHOLD).contains(streak))
                && record
                    .callback_failures
                    .values()
                    .copied()
                    .max()
                    .unwrap_or(0)
                    == record.failures,
            "invalid saved callback failure counts"
        );
        ensure!(
            !record.active || record.failures < FAILURE_THRESHOLD,
            "active mod exceeded failure limit"
        );
        for source in [
            record.source.as_ref(),
            record.previous.as_ref(),
            record.candidate.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                source.len() <= MAX_MOD_SOURCE_BYTES,
                "saved mod source exceeds its limit"
            );
        }
        if let Some(source) = &record.source {
            ensure!(
                record.revision > 0 && record.revision < self.next_revision,
                "invalid saved mod revision"
            );
            let program = self
                .host
                .compile(source)
                .map_err(|error| anyhow!("saved mod {name:?}: {}", error.message))?;
            ensure!(
                record
                    .callback_failures
                    .keys()
                    .all(|name| program.callback_arity(name).is_some()),
                "invalid saved callbacks with failures"
            );
            for subscription in &record.subscriptions {
                validate_label(&subscription.event, 64)
                    .context("invalid saved subscription event")?;
                ensure!(
                    program.callback_arity(&subscription.callback).is_some(),
                    "invalid saved subscription callback"
                );
            }
            for timer in &record.timers {
                ensure!(
                    timer.due > 0 && program.callback_arity(&timer.callback).is_some(),
                    "invalid saved timer callback"
                );
            }
            if record.active {
                self.programs.insert(name.clone(), program);
            } else {
                ensure!(
                    record.subscriptions.is_empty() && record.timers.is_empty(),
                    "inactive mod has saved callbacks"
                );
            }
        } else {
            ensure!(
                !record.active
                    && record.revision == 0
                    && record.candidate.is_some()
                    && record.subscriptions.is_empty()
                    && record.timers.is_empty(),
                "saved mod has invalid state"
            );
        }
        self.mods.insert(name, record);
        Ok(())
    }

    fn entities(&self) -> std::sync::MutexGuard<'_, EntityStore> {
        self.entities
            .lock()
            .expect("simulation entity store poisoned")
    }

    fn invocation(&self, subscriptions: Vec<Subscription>, timers: Vec<Timer>) -> Invocation {
        Invocation::new(
            self.entities.clone(),
            self.tick,
            self.next_id,
            MAX_PENDING_EVENTS
                .saturating_sub(self.pending_events.len() + usize::from(self.dispatching_event)),
            subscriptions,
            timers,
        )
    }

    fn commit(&mut self, name: &str, invocation: Invocation) {
        self.next_id = invocation.next_id;
        {
            let mut entities = self.entities();
            for (id, entity) in invocation.changes {
                if let Some(entity) = entity {
                    tracing::trace!(mod_name = name, entity = id, "script upserted entity");
                    entities.upsert(entity);
                } else if entities.remove(id) {
                    tracing::trace!(mod_name = name, entity = id, "script despawned entity");
                }
            }
        }
        let record = self.mods.get_mut(name).expect("mod record missing");
        record.subscriptions = invocation.subscriptions;
        record.timers = invocation.timers;
        for event in invocation.events {
            self.enqueue_event(event);
        }
        for message in invocation.logs {
            self.log("info".into(), message, Some(name.to_owned()));
        }
    }

    fn invoke(&mut self, target: &EventTarget, args: &Properties) {
        let name = &target.mod_name;
        let Some(record) = self.mods.get(name) else {
            return;
        };
        if !record.active || record.revision != target.revision {
            return;
        }
        let program = self.programs.get(name).expect("active mod program missing");
        let invocation = self.invocation(record.subscriptions.clone(), record.timers.clone());
        match self
            .host
            .callback(program, &target.callback, args, invocation)
        {
            Ok(invocation) => {
                self.commit(name, invocation);
                let record = self.mods.get_mut(name).expect("mod record missing");
                record.callback_failures.remove(&target.callback);
                record.failures = record
                    .callback_failures
                    .values()
                    .copied()
                    .max()
                    .unwrap_or(0);
                self.errors.resolve(name, Some(&target.callback));
            }
            Err(error) => {
                let record = self.mods.get_mut(name).expect("mod record missing");
                let streak = record
                    .callback_failures
                    .entry(target.callback.clone())
                    .or_default();
                *streak += 1;
                let failures = *streak;
                record.failures = record.failures.max(failures);
                let disabled = failures >= FAILURE_THRESHOLD;
                let source = record.source.clone().expect("active mod source missing");
                if disabled {
                    record.active = false;
                    record.subscriptions.clear();
                    record.timers.clear();
                    self.programs.remove(name);
                }
                self.script_error(
                    name,
                    &source,
                    Some(&target.callback),
                    error,
                    failures,
                    disabled,
                );
            }
        }
    }

    fn script_error(
        &mut self,
        name: &str,
        source: &str,
        callback: Option<&str>,
        error: ScriptFailure,
        failures: u32,
        disabled: bool,
    ) {
        let repro_snippet = error
            .line
            .and_then(|line| line.checked_sub(1))
            .and_then(|line| source.lines().nth(line))
            .map(str::to_owned);
        self.record_diagnostic(Diagnostic {
            mod_name: name.to_owned(),
            line: error.line,
            message: error.message,
            tick: self.tick,
            repro_snippet,
            severity: "error".to_owned(),
            category: error.category.to_owned(),
            callback: callback.map(str::to_owned),
            failure_count: failures,
            disabled,
            resolved: false,
        });
    }

    fn targets(&self, event: &str) -> VecDeque<EventTarget> {
        let mut targets = VecDeque::new();
        for (name, record) in &self.mods {
            if record.active {
                for subscription in &record.subscriptions {
                    if subscription.event == event {
                        targets.push_back(EventTarget {
                            mod_name: name.to_owned(),
                            callback: subscription.callback.clone(),
                            revision: record.revision,
                        });
                    }
                }
            }
        }
        targets
    }

    fn take_timer(&mut self) -> Option<EventTarget> {
        let (_, name, index) = self
            .mods
            .iter()
            .filter(|(_, record)| record.active)
            .flat_map(|(name, record)| {
                record
                    .timers
                    .iter()
                    .enumerate()
                    .map(move |(index, timer)| (timer.due, name, index))
            })
            .filter(|(due, _, _)| *due <= self.tick)
            .min()?;
        let name = name.clone();
        let record = self.mods.get_mut(&name).expect("timer mod record missing");
        let timer = record.timers.remove(index);
        Some(EventTarget {
            mod_name: name,
            callback: timer.callback,
            revision: record.revision,
        })
    }

    fn enqueue_event(&mut self, event: WorldEvent) {
        if self.pending_events.len() >= MAX_PENDING_EVENTS {
            self.record_log("warn", format!("event queue full; dropped {}", event.name));
            return;
        }
        push_bounded(&mut self.events, event.clone(), MAX_EVENTS);
        self.pending_events.push_back(PendingEvent {
            event,
            targets: None,
        });
    }

    fn dispatch_events(&mut self) {
        let mut callbacks = 0;
        for _ in 0..MAX_EVENTS_PER_TICK {
            let Some(mut pending) = self.pending_events.pop_front() else {
                break;
            };
            let mut targets = pending
                .targets
                .take()
                .unwrap_or_else(|| self.targets(&pending.event.name));
            self.dispatching_event = true;
            while callbacks < MAX_EVENT_WORK {
                let Some(target) = targets.pop_front() else {
                    break;
                };
                self.invoke(&target, &pending.event.data);
                callbacks += 1;
            }
            self.dispatching_event = false;
            if !targets.is_empty() {
                pending.targets = Some(targets);
                self.pending_events.push_front(pending);
                break;
            }
        }
    }

    fn log(&mut self, level: String, mut message: String, mod_name: Option<String>) {
        bound_text(&mut message, 4096);
        match level.as_str() {
            "trace" => tracing::trace!(tick = self.tick, mod_name, %message),
            "debug" => tracing::debug!(tick = self.tick, mod_name, %message),
            "warn" => tracing::warn!(tick = self.tick, mod_name, %message),
            "error" => tracing::error!(tick = self.tick, mod_name, %message),
            _ => tracing::info!(tick = self.tick, mod_name, %message),
        }
        push_bounded(
            &mut self.logs,
            WorldLog {
                tick: self.tick,
                level,
                message,
                mod_name,
            },
            MAX_LOGS,
        );
    }
}

pub fn validate_mod_name(name: &str) -> Result<&str> {
    let name = name.strip_suffix(".rhai").unwrap_or(name);
    ensure!(
        (1..=48).contains(&name.len())
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'),
        "mod names must use 1 to 48 lowercase ASCII letters, digits, underscores or hyphens and start with a letter or digit"
    );
    let device = matches!(name, "aux" | "con" | "nul" | "prn")
        || (name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && name.as_bytes()[3].is_ascii_digit());
    ensure!(!device, "mod name is reserved");
    Ok(name)
}

fn push_bounded<T>(entries: &mut VecDeque<T>, entry: T, limit: usize) {
    if entries.len() == limit {
        entries.pop_front();
    }
    entries.push_back(entry);
}

fn bound_text(text: &mut String, limit: usize) {
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}
