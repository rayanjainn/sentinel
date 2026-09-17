//! Ring buffers backing `get_resource_history` and the per-process 60 s graphs.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{Pid, ProcessHistoryPoint, ProcessSnapshot, ResourceSample, TimestampMs};

pub const RESOURCE_RETENTION_SECS: u32 = 900;
pub const PROCESS_RETENTION_SECS: u64 = 60;
/// Hard cap independent of timestamps (500 ms interval × 15 min, plus slack for clock jumps).
const RESOURCE_CAPACITY: usize = 1900;
const PROCESS_POINTS_CAPACITY: usize = 130;

#[derive(Debug, Default)]
pub struct ResourceHistory {
    samples: VecDeque<ResourceSample>,
}

impl ResourceHistory {
    pub fn push(&mut self, sample: ResourceSample) {
        let cutoff = sample
            .ts_ms
            .saturating_sub(u64::from(RESOURCE_RETENTION_SECS) * 1000);
        self.samples.push_back(sample);
        while self
            .samples
            .front()
            .is_some_and(|front| front.ts_ms < cutoff)
            || self.samples.len() > RESOURCE_CAPACITY
        {
            self.samples.pop_front();
        }
    }

    pub fn window(&self, window_secs: u32) -> Vec<ResourceSample> {
        let Some(newest) = self.samples.back() else {
            return Vec::new();
        };
        let window_ms = u64::from(window_secs.min(RESOURCE_RETENTION_SECS)) * 1000;
        let cutoff = newest.ts_ms.saturating_sub(window_ms);
        let start = self.samples.partition_point(|s| s.ts_ms < cutoff);
        self.samples.range(start..).cloned().collect()
    }

    pub fn latest(&self) -> Option<&ResourceSample> {
        self.samples.back()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct ProcessHistory {
    per_pid: HashMap<Pid, (u64, VecDeque<ProcessHistoryPoint>)>,
}

impl ProcessHistory {
    /// Appends one point per live process and forgets processes that exited or whose PID was
    /// reused.
    pub fn record(&mut self, snapshot: &ProcessSnapshot) {
        let cutoff: TimestampMs = snapshot.ts_ms.saturating_sub(PROCESS_RETENTION_SECS * 1000);
        let mut live = HashSet::with_capacity(snapshot.processes.len());
        for process in &snapshot.processes {
            live.insert(process.pid);
            let entry = self
                .per_pid
                .entry(process.pid)
                .or_insert_with(|| (process.start_time, VecDeque::new()));
            if entry.0 != process.start_time {
                *entry = (process.start_time, VecDeque::new());
            }
            let points = &mut entry.1;
            points.push_back(ProcessHistoryPoint {
                ts_ms: snapshot.ts_ms,
                cpu_percent: process.cpu_percent,
                memory_rss: process.memory_rss,
            });
            while points.front().is_some_and(|p| p.ts_ms < cutoff)
                || points.len() > PROCESS_POINTS_CAPACITY
            {
                points.pop_front();
            }
        }
        self.per_pid.retain(|pid, _| live.contains(pid));
    }

    pub fn points(&self, pid: Pid) -> Vec<ProcessHistoryPoint> {
        self.per_pid
            .get(&pid)
            .map(|(_, points)| points.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn tracked(&self) -> usize {
        self.per_pid.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MemoryBreakdown, NetThroughput, ProcessInfo, ProcessStatus};

    fn sample(ts_ms: u64) -> ResourceSample {
        ResourceSample {
            ts_ms,
            cpu_total: 1.0,
            per_core: vec![1.0],
            memory: MemoryBreakdown {
                total: 1,
                used: 1,
                available: 0,
                free: 0,
                cached: None,
                compressed: None,
                wired: None,
                swap_total: 0,
                swap_used: 0,
            },
            load: None,
            thermal: None,
            network: NetThroughput::default(),
        }
    }

    fn process(pid: Pid, start_time: u64) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid: None,
            name: "p".into(),
            cmd: vec![],
            exe: None,
            user: None,
            status: ProcessStatus::Running,
            cpu_percent: 5.0,
            cpu_percent_avg: 5.0,
            memory_rss: 10,
            memory_virtual: 20,
            start_time,
            run_time_secs: 1,
            thread_count: None,
            fd_count: None,
            nice: None,
            summary: crate::model::ProcessSummary::default(),
        }
    }

    #[test]
    fn resource_history_keeps_fifteen_minutes() {
        let mut history = ResourceHistory::default();
        for i in 0..1200u64 {
            history.push(sample(i * 1000));
        }
        let all = history.window(10_000);
        assert_eq!(all.len(), 901);
        assert_eq!(all.first().map(|s| s.ts_ms), Some(299_000));
        assert_eq!(history.window(10).len(), 11);
    }

    #[test]
    fn process_history_prunes_dead_and_reused_pids() {
        let mut history = ProcessHistory::default();
        let snap = |ts_ms, processes| ProcessSnapshot {
            ts_ms,
            processes,
            logical_cores: 1,
            total_memory: 1,
        };
        for i in 0..70u64 {
            history.record(&snap(i * 1000, vec![process(10, 1), process(11, 1)]));
        }
        assert_eq!(history.points(10).len(), 61);
        history.record(&snap(70_000, vec![process(10, 2)]));
        assert_eq!(history.points(10).len(), 1, "reused pid starts fresh");
        assert!(history.points(11).is_empty(), "dead pid pruned");
        assert_eq!(history.tracked(), 1);
    }
}
