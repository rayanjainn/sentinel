//! Test doubles for the agent integration tests. Product code never uses these.
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use sentinel_agent::backend::{
    AgentBackend, ChatRequest, ChatResponse, ContentBlock, StopReason, StreamDelta, Usage,
};
use sentinel_agent::events::{AgentEvent, AgentEventSink, AgentStreamPayload};
use sentinel_agent::settings::{ModelInfo, ProviderId};
use sentinel_core::action::*;
use sentinel_core::model::*;
use sentinel_core::service::{FileFilter, SystemQueries};
use sentinel_core::{CoreResult, SentinelError};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

// ---------------------------------------------------------------------------------------------
// System state

pub fn process(pid: Pid, name: &str, cpu: f32, start_time: TimestampSecs) -> ProcessInfo {
    ProcessInfo {
        pid,
        ppid: Some(1),
        name: name.to_owned(),
        cmd: vec![format!("/usr/bin/{name}")],
        exe: Some(format!("/usr/bin/{name}")),
        user: Some("tester".into()),
        status: ProcessStatus::Running,
        cpu_percent: cpu,
        cpu_percent_avg: cpu,
        memory_rss: 512 * 1024 * 1024,
        memory_virtual: 4 * 1024 * 1024 * 1024,
        start_time,
        run_time_secs: 3600,
        thread_count: Some(8),
        fd_count: Some(20),
        nice: Some(0),
        summary: ProcessSummary::default(),
    }
}

fn unavailable<T>(what: &str) -> CoreResult<T> {
    Err(SentinelError::Unavailable {
        feature: what.into(),
        reason: "not simulated in tests".into(),
    })
}

#[derive(Default)]
pub struct FakeQueries {
    pub processes: Mutex<Vec<ProcessInfo>>,
    pub sockets: Mutex<Vec<SocketEntry>>,
    pub files: Mutex<Vec<FileEntry>>,
    pub scans_started: AtomicU64,
}

impl FakeQueries {
    pub fn with_processes(processes: Vec<ProcessInfo>) -> Arc<Self> {
        Arc::new(Self {
            processes: Mutex::new(processes),
            ..Default::default()
        })
    }

    pub fn kill(&self, pid: Pid) {
        self.processes.lock().unwrap().retain(|p| p.pid != pid);
    }
}

impl SystemQueries for FakeQueries {
    fn system_info(&self) -> SystemInfo {
        SystemInfo {
            platform: Platform::Macos,
            hostname: Some("test-host".into()),
            os_name: "macOS".into(),
            os_version: Some("26.0".into()),
            kernel_version: None,
            arch: "aarch64".into(),
            cpu_brand: "Test CPU".into(),
            physical_cores: Some(8),
            logical_cores: 8,
            total_memory: 16_000_000_000,
            total_swap: 0,
            boot_time: 0,
        }
    }

    fn permission_status(&self) -> PermissionStatus {
        PermissionStatus {
            platform: Platform::Macos,
            full_disk_access: PermissionState::Granted,
            running_elevated: false,
            can_request_elevation: true,
            firewall: self.firewall_status(),
        }
    }

    fn resource_sample(&self) -> CoreResult<ResourceSample> {
        Ok(ResourceSample {
            ts_ms: now_ms(),
            cpu_total: 42.0,
            per_core: vec![40.0; 8],
            memory: MemoryBreakdown {
                total: 16_000_000_000,
                used: 11_000_000_000,
                available: 5_000_000_000,
                free: 1_000_000_000,
                cached: Some(3_000_000_000),
                compressed: None,
                wired: None,
                swap_total: 0,
                swap_used: 0,
            },
            load: None,
            thermal: None,
            network: NetThroughput::default(),
        })
    }

    fn resource_history(&self, _window_secs: u32) -> Vec<ResourceSample> {
        Vec::new()
    }

    fn process_snapshot(&self) -> CoreResult<ProcessSnapshot> {
        Ok(ProcessSnapshot {
            ts_ms: now_ms(),
            processes: self.processes.lock().unwrap().clone(),
            logical_cores: 8,
            total_memory: 16_000_000_000,
        })
    }

