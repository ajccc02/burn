use crate::{
    entity::Properties,
    error_bus::Diagnostic,
    llm::ToolDefinition,
    persist,
    replay::{Recorder, WorldAction},
    world::{validate_mod_name, ModInfo, World, WorldState, MAX_MOD_SOURCE_BYTES},
};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{interval, MissedTickBehavior};

const MAX_ITEMS: usize = 100;
const MAX_TICKS: u64 = 120;
const MAX_MOD_FILES: usize = 16;
const MAX_ARGUMENT_BYTES: usize = MAX_MOD_SOURCE_BYTES * 6 + 1024;

#[derive(Deserialize)]
#[serde(
    tag = "tool",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Action {
    GetWorldSnapshot {},
    GetRecentLogs {
        level: Option<String>,
        n: usize,
    },
    GetRecentErrors {
        n: usize,
    },
    ListMods {},
    ReadMod {
        name: String,
    },
    WriteMod {
        name: String,
        source: String,
    },
    DeleteMod {
        name: String,
    },
    RestoreMod {
        name: String,
    },
    ReloadMods {},
    RunTicks {
        n: u64,
    },
    SpawnEntity {
        kind: String,
        props_json: Properties,
    },
    SendMessageToPlayer {
        text: String,
    },
    ReportBug {
        summary: String,
        repro: String,
    },
    GetBudget {},
    #[serde(skip)]
    Observation,
    #[serde(skip)]
    CheckpointObservation,
    #[serde(skip)]
    Save,
    #[serde(skip)]
    Shutdown,
}

#[derive(Debug, Serialize)]
struct ToolError {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<Box<Diagnostic>>,
}

impl ToolError {
    fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
            diagnostic: None,
        }
    }
}

type Reply = std::result::Result<Value, ToolError>;

struct Command {
    action: Action,
    repair_target: Option<String>,
    reply: oneshot::Sender<Reply>,
}

#[derive(Clone)]
pub struct Tools {
    queue: mpsc::Sender<Command>,
}

