//! Cheap PID → name map for labelling sockets when the full process table is not being sampled.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::model::Pid;

const REFRESH_EVERY: Duration = Duration::from_secs(2);

pub struct ProcessNames {
    system: System,
    refreshed: Option<Instant>,
}

impl Default for ProcessNames {
    fn default() -> Self {
        Self {
            system: System::new(),
            refreshed: None,
        }
    }
}

impl ProcessNames {
    pub fn names(&mut self, wanted: impl IntoIterator<Item = Pid>) -> HashMap<Pid, String> {
        let wanted: Vec<Pid> = wanted.into_iter().collect();
        let missing = wanted
            .iter()
            .any(|pid| self.system.process(sysinfo::Pid::from_u32(*pid)).is_none());
        let stale = self
            .refreshed
            .is_none_or(|at| at.elapsed() >= REFRESH_EVERY);
        if stale
            || (missing
                && self
                    .refreshed
                    .is_some_and(|at| at.elapsed() >= Duration::from_millis(500)))
        {
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().without_tasks(),
            );
            self.refreshed = Some(Instant::now());
        }
        wanted
            .into_iter()
            .filter_map(|pid| {
                self.system
                    .process(sysinfo::Pid::from_u32(pid))
                    .map(|p| (pid, p.name().to_string_lossy().into_owned()))
            })
            .collect()
    }
}