    fn process_detail(&self, pid: Pid) -> CoreResult<ProcessDetail> {
        let info = self
            .processes
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.pid == pid)
            .cloned()
            .ok_or(SentinelError::ProcessNotFound { pid })?;
        Ok(ProcessDetail {
            info,
            open_files: Vec::new(),
            open_files_error: None,
            connections: Vec::new(),
            has_window: false,
            explanation: ProcessExplanation::default(),
        })
    }

    fn process_history(&self, _pid: Pid) -> Vec<ProcessHistoryPoint> {
        Vec::new()
    }

    fn network_snapshot(&self) -> CoreResult<NetworkSnapshot> {
        Ok(NetworkSnapshot {
            ts_ms: now_ms(),
            sockets: self.sockets.lock().unwrap().clone(),
            throughput: NetThroughput::default(),
            traffic_source: TrafficSource::InterfaceOnly,
        })
    }

    fn firewall_status(&self) -> FirewallStatus {
        FirewallStatus {
            backend: Some(FirewallBackend::Pf),
            available: true,
            firewall_enabled: Some(true),
            requires_elevation: true,
            note: None,
        }
    }

    fn firewall_rules(&self) -> CoreResult<Vec<FirewallRule>> {
        Ok(Vec::new())
    }

    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        Ok(vec![VolumeInfo {
            mount_point: "/".into(),
            name: "Macintosh HD".into(),
            file_system: "apfs".into(),
            total_bytes: 500_000_000_000,
            available_bytes: 120_000_000_000,
            is_removable: false,
            is_system: true,
        }])
    }

    fn start_scan(&self, request: ScanRequest) -> CoreResult<ScanId> {
        self.scans_started.fetch_add(1, Ordering::SeqCst);
        Ok(format!("scan:{}", request.root))
    }

    fn cancel_scan(&self, _scan_id: &str) -> CoreResult<()> {
        Ok(())
    }

    fn wait_for_scan(&self, scan_id: &str, _timeout: Duration) -> CoreResult<ScanSummary> {
        self.scan_summary(scan_id)
    }

    fn scan_covering(&self, _path: &str) -> Option<ScanId> {
        None
    }

    fn scan_summary(&self, scan_id: &str) -> CoreResult<ScanSummary> {
        let files = self.files.lock().unwrap().clone();
        Ok(ScanSummary {
            scan_id: scan_id.to_owned(),
            root: scan_id.trim_start_matches("scan:").to_owned(),
            started_at_ms: now_ms(),
            duration_ms: 10,
            total_bytes: files.iter().map(|f| f.size_bytes).sum(),
            file_count: files.len() as u64,
            dir_count: 1,
            unreadable_entries: 0,
            unreadable_samples: Vec::new(),
            by_extension: Vec::new(),
            largest_files: files,
            suggestions: Vec::new(),
        })
    }

    fn scan_tree(&self, _query: &TreeQuery) -> CoreResult<TreeNode> {
        unavailable("scan tree")
    }

    fn find_files(&self, _scan_id: &str, filter: &FileFilter) -> CoreResult<Vec<FileEntry>> {
        let now = now_ms() / 1000;
        Ok(self
            .files
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.size_bytes >= filter.min_size_bytes)
            .filter(|f| {
                filter.unused_for_days.is_none_or(|days| {
                    f.modified
                        .max(f.accessed)
                        .is_some_and(|t| now.saturating_sub(t) >= u64::from(days) * 86_400)
                })
            })
            .take(filter.limit as usize)
            .cloned()
            .collect())
    }

    fn start_duplicate_scan(&self, _scan_id: &str, _min_size_bytes: u64) -> CoreResult<JobId> {
        unavailable("duplicates")
    }

    fn cancel_duplicate_scan(&self, _job_id: &str) -> CoreResult<()> {
        Ok(())
    }

    fn wait_for_duplicates(
        &self,
        _job_id: &str,
        _timeout: Duration,
    ) -> CoreResult<DuplicateReport> {
        unavailable("duplicates")
    }
}