impl Tools {
    pub fn start(
        run_dir: &Path,
        seed: u64,
        tick_rate: u32,
        notices: broadcast::Sender<String>,
    ) -> Result<Self> {
        if !(1..=60).contains(&tick_rate) {
            bail!("tick rate must be between 1 and 60");
        }
        let lock = persist::lock_run(run_dir)?;
        let mods_dir = run_dir.join("mods");
        fs::create_dir_all(&mods_dir)?;
        check_directory(&mods_dir)?;
        fs::create_dir_all(run_dir.join("agent"))?;
        let state = persist::read_json::<WorldState>(&run_dir.join("world.json"))?;
        let new_save = state.is_none();
        if new_save
            && (run_dir.join("budget.json").try_exists()?
                || run_dir.join("agent/state.json").try_exists()?)
        {
            bail!("world.json is missing from an existing save");
        }
        let world = match state {
            Some(state) => World::restore(state).context("restoring saved world")?,
            None => World::new(seed),
        };
        let recorder = Recorder::start(run_dir, &world).context("starting world action journal")?;
        let mut run = Run {
            world,
            run_dir: run_dir.to_path_buf(),
            mods_dir,
            notices,
            journal_seen: HashSet::new(),
            faulted: false,
            lock: Some(lock),
            recorder: Some(recorder),
        };
        run.journal_seen = run
            .errors_with_keys()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        if new_save {
            run.reload()?;
            run.save()?;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?;
        let (queue, receiver) = mpsc::channel(32);
        std::thread::Builder::new()
            .name("spawn-lite-world".into())
            .spawn(move || {
                runtime.block_on(run.serve(receiver, tick_rate));
            })?;
        Ok(Self { queue })
    }

    async fn send(&self, action: Action) -> Reply {
        self.send_guarded(action, None).await
    }

    async fn send_guarded(&self, action: Action, repair_target: Option<String>) -> Reply {
        let (reply, response) = oneshot::channel();
        self.queue
            .send(Command {
                action,
                repair_target,
                reply,
            })
            .await
            .map_err(|_| ToolError::new("WORLD_OFFLINE", "Simulation has stopped"))?;
        response
            .await
            .map_err(|_| ToolError::new("WORLD_OFFLINE", "Simulation stopped before replying"))?
    }

    async fn local(&self, action: Action) -> Result<Value> {
        self.send(action)
            .await
            .map_err(|error| anyhow!("{}: {}", error.code, error.message))
    }

    pub async fn observation(&self) -> Result<Value> {
        self.local(Action::Observation).await
    }

    pub async fn checkpoint_observation(&self) -> Result<Value> {
        self.local(Action::CheckpointObservation).await
    }

    pub async fn save(&self) -> Result<()> {
        self.local(Action::Save).await.map(drop)
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.local(Action::Shutdown).await.map(drop)
    }

    pub async fn execute(&self, name: &str, args: Value) -> Value {
        self.execute_guarded(name, args, None).await
    }

    pub async fn execute_repair(&self, name: &str, args: Value, target: &str) -> Value {
        self.execute_guarded(name, args, Some(target.to_owned()))
            .await
    }

    async fn execute_guarded(
        &self,
        name: &str,
        args: Value,
        repair_target: Option<String>,
    ) -> Value {
        let result = match validate_action(name, args) {
            Ok(action) => self.send_guarded(action, repair_target).await,
            Err(error) => Err(error),
        };
        match result {
            Ok(data) => json!({"ok":true,"tool":name,"data":data}),
            Err(error) => json!({"ok":false,"tool":name,"error":error}),
        }
    }
}

fn validate_action(name: &str, args: Value) -> std::result::Result<Action, ToolError> {
    if !args.is_object()
        || serde_json::to_vec(&args).map_or(true, |bytes| bytes.len() > MAX_ARGUMENT_BYTES)
    {
        return Err(ToolError::new(
            "INVALID_ARGUMENTS",
            "Expected a bounded JSON object",
        ));
    }
    let action: Action = serde_json::from_value(json!({"tool":name,"arguments":args}))
        .map_err(|error| ToolError::new("INVALID_TOOL_CALL", error))?;
    let validation = match &action {
        Action::GetRecentLogs { level, n } => {
            if level
                .as_deref()
                .is_some_and(|s| !matches!(s, "trace" | "debug" | "info" | "warn" | "error"))
            {
                Err(anyhow!("Unsupported log level"))
            } else {
                bounded_count(*n as u64, MAX_ITEMS as u64)
            }
        }
        Action::GetRecentErrors { n } => bounded_count(*n as u64, MAX_ITEMS as u64),
        Action::ReadMod { name } | Action::DeleteMod { name } | Action::RestoreMod { name } => {
            validate_mod_name(name).map(drop)
        }
        Action::WriteMod { name, source } => validate_mod_name(name).map(drop).and_then(|()| {
            if source.len() > MAX_MOD_SOURCE_BYTES {
                bail!("mod source exceeds {MAX_MOD_SOURCE_BYTES} bytes");
            }
            Ok(())
        }),
        Action::RunTicks { n } => bounded_count(*n, MAX_TICKS),
        Action::SendMessageToPlayer { text } => bounded_text(text, 2048),
        Action::ReportBug { summary, repro } => {
            bounded_text(summary, 2048).and_then(|()| bounded_text(repro, 4096))
        }
        _ => Ok(()),
    };
    validation.map_err(|error| ToolError::new("INVALID_ARGUMENTS", error))?;
    Ok(action)
}

fn bounded_count(n: u64, max: u64) -> Result<()> {
    if !(1..=max).contains(&n) {
        bail!("count must be between 1 and {max}");
    }
    Ok(())
}

fn bounded_text(text: &str, max: usize) -> Result<()> {
    if text.trim().is_empty() || text.len() > max {
        bail!("expected nonblank text of at most {max} bytes");
    }
    Ok(())
}

pub fn definitions() -> Vec<ToolDefinition> {
    let string = || json!({"type":"string"});
    let integer = |max| json!({"type":"integer","minimum":1,"maximum":max});
    let def = |name: &str, description: &str, properties: Value, required: &[&str]| {
        ToolDefinition {
            name: name.into(),
            description: description.into(),
            parameters: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        }
    };
    vec![
        def("get_world_snapshot", "Inspect entities, tick, mod status, and recent world events.", json!({}), &[]),
        def("get_recent_logs", "Get recent world logs, optionally filtered by exact level.", json!({"level":{"enum":["trace","debug","info","warn","error"]},"n":integer(MAX_ITEMS as u64)}), &["n"]),
        def("get_recent_errors", "Get recent script diagnostics with source lines when available.", json!({"n":integer(MAX_ITEMS as u64)}), &["n"]),
        def("list_mods", "List installed, disabled, and rejected mods.", json!({}), &[]),
        def("read_mod", "Read active source and any rejected candidate.", json!({"name":string()}), &["name"]),
        def("write_mod", "Validate and activate a complete Rhai source file. Preserve working behavior if validation fails.", json!({"name":string(),"source":string()}), &["name","source"]),
        def("delete_mod", "Remove a mod and its registrations. Existing entities remain.", json!({"name":string()}), &["name"]),
        def("restore_mod", "Restore the last working mod version.", json!({"name":string()}), &["name"]),
        def("reload_mods", "Read and validate changed .rhai files from the save's mods directory.", json!({}), &[]),
        def("run_ticks", "Advance the world by up to 120 ticks and return observable state.", json!({"n":integer(MAX_TICKS)}), &["n"]),
        def("spawn_entity", "Create a single entity with the specified kind and JSON properties.", json!({"kind":string(),"props_json":{"type":"object"}}), &["kind","props_json"]),
        def("send_message_to_player", "Send useful progress or questions to the player. Use verification to complete a changed-world task.", json!({"text":string()}), &["text"]),
        def("report_bug", "Record a reproducible simulation issue.", json!({"summary":string(),"repro":string()}), &["summary","repro"]),
        def("get_budget", "Check remaining model calls, tokens, and estimated spending.", json!({}), &[]),
    ]
}

struct Run {
    world: World,
    run_dir: PathBuf,
    mods_dir: PathBuf,
    notices: broadcast::Sender<String>,
    journal_seen: HashSet<String>,
    faulted: bool,
    lock: Option<File>,
    recorder: Option<Recorder>,
}

impl Run {
    async fn serve(&mut self, mut receiver: mpsc::Receiver<Command>, tick_rate: u32) {
        let mut ticks = interval(Duration::from_secs_f64(1.0 / f64::from(tick_rate)));
        let mut saves = interval(Duration::from_secs(10));
        ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
        saves.set_missed_tick_behavior(MissedTickBehavior::Skip);
        ticks.tick().await;
        saves.tick().await;
        tracing::info!(tick_rate, tick = self.world.tick(), "simulation started");
        loop {
            tokio::select! {
                command = receiver.recv() => {
                    let Some(command) = command else {
                        if let Err(error) = self.checkpoint() { tracing::error!(%error, "final world save failed"); }
                        break;
                    };
                    let stopping = matches!(command.action, Action::Shutdown);
                    let reply = if command.repair_target.as_ref().is_some_and(|name| self.world.read_mod(name).is_err()) {
                        Err(ToolError::new("REPAIR_TARGET_REMOVED", "The repair target was removed before this action could run"))
                    } else { self.dispatch(command.action) };
                    self.update_error_journal();
                    if stopping { drop(self.lock.take()); }
                    let _ = command.reply.send(reply);
                    if stopping { break; }
                }
                _ = ticks.tick(), if !self.faulted => {
                    if let Err(error) = self.world.run_ticks(1) {
                        self.faulted = true;
                        self.notice(format!("Simulation ticks paused after a world failure: {error:#}"));
                        tracing::error!(%error, "world tick failed");
                    }
                    self.update_error_journal();
                }
                _ = saves.tick() => {
                    if let Err(error) = self.checkpoint() { tracing::error!(%error, "world autosave failed"); }
                }
            }
        }
        tracing::info!(tick = self.world.tick(), "simulation stopped");
    }

