use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub(crate) const MAX_ERRORS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub mod_name: String,
    pub line: Option<usize>,
    pub message: String,
    pub tick: u64,
    pub repro_snippet: Option<String>,
    pub severity: String,
    pub category: String,
    pub callback: Option<String>,
    pub failure_count: u32,
    pub disabled: bool,
    pub resolved: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ErrorBus {
    entries: VecDeque<Diagnostic>,
}

impl ErrorBus {
    pub fn from_entries(entries: VecDeque<Diagnostic>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &VecDeque<Diagnostic> {
        &self.entries
    }

    pub fn record(&mut self, diagnostic: Diagnostic) {
        if self.entries.len() == MAX_ERRORS {
            self.entries.pop_front();
        }
        self.entries.push_back(diagnostic);
    }

    pub fn recent(&self, count: usize) -> Vec<Diagnostic> {
        self.entries.iter().rev().take(count).cloned().collect()
    }

    pub fn resolve(&mut self, mod_name: &str, callback: Option<&str>) {
        for diagnostic in &mut self.entries {
            if diagnostic.mod_name == mod_name && diagnostic.callback.as_deref() == callback {
                diagnostic.resolved = true;
            }
        }
    }

    pub fn resolve_removed_callbacks(
        &mut self,
        mod_name: &str,
        callback_exists: impl Fn(&str) -> bool,
    ) {
        for diagnostic in &mut self.entries {
            if diagnostic.mod_name == mod_name
                && diagnostic
                    .callback
                    .as_deref()
                    .is_none_or(|callback| !callback_exists(callback))
            {
                diagnostic.resolved = true;
            }
        }
    }
}