// ---------------------------------------------------------------------------------------------
// Action pipeline: one object implementing both traits, like the real `ActionService`.

#[derive(Debug, Clone)]
struct IssuedToken {
    action: Action,
    origin: Origin,
    expires_at_ms: u64,
    used: bool,
}

pub struct FakeActions {
    tokens: Mutex<HashMap<String, IssuedToken>>,
    pub prepared: Mutex<Vec<(Action, Origin)>>,
    pub committed: Mutex<Vec<Action>>,
    pub rejected: Mutex<Vec<String>>,
    /// Tokens are issued already expired when set, to exercise expiry paths.
    pub issue_expired: Mutex<bool>,
    counter: AtomicU64,
}

impl FakeActions {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tokens: Mutex::new(HashMap::new()),
            prepared: Mutex::new(Vec::new()),
            committed: Mutex::new(Vec::new()),
            rejected: Mutex::new(Vec::new()),
            issue_expired: Mutex::new(false),
            counter: AtomicU64::new(0),
        })
    }

    pub fn commit_count(&self) -> usize {
        self.committed.lock().unwrap().len()
    }

    pub fn prepare_count(&self) -> usize {
        self.prepared.lock().unwrap().len()
    }
}

fn title_of(action: &Action) -> String {
    match action {
        Action::TerminateProcess { target } => format!("Quit PID {}", target.pid),
        Action::ForceKillProcess { target } => format!("Force kill PID {}", target.pid),
        Action::SetProcessPriority { target, nice } => {
            format!("Set PID {} nice {nice}", target.pid)
        }
        Action::TrashPaths { paths } => format!("Move {} items to Trash", paths.len()),
        Action::MovePaths { paths, .. } => format!("Move {} items", paths.len()),
        Action::AddFirewallRule { .. } => "Add firewall rule".into(),
        Action::RemoveFirewallRule { .. } => "Remove firewall rule".into(),
    }
}

impl ActionPreparer for FakeActions {
    fn prepare(&self, action: Action, origin: Origin) -> CoreResult<ActionPreview> {
        self.prepared
            .lock()
            .unwrap()
            .push((action.clone(), origin.clone()));
        let token = format!("tok-{}", self.counter.fetch_add(1, Ordering::SeqCst));
        let now = now_ms();
        let expires_at_ms = if *self.issue_expired.lock().unwrap() {
            now - 1
        } else {
            now + 300_000
        };
        self.tokens.lock().unwrap().insert(
            token.clone(),
            IssuedToken {
                action: action.clone(),
                origin: origin.clone(),
                expires_at_ms,
                used: false,
            },
        );
        Ok(ActionPreview {
            token,
            title: title_of(&action),
            description: "Test action".into(),
            action,
            origin,
            targets: Vec::new(),
            impact: Vec::new(),
            estimated_bytes_freed: None,
            risk: ActionRisk::High,
            reversibility: Reversibility::Irreversible,
            warnings: Vec::new(),
            requires_elevation: false,
            created_at_ms: now,
            expires_at_ms,
        })
    }
}

impl ActionCommitter for FakeActions {
    fn commit(&self, token: &str) -> CoreResult<ActionOutcome> {
        let mut tokens = self.tokens.lock().unwrap();
        let issued = tokens
            .get_mut(token)
            .ok_or(SentinelError::ActionTokenInvalid)?;
        if issued.used || issued.expires_at_ms <= now_ms() {
            return Err(SentinelError::ActionTokenInvalid);
        }
        issued.used = true;
        let issued = issued.clone();
        drop(tokens);
        self.committed.lock().unwrap().push(issued.action.clone());
        Ok(ActionOutcome {
            summary: format!("{} done", title_of(&issued.action)),
            action: issued.action,
            origin: issued.origin,
            status: OutcomeStatus::Succeeded,
            items: Vec::new(),
            before: vec![Metric {
                key: "cpu".into(),
                label: "CPU".into(),
                value: 90.0,
                unit: MetricUnit::Percent,
            }],
            after: vec![Metric {
                key: "cpu".into(),
                label: "CPU".into(),
                value: 20.0,
                unit: MetricUnit::Percent,
            }],
            audit_id: 1,
            finished_at_ms: now_ms(),
        })
    }