    fn dispatch(&mut self, action: Action) -> Reply {
        tracing::trace!(tick = self.world.tick(), "executing simulation command");
        match action {
            Action::GetWorldSnapshot {} => Ok(json!(self.world.snapshot())),
            Action::GetRecentLogs { level, n } => {
                Ok(json!(self.world.recent_logs(level.as_deref(), n)))
            }
            Action::GetRecentErrors { n } => Ok(json!(self.world.recent_errors(n))),
            Action::ListMods {} => Ok(json!(self.world.list_mods())),
            Action::ReadMod { name } => self
                .world
                .read_mod(&name)
                .map(|source| json!(source))
                .map_err(|error| ToolError::new("MOD_NOT_FOUND", error)),
            Action::WriteMod { name, source } => self.write_mod(&name, &source),
            Action::DeleteMod { name } => self.delete_mod(&name),
            Action::RestoreMod { name } => self.restore_mod(&name),
            Action::ReloadMods {} => self
                .reload()
                .map_err(|error| ToolError::new("MOD_IO_ERROR", error)),
            Action::RunTicks { n } => {
                let tick = self.world.tick();
                self.world
                    .run_ticks(n)
                    .map_err(|error| ToolError::new("TICK_FAILED", error))?;
                self.record(tick, WorldAction::Advance { ticks: n });
                Ok(json!(self.world.snapshot()))
            }
            Action::SpawnEntity { kind, props_json } => {
                let id = self
                    .world
                    .spawn(&kind, props_json.clone())
                    .map_err(|error| ToolError::new("INVALID_ENTITY", error))?;
                self.record(
                    self.world.tick(),
                    WorldAction::Spawn {
                        kind,
                        props: props_json,
                        id,
                    },
                );
                Ok(json!({"id":id,"tick":self.world.tick()}))
            }
            Action::SendMessageToPlayer { text } => {
                self.notice(text);
                Ok(json!({"sent":true}))
            }
            Action::ReportBug { summary, repro } => {
                self.world.record_diagnostic(diagnostic(
                    self.world.tick(),
                    "",
                    "agent_report",
                    summary,
                    Some(repro),
                ));
                Ok(json!({"recorded":true}))
            }
            Action::GetBudget {} => Err(ToolError::new(
                "AGENT_ONLY",
                "Budget is provided by the agent runtime",
            )),
            Action::Save | Action::Shutdown => self
                .checkpoint()
                .map(|()| json!({"saved":true}))
                .map_err(|error| ToolError::new("SAVE_FAILED", format!("{error:#}"))),
            Action::Observation => Ok(self.observation()),
            Action::CheckpointObservation => {
                self.checkpoint()
                    .map_err(|error| ToolError::new("SAVE_FAILED", format!("{error:#}")))?;
                Ok(self.observation())
            }
        }
    }

