use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crate::persist;

pub(crate) const MAX_HISTORY: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskKind {
    Player,
    Repair,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskState {
    Queued,
    Implementing,
    Testing,
    Repairing,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Task {
    pub id: u64,
    pub kind: TaskKind,
    pub instruction: String,
    pub state: TaskState,
    pub target_mod: Option<String>,
    pub error_key: Option<String>,
    pub last_action: Option<String>,
    pub mutation_tick: Option<u64>,
    pub tested_ticks: u64,
    pub iterations: u32,
    pub repair_attempts: u32,
    pub read_world: bool,
    pub changed_mods: Vec<String>,
    pub attempted: Vec<u64>,
    pub results: VecDeque<Value>,
}

impl Task {
    pub fn new(id: u64, kind: TaskKind, instruction: String) -> Self {
        Self {
            id,
            kind,
            instruction,
            state: TaskState::Queued,
            target_mod: None,
            error_key: None,
            last_action: None,
            mutation_tick: None,
            tested_ticks: 0,
            iterations: 0,
            repair_attempts: 0,
            read_world: false,
            changed_mods: Vec::new(),
            attempted: Vec::new(),
            results: VecDeque::new(),
        }
    }

    pub fn model_view(&self) -> Value {
        json!({"id":self.id,"kind":self.kind,"instruction":self.instruction,"state":self.state,
            "target_mod":self.target_mod,"last_action":self.last_action,"mutation_tick":self.mutation_tick,
            "tested_ticks":self.tested_ticks,"iterations":self.iterations,
            "repair_attempts":self.repair_attempts,"changed_mods":self.changed_mods})
    }

    pub fn remember_result(&mut self, result: Value) {
        if self.results.len() == 8 {
            self.results.pop_front();
        }
        self.results.push_back(result);
    }

    pub fn finished(&self) -> bool {
        matches!(self.state, TaskState::Complete | TaskState::Failed)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct JournalEntry {
    tick: Option<u64>,
    task_id: Option<u64>,
    action: String,
    detail: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AgentState {
    version: u32,
    pub next_task_id: u64,
    pub user_paused: bool,
    pub tasks: VecDeque<Task>,
    pub recent: VecDeque<JournalEntry>,
    pub seen_errors: VecDeque<String>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            version: 1,
            next_task_id: 1,
            user_paused: false,
            tasks: VecDeque::new(),
            recent: VecDeque::new(),
            seen_errors: VecDeque::new(),
        }
    }
}

impl AgentState {
    pub fn pending(&self) -> usize {
        self.tasks.iter().filter(|task| !task.finished()).count()
    }

    pub fn prune(&mut self) {
        while self.tasks.iter().filter(|task| task.finished()).count() > MAX_HISTORY {
            let Some(index) = self.tasks.iter().position(Task::finished) else {
                break;
            };
            self.tasks.remove(index);
        }
        while self.seen_errors.len() > MAX_HISTORY {
            self.seen_errors.pop_front();
        }
    }
}

pub(crate) struct Journal {
    state_path: PathBuf,
    journal_path: PathBuf,
}

impl Journal {
    pub fn open(run_dir: &Path) -> Result<(Self, AgentState)> {
        let directory = run_dir.join("agent");
        let journal = Self {
            state_path: directory.join("state.json"),
            journal_path: directory.join("journal.jsonl"),
        };
        let state: Option<AgentState> = persist::read_json(&journal.state_path)?;
        if state.is_none() {
            let ledger: Option<Value> = persist::read_json(&run_dir.join("budget.json"))?;
            ensure!(ledger.as_ref().and_then(|ledger| ledger["requests"].as_u64()).unwrap_or(0) == 0,
                "agent/state.json is missing from a save with recorded LLM requests. Restore the agent state before restarting.");
        }
        let state = state.unwrap_or_default();
        ensure!(
            state.version == 1,
            "unsupported agent schema version {}",
            state.version
        );
        ensure!(
            state.next_task_id > state.tasks.iter().map(|task| task.id).max().unwrap_or(0),
            "invalid persisted agent task sequence"
        );
        journal.save(&state)?;
        Ok((journal, state))
    }

    pub fn save(&self, state: &AgentState) -> Result<()> {
        persist::write_json(&self.state_path, state)
    }

    pub fn record(
        &self,
        state: &mut AgentState,
        tick: Option<u64>,
        task_id: Option<u64>,
        action: &str,
        detail: Value,
        recent_detail: Value,
    ) -> Result<()> {
        let mut entry = JournalEntry {
            tick,
            task_id,
            action: action.to_owned(),
            detail,
        };
        persist::append_json_line(&self.journal_path, &entry)?;
        entry.detail = recent_detail;
        if state.recent.len() == MAX_HISTORY {
            state.recent.pop_front();
        }
        state.recent.push_back(entry);
        Ok(())
    }
}