    fn reject(&self, token: &str) -> CoreResult<()> {
        let mut tokens = self.tokens.lock().unwrap();
        let issued = tokens
            .get_mut(token)
            .ok_or(SentinelError::ActionTokenInvalid)?;
        if issued.used {
            return Err(SentinelError::ActionTokenInvalid);
        }
        issued.used = true;
        self.rejected.lock().unwrap().push(token.to_owned());
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Scripted model

pub fn text(t: &str) -> ContentBlock {
    ContentBlock::Text { text: t.into() }
}

pub fn tool(id: &str, name: &str, input: Value) -> ContentBlock {
    ContentBlock::ToolUse {
        id: id.into(),
        name: name.into(),
        input,
    }
}

pub fn reply(content: Vec<ContentBlock>) -> ChatResponse {
    let stop_reason = if content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    {
        StopReason::ToolUse
    } else {
        StopReason::EndTurn
    };
    ChatResponse {
        content,
        stop_reason,
        usage: Usage::default(),
    }
}

pub struct ScriptedBackend {
    pub provider: ProviderId,
    script: Mutex<VecDeque<CoreResult<ChatResponse>>>,
    pub requests: Mutex<Vec<ChatRequest>>,
    pub tool_capable: bool,
    /// Delay before each response, to exercise cancellation.
    pub delay: Duration,
}

impl ScriptedBackend {
    pub fn new(provider: ProviderId, script: Vec<ChatResponse>) -> Arc<Self> {
        Arc::new(Self {
            provider,
            script: Mutex::new(script.into_iter().map(Ok).collect()),
            requests: Mutex::new(Vec::new()),
            tool_capable: true,
            delay: Duration::ZERO,
        })
    }

    pub fn with(
        provider: ProviderId,
        script: Vec<CoreResult<ChatResponse>>,
        tool_capable: bool,
        delay: Duration,
    ) -> Arc<Self> {
        Arc::new(Self {
            provider,
            script: Mutex::new(script.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            tool_capable,
            delay,
        })
    }

    /// Every tool result the engine sent back to the model, by tool-use id.
    pub fn tool_results(&self) -> Vec<(String, String, bool)> {
        self.requests
            .lock()
            .unwrap()
            .last()
            .map(|r| {
                r.messages
                    .iter()
                    .flat_map(|m| m.content.iter())
                    .filter_map(|b| match b {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => Some((tool_use_id.clone(), content.clone(), *is_error)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait]
impl AgentBackend for ScriptedBackend {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn supports_tool_calling(&self, _model: &str) -> bool {
        self.tool_capable
    }

    async fn list_models(&self) -> CoreResult<Vec<ModelInfo>> {
        Ok(Vec::new())
    }

    async fn send_message(&self, request: &ChatRequest) -> CoreResult<ChatResponse> {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        self.stream_response(request, tx).await
    }

    async fn stream_response(
        &self,
        request: &ChatRequest,
        deltas: UnboundedSender<StreamDelta>,
    ) -> CoreResult<ChatResponse> {
        self.requests.lock().unwrap().push(request.clone());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        let next = self.script.lock().unwrap().pop_front();
        let response = next.unwrap_or_else(|| Ok(reply(vec![text("(script exhausted)")])))?;
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                let _ = deltas.send(StreamDelta::Text(text.clone()));
            }
        }
        Ok(response)
    }
}

// ---------------------------------------------------------------------------------------------
// Events

#[derive(Default)]
pub struct RecordingSink {
    pub events: Mutex<Vec<AgentStreamPayload>>,
}

impl RecordingSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn events(&self) -> Vec<AgentEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|p| p.event.clone())
            .collect()
    }
}

impl AgentEventSink for RecordingSink {
    fn emit(&self, payload: AgentStreamPayload) {
        self.events.lock().unwrap().push(payload);
    }
}

pub fn json_of(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| json!(s))
}