    fn observation(&self) -> Value {
        let mods: Vec<_> = self.world.list_mods().into_iter().filter_map(|info| {
            let source = self.world.read_mod(&info.name).ok()?;
            Some(json!({"name":info.name,"status":info.status,"source":source.source,"candidate":source.candidate}))
        }).collect();
        json!({"world":self.world.snapshot(),"logs":self.world.recent_logs(None, MAX_ITEMS),"errors":self.world.recent_errors(MAX_ITEMS),"mods":mods})
    }

    fn write_mod(&mut self, name: &str, source: &str) -> Reply {
        let name = validated_name(name)?;
        let path = self.mods_dir.join(format!("{name}.rhai"));
        self.ensure_mod_path(&path)?;
        let old = optional_source(&path).map_err(mod_io)?;
        let known = self.world.read_mod(name).ok();
        if known.is_none() && self.world.list_mods().len() >= MAX_MOD_FILES {
            return Err(ToolError::new(
                "MOD_LIMIT",
                "Save has reached its mod limit",
            ));
        }
        if old.as_deref() != Some(source) {
            persist::write_bytes(&path, source.as_bytes()).map_err(mod_io)?;
        }
        let previous = self.world.export();
        let already_active = known.as_ref().and_then(|current| current.source.as_deref())
            == Some(source)
            && self
                .world
                .list_mods()
                .iter()
                .any(|info| info.name == name && info.active);
        let result = self
            .world
            .write_mod(name, source)
            .map(|info| mod_result(info, self.world.tick(), !already_active))
            .map_err(|error| {
                let mut failure = ToolError::new("MOD_REJECTED", format!("{error:#}"));
                failure.diagnostic = self
                    .world
                    .recent_errors(MAX_ITEMS)
                    .into_iter()
                    .find(|entry| entry.mod_name == name)
                    .map(Box::new);
                failure
            });
        self.commit_mod_change(previous, &path, old.as_deref())?;
        self.record(
            self.world.tick(),
            WorldAction::WriteMod {
                name: name.into(),
                source: source.into(),
                accepted: result.is_ok(),
            },
        );
        result
    }

