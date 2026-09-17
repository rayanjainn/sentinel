//! `sysinfo`-backed process table shared by every OS, with per-OS extras (threads, handles, nice).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
    Uid, UpdateKind, Users,
};

use crate::error::{CoreResult, SentinelError};
use crate::model::{OpenFile, Pid, ProcessInfo, ProcessSnapshot, ProcessStatus};
use crate::provider::ProcessProvider;
use crate::util::now_ms;

/// Values `sysinfo` does not expose, filled in per OS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProcExtra {
    pub thread_count: Option<u32>,
    pub fd_count: Option<u32>,
    pub nice: Option<i32>,
}

pub(crate) trait ProcessExtras: Send {
    /// Called once before a full refresh (e.g. to take a Toolhelp snapshot).
    fn begin_refresh(&mut self) {}
    fn extra(&mut self, pid: Pid) -> ProcExtra;
    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>>;
}

/// Smoothing factor for a ~10 sample time constant.
const EMA_ALPHA: f32 = 0.095;
const USER_LIST_RETRY: Duration = Duration::from_secs(30);
/// Thread, handle and priority details change slowly and cost a syscall or two per process, so
/// each process refreshes them at most this often (new processes immediately).
const EXTRA_MAX_AGE: Duration = Duration::from_secs(3);

pub(crate) struct ProcTable<E: ProcessExtras> {
    system: System,
    users: Users,
    users_refreshed: Instant,
    extras: E,
    /// pid → (start_time, smoothed cpu)
    ema: HashMap<Pid, (u64, f32)>,
    /// pid → (start_time, fetched at, details)
    extra_cache: HashMap<Pid, (u64, Instant, ProcExtra)>,
    logical_cores: u32,
    total_memory: u64,
}

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_user(UpdateKind::OnlyIfNotSet)
        .without_tasks()
}

impl<E: ProcessExtras> ProcTable<E> {
    pub fn new(extras: E) -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        let logical_cores = system.cpus().len().max(1) as u32;
        let total_memory = system.total_memory();
        // Prime CPU counters so the first published snapshot has real percentages.
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        Self {
            system,
            users: Users::new_with_refreshed_list(),
            users_refreshed: Instant::now(),
            extras,
            ema: HashMap::new(),
            extra_cache: HashMap::new(),
            logical_cores,
            total_memory,
        }
    }

    fn user_name(&mut self, uid: Option<Uid>) -> Option<String> {
        let uid = uid?;
        if let Some(user) = self.users.get_user_by_id(&uid) {
            return Some(user.name().to_owned());
        }
        if self.users_refreshed.elapsed() >= USER_LIST_RETRY {
            self.users.refresh();
            self.users_refreshed = Instant::now();
            return self
                .users
                .get_user_by_id(&uid)
                .map(|user| user.name().to_owned());
        }
        None
    }

    fn cached_extra(&mut self, pid: Pid, start_time: u64, fresh: bool) -> ProcExtra {
        if !fresh
            && let Some((cached_start, at, extra)) = self.extra_cache.get(&pid)
            && *cached_start == start_time
            && at.elapsed() < EXTRA_MAX_AGE
        {
            return *extra;
        }
        let extra = self.extras.extra(pid);
        self.extra_cache
            .insert(pid, (start_time, Instant::now(), extra));
        extra
    }

    fn build_info(&mut self, pid: sysinfo::Pid) -> Option<ProcessInfo> {
        let process = self.system.process(pid)?;
        if !process.exists() {
            return None;
        }
        let pid_u32 = pid.as_u32();
        let start_time = process.start_time();
        let cpu = sanitize_cpu(process.cpu_usage());
        let name = process.name().to_string_lossy().into_owned();
        let cmd = process
            .cmd()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let exe = process.exe().map(|p| p.to_string_lossy().into_owned());
        let ppid = process.parent().map(|p| p.as_u32());
        let status = map_status(process.status());
        let memory_rss = process.memory();
        let memory_virtual = process.virtual_memory();
        let run_time_secs = process.run_time();
        let uid = process.user_id().cloned();
        let user = self.user_name(uid);

        let avg = match self.ema.get(&pid_u32) {
            Some((st, prev)) if *st == start_time => prev + EMA_ALPHA * (cpu - prev),
            _ => cpu,
        };
        self.ema.insert(pid_u32, (start_time, avg));
        let extra = self.cached_extra(pid_u32, start_time, false);

        Some(ProcessInfo {
            pid: pid_u32,
            ppid,
            name,
            cmd,
            exe,
            user,
            status,
            cpu_percent: cpu,
            cpu_percent_avg: avg,
            memory_rss,
            memory_virtual,
            start_time,
            run_time_secs,
            thread_count: extra.thread_count,
            fd_count: extra.fd_count,
            nice: extra.nice,
        })
    }
}

impl<E: ProcessExtras> ProcessProvider for ProcTable<E> {
    fn refresh(&mut self) -> CoreResult<ProcessSnapshot> {
        self.system
            .refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        self.extras.begin_refresh();
        let pids: Vec<sysinfo::Pid> = self.system.processes().keys().copied().collect();
        let mut processes = Vec::with_capacity(pids.len());
        for pid in pids {
            if let Some(info) = self.build_info(pid) {
                processes.push(info);
            }
        }
        let live: std::collections::HashSet<Pid> = processes.iter().map(|p| p.pid).collect();
        self.ema.retain(|pid, _| live.contains(pid));
        self.extra_cache.retain(|pid, _| live.contains(pid));
        processes.sort_unstable_by_key(|p| p.pid);
        Ok(ProcessSnapshot {
            ts_ms: now_ms(),
            processes,
            logical_cores: self.logical_cores,
            total_memory: self.total_memory,
        })
    }

    fn lookup(&mut self, pid: Pid) -> CoreResult<ProcessInfo> {
        let spid = sysinfo::Pid::from_u32(pid);
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[spid]),
            true,
            refresh_kind(),
        );
        self.extra_cache.remove(&pid);
        self.build_info(spid)
            .ok_or(SentinelError::ProcessNotFound { pid })
    }

    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>> {
        self.extras.open_files(pid)
    }
}

fn sanitize_cpu(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

fn map_status(status: sysinfo::ProcessStatus) -> ProcessStatus {
    use sysinfo::ProcessStatus as S;
    match status {
        S::Run => ProcessStatus::Running,
        S::Sleep => ProcessStatus::Sleeping,
        S::Idle => ProcessStatus::Idle,
        S::Stop | S::Tracing | S::Suspended => ProcessStatus::Stopped,
        S::Zombie => ProcessStatus::Zombie,
        S::Dead => ProcessStatus::Dead,
        S::UninterruptibleDiskSleep | S::Waking | S::Wakekill | S::Parked | S::LockBlocked => {
            ProcessStatus::Waiting
        }
        _ => ProcessStatus::Unknown,
    }
}