    fn delete_mod(&mut self, name: &str) -> Reply {
        let name = validated_name(name)?;
        let path = self.mods_dir.join(format!("{name}.rhai"));
        self.ensure_mod_path(&path)?;
        let old = optional_source(&path).map_err(mod_io)?;
        let previous = self.world.export();
        let deleted = self.world.delete_mod(name).map_err(mod_io)?;
        if old.is_some() {
            if let Err(error) = fs::remove_file(&path) {
                self.restore_world(previous);
                return Err(mod_io(error));
            }
        }
        self.commit_mod_change(previous, &path, old.as_deref())?;
        if deleted {
            self.record(
                self.world.tick(),
                WorldAction::DeleteMod { name: name.into() },
            );
        }
        Ok(json!({"deleted":deleted || old.is_some(),"changed":deleted,"tick":self.world.tick()}))
    }

    fn restore_mod(&mut self, name: &str) -> Reply {
        let name = validated_name(name)?;
        let path = self.mods_dir.join(format!("{name}.rhai"));
        self.ensure_mod_path(&path)?;
        let old = optional_source(&path).map_err(mod_io)?;
        let previous = self.world.export();
        let info = match self.world.restore_mod(name) {
            Ok(info) => info,
            Err(error) => {
                self.commit_mod_change(previous, &path, old.as_deref())?;
                self.record(
                    self.world.tick(),
                    WorldAction::RestoreMod {
                        name: name.into(),
                        accepted: false,
                    },
                );
                return Err(ToolError::new("RESTORE_REJECTED", error));
            }
        };
        let working = self.world.read_mod(name).map_err(mod_io)?.source;
        if let Some(source) = working {
            if let Err(error) = persist::write_bytes(&path, source.as_bytes()) {
                self.restore_world(previous);
                return Err(mod_io(error));
            }
        }
        self.commit_mod_change(previous, &path, old.as_deref())?;
        self.record(
            self.world.tick(),
            WorldAction::RestoreMod {
                name: name.into(),
                accepted: true,
            },
        );
        Ok(mod_result(info, self.world.tick(), true))
    }

    fn commit_mod_change(
        &mut self,
        previous: WorldState,
        path: &Path,
        old: Option<&str>,
    ) -> std::result::Result<(), ToolError> {
        if let Err(error) = self.save() {
            self.restore_world(previous);
            let rollback = match old {
                Some(source) => persist::write_bytes(path, source.as_bytes()),
                None => match fs::remove_file(path) {
                    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
                    _ => Ok(()),
                },
            };
            if let Err(rollback_error) = rollback {
                tracing::error!(%rollback_error, "could not restore staged mod file");
                return Err(mod_io(format!("{error:#}. The editable mod file also could not be restored: {rollback_error:#}")));
            }
            return Err(mod_io(format!("{error:#}")));
        }
        Ok(())
    }

    fn restore_world(&mut self, previous: WorldState) {
        match World::restore(previous) {
            Ok(world) => self.world = world,
            Err(error) => {
                self.faulted = true;
                tracing::error!(%error, "could not restore world after a failed save");
                self.notice(format!(
                    "Simulation ticks paused because a save could not be rolled back: {error:#}"
                ));
            }
        }
    }

    fn reload(&mut self) -> Result<Value> {
        check_directory(&self.mods_dir)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.mods_dir)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|suffix| suffix == "rhai")
            {
                if entries.len() == MAX_MOD_FILES {
                    bail!("mods directory contains more than {MAX_MOD_FILES} .rhai files");
                }
                entries.push(entry.file_name());
            }
        }
        entries.sort();
        let mut results = serde_json::Map::new();
        for entry in entries {
            let display = entry.to_string_lossy();
            let attempt = (|| {
                let name = validated_name(&display)?;
                let path = self.mods_dir.join(&entry);
                if path != self.mods_dir.join(format!("{name}.rhai")) {
                    return Err(ToolError::new(
                        "INVALID_MOD_NAME",
                        "Mod filenames must use their canonical form",
                    ));
                }
                self.ensure_mod_path(&path)?;
                let source = optional_source(&path)
                    .map_err(mod_io)?
                    .ok_or_else(|| mod_io("mod disappeared before reload"))?;
                if let Ok(current) = self.world.read_mod(name) {
                    if current.candidate.as_deref() == Some(source.as_str()) {
                        return Ok(json!({"status":"retained_rejection"}));
                    }
                    if current.source.as_deref() == Some(source.as_str()) {
                        return Ok(json!({"status":"unchanged"}));
                    }
                }
                self.write_mod(name, &source)
            })();
            let result = match attempt {
                Ok(data) => data,
                Err(error) => {
                    if error.diagnostic.is_none() {
                        self.world.record_diagnostic(diagnostic(
                            self.world.tick(),
                            &display,
                            "mod_io",
                            error.message.clone(),
                            None,
                        ));
                    }
                    json!({"status":"rejected","error":error})
                }
            };
            results.insert(display.into_owned(), result);
        }
        Ok(Value::Object(results))
    }

    fn ensure_mod_path(&self, path: &Path) -> std::result::Result<(), ToolError> {
        check_directory(&self.mods_dir).map_err(mod_io)?;
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() && !is_link(&meta) => Ok(()),
            Ok(_) => Err(mod_io("mod path must be a regular file")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(mod_io(error)),
        }
    }

    fn save(&self) -> Result<()> {
        persist::write_json(&self.run_dir.join("world.json"), &self.world.export())?;
        tracing::debug!(tick = self.world.tick(), "saved world snapshot");
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<()> {
        self.save()?;
        self.record(self.world.tick(), WorldAction::Checkpoint);
        Ok(())
    }

    fn record(&mut self, tick: u64, action: WorldAction) {
        if let Some(recorder) = &self.recorder {
            if let Err(error) = recorder.append(tick, action) {
                self.recorder = None;
                tracing::error!(%error, "world replay journal is incomplete");
                self.notice(format!("World replay recording stopped: {error:#}"));
            }
        }
    }

    fn errors_with_keys(&self) -> Vec<(String, Diagnostic)> {
        self.world
            .recent_errors(MAX_ITEMS)
            .into_iter()
            .filter_map(|entry| Some((serde_json::to_string(&entry).ok()?, entry)))
            .collect()
    }

    fn update_error_journal(&mut self) {
        let entries = self.errors_with_keys();
        let mut seen = HashSet::with_capacity(entries.len());
        for (key, entry) in entries {
            if !self.journal_seen.contains(&key) && !seen.contains(&key) {
                if let Err(error) =
                    persist::append_json_line(&self.run_dir.join("errors.jsonl"), &entry)
                {
                    tracing::warn!(%error, "could not append error journal");
                }
                if entry.disabled && !entry.resolved {
                    self.notice(format!(
                        "Mod {} disabled at tick {}. {}",
                        entry.mod_name, entry.tick, entry.message
                    ));
                }
            }
            seen.insert(key);
        }
        self.journal_seen = seen;
    }

    fn notice(&self, text: String) {
        let _ = self.notices.send(text);
    }
}

fn mod_io(error: impl ToString) -> ToolError {
    ToolError::new("MOD_IO_ERROR", error)
}

fn mod_result(info: ModInfo, tick: u64, changed: bool) -> Value {
    let mut result = json!(info);
    result["tick"] = json!(tick);
    result["changed"] = json!(changed);
    result
}

fn validated_name(name: &str) -> std::result::Result<&str, ToolError> {
    validate_mod_name(name).map_err(|error| ToolError::new("INVALID_MOD_NAME", error))
}

fn optional_source(path: &Path) -> Result<Option<String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut contents = String::new();
    file.take(MAX_MOD_SOURCE_BYTES as u64 + 1)
        .read_to_string(&mut contents)?;
    if contents.len() > MAX_MOD_SOURCE_BYTES {
        bail!("mod source exceeds {MAX_MOD_SOURCE_BYTES} bytes");
    }
    Ok(Some(contents))
}

fn check_directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || is_link(&meta) {
        bail!("{} must be a regular directory", path.display());
    }
    Ok(())
}

fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn diagnostic(
    tick: u64,
    name: &str,
    category: &str,
    message: String,
    repro: Option<String>,
) -> Diagnostic {
    Diagnostic {
        mod_name: name.into(),
        line: None,
        message,
        tick,
        repro_snippet: repro,
        severity: "error".into(),
        category: category.into(),
        callback: None,
        failure_count: 0,
        disabled: false,
        resolved: false,
    }
}
