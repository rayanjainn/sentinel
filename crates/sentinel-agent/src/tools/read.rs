//! `ReadToolExecutor` over the shared `SystemQueries` facade — the same data the UI shows.
//!
//! Every result is a compact JSON object with a one-line `summary` (surfaced in the chat's tool
//! row). Lists are truncated with totals so the model knows what it is not seeing.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{Map, Value, json};

use sentinel_core::model::*;
use sentinel_core::service::{FileFilter, SystemQueries};
use sentinel_core::{CoreResult, SentinelError};

use super::{ReadToolExecutor, ToolAccess, access_of, names, paths, specs};

pub struct SystemReadTools {
    queries: Arc<dyn SystemQueries>,
    scan_timeout: Duration,
}

impl SystemReadTools {
    pub fn new(queries: Arc<dyn SystemQueries>) -> Self {
        Self {
            queries,
            scan_timeout: Duration::from_secs(300),
        }
    }

    pub fn with_scan_timeout(mut self, timeout: Duration) -> Self {
        self.scan_timeout = timeout;
        self
    }
}

#[async_trait]
impl ReadToolExecutor for SystemReadTools {
    async fn call(&self, name: &str, input: Value) -> CoreResult<Value> {
        if access_of(name) != Some(ToolAccess::Read) {
            return Err(SentinelError::invalid(format!(
                "`{name}` is not a read tool"
            )));
        }
        let args = Args::parse(name, input)?;
        let queries = Arc::clone(&self.queries);
        let name = name.to_owned();
        let timeout = self.scan_timeout;
        tokio::task::spawn_blocking(move || dispatch(&*queries, &name, &args, timeout))
            .await
            .map_err(SentinelError::internal)?
    }
}

/// Extracts the one-line summary a read result carries.
pub fn summary_of(result: &Value) -> String {
    result["summary"].as_str().unwrap_or("Done").to_owned()
}

fn dispatch(
    q: &dyn SystemQueries,
    name: &str,
    args: &Args,
    timeout: Duration,
) -> CoreResult<Value> {
    match name {
        names::GET_SYSTEM_OVERVIEW => system_overview(q),
        names::GET_RESOURCE_USAGE => resource_usage(q, args),
        names::LIST_PROCESSES => list_processes(q, args),
        names::GET_PROCESS_DETAILS => process_details(q, args),
        names::LIST_NETWORK_SOCKETS => network_sockets(q, args),
        names::FIND_PORT_OWNER => port_owner(q, args),
        names::LIST_VOLUMES => volumes(q),
        names::SCAN_STORAGE => scan_storage(q, args, timeout),
        names::GET_STORAGE_BREAKDOWN => storage_breakdown(q, args, timeout),
        names::FIND_LARGE_FILES => large_files(q, args, timeout),
        names::GET_CLEANUP_SUGGESTIONS => cleanup_suggestions(q, args, timeout),
        names::FIND_DUPLICATE_FILES => duplicates(q, args, timeout),
        names::LIST_FIREWALL_RULES => firewall_rules(q),
        other => Err(SentinelError::invalid(format!("unknown tool `{other}`"))),
    }
}

// ---------------------------------------------------------------------------------------------
// Arguments: validated against the tool's schema, tolerant of numbers sent as strings (small
// local models frequently emit `"limit": "5"`).

#[derive(Debug)]
pub struct Args(Map<String, Value>);

impl Args {
    pub fn parse(tool: &str, input: Value) -> CoreResult<Self> {
        let map = match input {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            Value::String(raw) => {
                return Err(SentinelError::invalid(format!(
                    "arguments for `{tool}` were not valid JSON: {}",
                    super::super::providers::http::truncate(&raw)
                )));
            }
            _ => {
                return Err(SentinelError::invalid(format!(
                    "arguments for `{tool}` must be a JSON object"
                )));
            }
        };
        let spec = specs::spec(tool)
            .ok_or_else(|| SentinelError::invalid(format!("unknown tool `{tool}`")))?;
        let known = spec.input_schema["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        if let Some(unknown) = map.keys().find(|k| !known.contains_key(*k)) {
            let allowed: Vec<&String> = known.keys().collect();
            return Err(SentinelError::invalid(format!(
                "`{tool}` has no parameter `{unknown}` (allowed: {allowed:?})"
            )));
        }
        for required in spec.input_schema["required"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let key = required.as_str().unwrap_or_default();
            if map.get(key).is_none_or(Value::is_null) {
                return Err(SentinelError::invalid(format!("`{tool}` requires `{key}`")));
            }
        }
        let args = Self(map);
        for (key, schema) in &known {
            args.check(key, schema)?;
        }
        Ok(args)
    }

    fn check(&self, key: &str, schema: &Value) -> CoreResult<()> {
        if self.0.get(key).is_none_or(Value::is_null) {
            return Ok(());
        }
        match schema["type"].as_str() {
            Some("integer") => {
                self.int(key)?;
            }
            Some("number") => {
                self.number(key)?;
            }
            Some("boolean") => {
                self.bool(key)?;
            }
            Some("string") => {
                self.string(key)?;
            }
            _ => {}
        }
        let as_number = || self.number(key).ok().flatten();
        if let (Some(min), Some(v)) = (schema["minimum"].as_f64(), as_number())
            && v < min
        {
            return Err(SentinelError::invalid(format!(
                "`{key}` must be at least {min}"
            )));
        }
        if let (Some(max), Some(v)) = (schema["maximum"].as_f64(), as_number())
            && v > max
        {
            return Err(SentinelError::invalid(format!(
                "`{key}` must be at most {max}"
            )));
        }
        if let Some(allowed) = schema["enum"].as_array()
            && let Some(v) = self.string(key)?
            && !allowed.iter().any(|a| a.as_str() == Some(v.as_str()))
        {
            return Err(SentinelError::invalid(format!(
                "`{key}` must be one of {allowed:?}"
            )));
        }
        Ok(())
    }

    fn raw(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|v| !v.is_null())
    }

    pub fn number(&self, key: &str) -> CoreResult<Option<f64>> {
        match self.raw(key) {
            None => Ok(None),
            Some(Value::Number(n)) => Ok(n.as_f64()),
            Some(Value::String(s)) => s
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|_| SentinelError::invalid(format!("`{key}` must be a number"))),
            Some(_) => Err(SentinelError::invalid(format!("`{key}` must be a number"))),
        }
    }

    pub fn int(&self, key: &str) -> CoreResult<Option<i64>> {
        match self.number(key)? {
            None => Ok(None),
            Some(v) if v.fract() == 0.0 && v.abs() < 9.0e15 => Ok(Some(v as i64)),
            Some(_) => Err(SentinelError::invalid(format!(
                "`{key}` must be a whole number"
            ))),
        }
    }

    pub fn bool(&self, key: &str) -> CoreResult<Option<bool>> {
        match self.raw(key) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(Value::String(s)) if s.eq_ignore_ascii_case("true") => Ok(Some(true)),
            Some(Value::String(s)) if s.eq_ignore_ascii_case("false") => Ok(Some(false)),
            Some(_) => Err(SentinelError::invalid(format!(
                "`{key}` must be true or false"
            ))),
        }
    }

    pub fn string(&self, key: &str) -> CoreResult<Option<String>> {
        match self.raw(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(Value::Number(n)) => Ok(Some(n.to_string())),
            Some(_) => Err(SentinelError::invalid(format!("`{key}` must be a string"))),
        }
    }

    pub fn strings(&self, key: &str) -> CoreResult<Option<Vec<String>>> {
        match self.raw(key) {
            None => Ok(None),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_owned).ok_or_else(|| {
                        SentinelError::invalid(format!("`{key}` must be a list of strings"))
                    })
                })
                .collect::<CoreResult<Vec<_>>>()
                .map(Some),
            // A single path sent as a bare string.
            Some(Value::String(s)) => Ok(Some(vec![s.clone()])),
            Some(_) => Err(SentinelError::invalid(format!(
                "`{key}` must be a list of strings"
            ))),
        }
    }

    fn uint_or(&self, key: &str, default: u64) -> CoreResult<u64> {
        Ok(self.int(key)?.map_or(default, |v| v.max(0) as u64))
    }
}

// ---------------------------------------------------------------------------------------------
// Formatting

pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn fmt_duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        round1(part as f64 * 100.0 / whole as f64)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn days_ago(ts: Option<TimestampSecs>) -> Option<u64> {
    ts.map(|t| now_secs().saturating_sub(t) / 86_400)
}

/// Days since the latest of modification and access — "last used".
fn last_used_days(modified: Option<TimestampSecs>, accessed: Option<TimestampSecs>) -> Option<u64> {
    days_ago(modified.max(accessed))
}

fn truncated<T>(items: Vec<T>, limit: usize) -> (Vec<T>, usize, bool) {
    let total = items.len();
    let items: Vec<T> = items.into_iter().take(limit).collect();
    (items, total, total > limit)
}

fn error_text(err: &SentinelError) -> String {
    err.to_string()
}

// ---------------------------------------------------------------------------------------------
// System and processes

fn process_row(p: &ProcessInfo) -> Value {
    json!({
        "pid": p.pid,
        "ppid": p.ppid,
        "name": p.name,
        "user": p.user,
        "cpu_percent": round1(p.cpu_percent as f64),
        "cpu_avg_percent": round1(p.cpu_percent_avg as f64),
        "memory": fmt_bytes(p.memory_rss),
        "status": p.status,
        "running_for": fmt_duration(p.run_time_secs),
    })
}

fn system_overview(q: &dyn SystemQueries) -> CoreResult<Value> {
    let info = q.system_info();
    let sample = q.resource_sample()?;
    let perms = q.permission_status();
    let m = &sample.memory;
    let volumes = match q.volumes() {
        Ok(vols) => json!(vols.iter().map(volume_row).collect::<Vec<_>>()),
        Err(err) => json!({"error": error_text(&err)}),
    };
    let (process_count, top) = match q.process_snapshot() {
        Ok(mut snap) => {
            snap.processes
                .sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
            let count = snap.processes.len();
            (
                json!(count),
                json!(
                    snap.processes
                        .iter()
                        .take(5)
                        .map(process_row)
                        .collect::<Vec<_>>()
                ),
            )
        }
        Err(err) => (Value::Null, json!({"error": error_text(&err)})),
    };
    let summary = format!(
        "CPU {:.0}%, memory {} of {} used{}",
        sample.cpu_total,
        fmt_bytes(m.used),
        fmt_bytes(m.total),
        process_count
            .as_u64()
            .map(|c| format!(", {c} processes"))
            .unwrap_or_default()
    );
    Ok(json!({
        "summary": summary,
        "os": format!("{} {}", info.os_name, info.os_version.clone().unwrap_or_default()).trim().to_owned(),
        "hostname": info.hostname,
        "arch": info.arch,
        "cpu": info.cpu_brand,
        "logical_cores": info.logical_cores,
        "physical_cores": info.physical_cores,
        "uptime": fmt_duration(now_secs().saturating_sub(info.boot_time)),
        "cpu_percent": round1(sample.cpu_total as f64),
        "memory": {"total": fmt_bytes(m.total), "used": fmt_bytes(m.used), "available": fmt_bytes(m.available), "percent_used": percent(m.used, m.total)},
        "swap": {"total": fmt_bytes(m.swap_total), "used": fmt_bytes(m.swap_used)},
        "load_average": sample.load.map(|l| json!([round1(l.one), round1(l.five), round1(l.fifteen)])),
        "volumes": volumes,
        "permissions": {
            "full_disk_access": perms.full_disk_access,
            "running_elevated": perms.running_elevated,
            "can_request_elevation": perms.can_request_elevation,
            "firewall_available": perms.firewall.available,
        },
        "process_count": process_count,
        "top_cpu_processes": top,
        "note": "cpu_percent for processes: 100 = one full logical core",
    }))
}

fn resource_usage(q: &dyn SystemQueries, args: &Args) -> CoreResult<Value> {
    let sample = q.resource_sample()?;
    let m = &sample.memory;
    let mut out = json!({
        "cpu_percent": round1(sample.cpu_total as f64),
        "per_core_percent": sample.per_core.iter().map(|c| c.round() as i64).collect::<Vec<_>>(),
        "memory": {
            "total": fmt_bytes(m.total),
            "used": fmt_bytes(m.used),
            "available": fmt_bytes(m.available),
            "cached": m.cached.map(fmt_bytes),
            "compressed": m.compressed.map(fmt_bytes),
            "wired": m.wired.map(fmt_bytes),
            "percent_used": percent(m.used, m.total),
            "swap_used": fmt_bytes(m.swap_used),
            "swap_total": fmt_bytes(m.swap_total),
        },
        "load_average": sample.load.map(|l| json!([round1(l.one), round1(l.five), round1(l.fifteen)])),
        "network": {"receive_per_second": fmt_bytes(sample.network.rx_bps), "send_per_second": fmt_bytes(sample.network.tx_bps)},
        "thermal": match &sample.thermal {
            Some(t) => json!({
                "temperatures": t.temperatures.iter().map(|r| json!({"label": r.label, "celsius": round1(r.celsius as f64)})).collect::<Vec<_>>(),
                "fans": t.fans.iter().map(|f| json!({"label": f.label, "rpm": f.rpm})).collect::<Vec<_>>(),
            }),
            None => json!("not exposed by this system"),
        },
    });
    let window = args.uint_or("history_seconds", 0)?.min(900) as u32;
    if window > 0 {
        let history = q.resource_history(window);
        if !history.is_empty() {
            let n = history.len() as f64;
            let cpu_avg = history.iter().map(|s| s.cpu_total as f64).sum::<f64>() / n;
            let cpu_peak = history
                .iter()
                .map(|s| s.cpu_total as f64)
                .fold(0.0, f64::max);
            let mem_avg = history.iter().map(|s| s.memory.used as f64).sum::<f64>() / n;
            let mem_peak = history.iter().map(|s| s.memory.used).max().unwrap_or(0);
            out["history"] = json!({
                "seconds": window,
                "samples": history.len(),
                "cpu_avg_percent": round1(cpu_avg),
                "cpu_peak_percent": round1(cpu_peak),
                "memory_used_avg": fmt_bytes(mem_avg as u64),
                "memory_used_peak": fmt_bytes(mem_peak),
            });
        } else {
            out["history"] = json!("no history recorded yet");
        }
    }
    out["summary"] = json!(format!(
        "CPU {:.0}% across {} cores, memory {:.0}% used",
        sample.cpu_total,
        sample.per_core.len(),
        percent(m.used, m.total)
    ));
    Ok(out)
}

fn list_processes(q: &dyn SystemQueries, args: &Args) -> CoreResult<Value> {
    let snap = q.process_snapshot()?;
    let needle = args.string("name_contains")?.map(|s| s.to_lowercase());
    let user = args.string("user")?;
    let mut rows: Vec<ProcessInfo> = snap
        .processes
        .into_iter()
        .filter(|p| {
            needle.as_ref().is_none_or(|n| {
                p.name.to_lowercase().contains(n)
                    || p.exe.as_ref().is_some_and(|e| e.to_lowercase().contains(n))
            })
        })
        .filter(|p| {
            user.as_ref()
                .is_none_or(|u| p.user.as_deref() == Some(u.as_str()))
        })
        .collect();
    match args.string("sort_by")?.as_deref().unwrap_or("cpu") {
        "memory" => rows.sort_by_key(|r| Reverse(r.memory_rss)),
        "name" => rows.sort_by_key(|r| r.name.to_lowercase()),
        "pid" => rows.sort_by_key(|p| p.pid),
        "start_time" => rows.sort_by_key(|r| Reverse(r.start_time)),
        _ => rows.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent)),
    }
    let limit = args.uint_or("limit", 25)?.clamp(1, 100) as usize;
    let (rows, total, is_truncated) = truncated(rows, limit);
    let summary = match rows.first() {
        Some(top) if total > 0 => format!(
            "{total} processes; top {} (PID {}) at {:.0}% CPU",
            top.name, top.pid, top.cpu_percent
        ),
        _ => "No matching processes".to_owned(),
    };
    Ok(json!({
        "summary": summary,
        "total_matching": total,
        "truncated": is_truncated,
        "logical_cores": snap.logical_cores,
        "total_memory": fmt_bytes(snap.total_memory),
        "note": "cpu_percent: 100 = one full logical core",
        "processes": rows.iter().map(process_row).collect::<Vec<_>>(),
    }))
}

fn pid_arg(args: &Args, key: &str) -> CoreResult<Pid> {
    let pid = args
        .int(key)?
        .ok_or_else(|| SentinelError::invalid(format!("`{key}` is required")))?;
    Pid::try_from(pid).map_err(|_| SentinelError::invalid(format!("`{key}` is not a valid PID")))
}

fn socket_row(s: &SocketEntry) -> Value {
    json!({
        "protocol": s.protocol,
        "local": format!("{}:{}", s.local_addr, s.local_port),
        "remote": s.remote_addr.as_ref().map(|a| format!("{a}:{}", s.remote_port.unwrap_or(0))),
        "remote_host": s.remote_host,
        "state": s.state,
        "pid": s.pid,
        "process": s.process_name,
        "received": s.bytes_in.map(fmt_bytes),
        "sent": s.bytes_out.map(fmt_bytes),
    })
}

fn process_details(q: &dyn SystemQueries, args: &Args) -> CoreResult<Value> {
    let pid = pid_arg(args, "pid")?;
    let detail = q.process_detail(pid)?;
    let p = &detail.info;
    let command = p.cmd.join(" ");
    let command = if command.chars().count() > 500 {
        format!("{}…", command.chars().take(500).collect::<String>())
    } else {
        command
    };
    let files: Vec<&str> = detail
        .open_files
        .iter()
        .filter_map(|f| f.path.as_deref())
        .take(20)
        .collect();
    let (connections, connection_total, _) =
        truncated(detail.connections.iter().map(socket_row).collect(), 20);
    Ok(json!({
        "summary": format!("{} (PID {}), {:.0}% CPU, {}", p.name, p.pid, p.cpu_percent, fmt_bytes(p.memory_rss)),
        "pid": p.pid,
        "ppid": p.ppid,
        "name": p.name,
        "user": p.user,
        "status": p.status,
        "executable": p.exe,
        "command_line": command,
        "start_time_unix": p.start_time,
        "running_for": fmt_duration(p.run_time_secs),
        "cpu_percent": round1(p.cpu_percent as f64),
        "cpu_avg_percent": round1(p.cpu_percent_avg as f64),
        "memory_resident": fmt_bytes(p.memory_rss),
        "memory_virtual": fmt_bytes(p.memory_virtual),
        "threads": p.thread_count,
        "nice": p.nice,
        "open_files_count": detail.open_files.len(),
        "open_files_sample": files,
        "open_files_error": detail.open_files_error.as_ref().map(|e| e.message.clone()),
        "connections_total": connection_total,
        "connections": connections,
        "graceful_terminate": if detail.has_window { "asks the app to quit (closes its windows)" } else { "sends a termination signal" },
    }))
}

fn network_sockets(q: &dyn SystemQueries, args: &Args) -> CoreResult<Value> {
    let snap = q.network_snapshot()?;
    let state = args.string("state")?.unwrap_or_else(|| "all".into());
    let protocol = args.string("protocol")?;
    let pid = args.int("pid")?;
    let listening = snap
        .sockets
        .iter()
        .filter(|s| s.state == Some(TcpState::Listen))
        .count();
    let rows: Vec<&SocketEntry> = snap
        .sockets
        .iter()
        .filter(|s| match state.as_str() {
            "listen" => s.state == Some(TcpState::Listen),
            "established" => s.state == Some(TcpState::Established),
            _ => true,
        })
        .filter(|s| {
            protocol.as_deref().is_none_or(|p| match s.protocol {
                TransportProtocol::Tcp => p == "tcp",
                TransportProtocol::Udp => p == "udp",
            })
        })
        .filter(|s| pid.is_none_or(|pid| s.pid.map(i64::from) == Some(pid)))
        .collect();
    let limit = args.uint_or("limit", 50)?.clamp(1, 200) as usize;
    let (rows, total, is_truncated) = truncated(rows, limit);
    Ok(json!({
        "summary": format!("{total} matching sockets ({} sockets total, {listening} listening)", snap.sockets.len()),
        "total_matching": total,
        "truncated": is_truncated,
        "throughput": {"receive_per_second": fmt_bytes(snap.throughput.rx_bps), "send_per_second": fmt_bytes(snap.throughput.tx_bps)},
        "sockets": rows.into_iter().map(socket_row).collect::<Vec<_>>(),
    }))
}

fn port_owner(q: &dyn SystemQueries, args: &Args) -> CoreResult<Value> {
    let port = args.int("port")?.unwrap_or(0);
    let protocol = args.string("protocol")?;
    let snap = q.network_snapshot()?;
    let mut sockets: Vec<&SocketEntry> = snap
        .sockets
        .iter()
        .filter(|s| i64::from(s.local_port) == port)
        .filter(|s| {
            protocol.as_deref().is_none_or(|p| match s.protocol {
                TransportProtocol::Tcp => p == "tcp",
                TransportProtocol::Udp => p == "udp",
            })
        })
        .collect();
    sockets.sort_by_key(|s| s.state != Some(TcpState::Listen));
    let processes: HashMap<Pid, ProcessInfo> = match q.process_snapshot() {
        Ok(snap) => snap.processes.into_iter().map(|p| (p.pid, p)).collect(),
        Err(_) => HashMap::new(),
    };
    let mut owners: Vec<Value> = Vec::new();
    let mut seen = Vec::new();
    for socket in &sockets {
        let Some(pid) = socket.pid else { continue };
        if seen.contains(&pid) {
            continue;
        }
        seen.push(pid);
        let info = processes.get(&pid);
        owners.push(json!({
            "pid": pid,
            "name": info.map(|p| p.name.clone()).or_else(|| socket.process_name.clone()),
            "user": info.and_then(|p| p.user.clone()),
            "executable": info.and_then(|p| p.exe.clone()),
            "start_time_unix": info.map(|p| p.start_time),
            "cpu_percent": info.map(|p| round1(p.cpu_percent as f64)),
            "memory": info.map(|p| fmt_bytes(p.memory_rss)),
        }));
    }
    let summary = if sockets.is_empty() {
        format!("Nothing is using port {port}")
    } else if owners.is_empty() {
        format!(
            "Port {port} is in use, but its owner is not visible (it may belong to another user)"
        )
    } else {
        let names: Vec<String> = owners
            .iter()
            .map(|o| {
                format!(
                    "{} (PID {})",
                    o["name"].as_str().unwrap_or("unknown"),
                    o["pid"]
                )
            })
            .collect();
        format!("Port {port}: {}", names.join(", "))
    };
    Ok(json!({
        "summary": summary,
        "port": port,
        "owners": owners,
        "sockets": sockets.into_iter().take(30).map(socket_row).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------------------------------------
// Storage

fn volume_row(v: &VolumeInfo) -> Value {
    let used = v.total_bytes.saturating_sub(v.available_bytes);
    json!({
        "name": v.name,
        "mount_point": v.mount_point,
        "file_system": v.file_system,
        "total": fmt_bytes(v.total_bytes),
        "free": fmt_bytes(v.available_bytes),
        "percent_used": percent(used, v.total_bytes),
        "system_volume": v.is_system,
        "removable": v.is_removable,
    })
}

fn volumes(q: &dyn SystemQueries) -> CoreResult<Value> {
    let vols = q.volumes()?;
    let summary = match vols.iter().find(|v| v.is_system).or(vols.first()) {
        Some(v) => format!(
            "{} volumes; {} {:.0}% used, {} free",
            vols.len(),
            v.name,
            percent(
                v.total_bytes.saturating_sub(v.available_bytes),
                v.total_bytes
            ),
            fmt_bytes(v.available_bytes)
        ),
        None => "No volumes reported".to_owned(),
    };
    Ok(json!({"summary": summary, "volumes": vols.iter().map(volume_row).collect::<Vec<_>>()}))
}

fn path_arg(args: &Args, key: &str) -> CoreResult<PathBuf> {
    let raw = args
        .string(key)?
        .ok_or_else(|| SentinelError::invalid(format!("`{key}` is required")))?;
    let path = paths::normalize(&raw)?;
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(SentinelError::PathNotFound {
            path: paths::display(&path),
        });
    }
    Ok(path)
}

/// Reuses a completed scan covering `path`, or scans and waits.
fn ensure_scan(
    q: &dyn SystemQueries,
    path: &Path,
    force: bool,
    timeout: Duration,
) -> CoreResult<ScanSummary> {
    let display = paths::display(path);
    if !force && let Some(id) = q.scan_covering(&display) {
        return q.scan_summary(&id);
    }
    let id = q.start_scan(ScanRequest {
        root: display.clone(),
        cross_mounts: false,
    })?;
    q.wait_for_scan(&id, timeout).map_err(|err| match err {
        SentinelError::Unavailable { .. } => SentinelError::Unavailable {
            feature: "storage scan".into(),
            reason: format!(
                "the scan of {display} is still running after {} minutes. Its progress is visible in Storage; ask the user to wait, then call this tool again to use the finished scan.",
                timeout.as_secs() / 60
            ),
        },
        other => other,
    })
}

fn scan_age(summary: &ScanSummary) -> String {
    let now_ms = now_secs() * 1000;
    fmt_duration(now_ms.saturating_sub(summary.started_at_ms + summary.duration_ms) / 1000)
}

fn scan_storage(q: &dyn SystemQueries, args: &Args, timeout: Duration) -> CoreResult<Value> {
    let path = path_arg(args, "path")?;
    let force = args.bool("force_rescan")?.unwrap_or(false);
    let s = ensure_scan(q, &path, force, timeout)?;
    let reclaimable: u64 = s
        .suggestions
        .iter()
        .filter(|x| x.actionable)
        .map(|x| x.size_bytes)
        .sum();
    let mut by_type = s.by_extension.clone();
    by_type.sort_by_key(|t| Reverse(t.bytes));
    Ok(json!({
        "summary": format!("{}: {} in {} files", s.root, fmt_bytes(s.total_bytes), s.file_count),
        "root": s.root,
        "scan_finished_ago": scan_age(&s),
        "total_size": fmt_bytes(s.total_bytes),
        "files": s.file_count,
        "folders": s.dir_count,
        "unreadable_entries": s.unreadable_entries,
        "unreadable_samples": s.unreadable_samples.iter().take(5).collect::<Vec<_>>(),
        "largest_files": s.largest_files.iter().take(10).map(file_row).collect::<Vec<_>>(),
        "space_by_type": by_type.iter().take(8).map(|e| json!({
            "extension": e.extension, "kind": e.file_kind, "size": fmt_bytes(e.bytes), "files": e.count,
        })).collect::<Vec<_>>(),
        "cleanup_suggestions": {"count": s.suggestions.len(), "reclaimable": fmt_bytes(reclaimable)},
    }))
}

fn file_row(f: &FileEntry) -> Value {
    json!({
        "path": f.path,
        "size": fmt_bytes(f.size_bytes),
        "size_bytes": f.size_bytes,
        "kind": f.file_kind,
        "last_modified_days_ago": days_ago(f.modified),
        "last_accessed_days_ago": days_ago(f.accessed),
        "last_used_days_ago": last_used_days(f.modified, f.accessed),
    })
}

/// Walks the scan tree from its root to the node whose path is `target`.
fn locate_node(q: &dyn SystemQueries, scan_id: &str, target: &Path) -> CoreResult<TreeNode> {
    let mut node = q.scan_tree(&TreeQuery {
        scan_id: scan_id.to_owned(),
        node_id: None,
        depth: 1,
        max_children: u32::MAX,
    })?;
    for _ in 0..4096 {
        if Path::new(&node.path) == target {
            return Ok(node);
        }
        let next = node
            .children
            .as_ref()
            .and_then(|children| {
                children
                    .iter()
                    .find(|c| {
                        c.kind == NodeKind::Directory
                            && paths::is_within(target, Path::new(&c.path))
                    })
                    .or_else(|| children.iter().find(|c| Path::new(&c.path) == target))
            })
            .map(|c| c.id);
        let Some(id) = next else { break };
        node = q.scan_tree(&TreeQuery {
            scan_id: scan_id.to_owned(),
            node_id: Some(id),
            depth: 1,
            max_children: u32::MAX,
        })?;
    }
    Err(SentinelError::PathNotFound {
        path: paths::display(target),
    })
}

fn tree_row(node: &TreeNode, depth: u8) -> Value {
    let mut row = json!({
        "name": node.name,
        "path": node.path,
        "kind": node.kind,
        "size": fmt_bytes(node.size_bytes),
        "items": node.item_count,
        "last_used_days_ago": last_used_days(node.modified, node.accessed),
    });
    if let Some(category) = node.category {
        row["cleanup_category"] = json!(category);
    }
    if node.unreadable_entries > 0 {
        row["unreadable_entries"] = json!(node.unreadable_entries);
    }
    if depth > 1
        && let Some(children) = &node.children
    {
        row["children"] = json!(
            children
                .iter()
                .map(|c| tree_row(c, depth - 1))
                .collect::<Vec<_>>()
        );
    }
    row
}

fn storage_breakdown(q: &dyn SystemQueries, args: &Args, timeout: Duration) -> CoreResult<Value> {
    let path = path_arg(args, "path")?;
    let depth = args.uint_or("depth", 1)?.clamp(1, 3) as u8;
    let max_children = args.uint_or("max_children", 15)?.clamp(1, 50) as u32;
    let summary = ensure_scan(q, &path, false, timeout)?;
    let located = locate_node(q, &summary.scan_id, &path)?;
    let node = q.scan_tree(&TreeQuery {
        scan_id: summary.scan_id.clone(),
        node_id: Some(located.id),
        depth,
        max_children,
    })?;
    let children: Vec<Value> = node
        .children
        .iter()
        .flatten()
        .map(|c| tree_row(c, depth))
        .collect();
    Ok(json!({
        "summary": format!("{}: {} in {} items", node.path, fmt_bytes(node.size_bytes), node.item_count),
        "path": node.path,
        "size": fmt_bytes(node.size_bytes),
        "items": node.item_count,
        "scan_finished_ago": scan_age(&summary),
        "children": children,
    }))
}

fn large_files(q: &dyn SystemQueries, args: &Args, timeout: Duration) -> CoreResult<Value> {
    let path = path_arg(args, "path")?;
    let min_mb = args.number("min_size_mb")?.unwrap_or(100.0).max(0.0);
    let unused = args
        .int("unused_for_days")?
        .map(|d| d.clamp(1, 36_500) as u32);
    let limit = args.uint_or("limit", 50)?.clamp(1, 200) as u32;
    let summary = ensure_scan(q, &path, false, timeout)?;
    let files = q.find_files(
        &summary.scan_id,
        &FileFilter {
            under_path: Some(paths::display(&path)),
            min_size_bytes: (min_mb * 1_000_000.0) as u64,
            unused_for_days: unused,
            limit,
        },
    )?;
    let total: u64 = files.iter().map(|f| f.size_bytes).sum();
    let constraint = unused
        .map(|d| format!(", unused for {d}+ days"))
        .unwrap_or_default();
    Ok(json!({
        "summary": format!("{} files over {min_mb} MB{constraint}, {} total", files.len(), fmt_bytes(total)),
        "path": paths::display(&path),
        "scan_finished_ago": scan_age(&summary),
        "count": files.len(),
        "total_size": fmt_bytes(total),
        "total_size_bytes": total,
        "limit_reached": files.len() as u32 >= limit,
        "files": files.iter().map(file_row).collect::<Vec<_>>(),
    }))
}

fn cleanup_suggestions(q: &dyn SystemQueries, args: &Args, timeout: Duration) -> CoreResult<Value> {
    let path = match args.string("path")? {
        Some(_) => path_arg(args, "path")?,
        None => paths::home_dir()
            .ok_or_else(|| SentinelError::invalid("home directory is unknown; pass `path`"))?,
    };
    let unused = args.int("unused_for_days")?;
    let summary = ensure_scan(q, &path, false, timeout)?;
    let now = now_secs();
    let rows: Vec<&CleanupSuggestion> = summary
        .suggestions
        .iter()
        .filter(|s| paths::is_within(Path::new(&s.path), &path))
        .filter(|s| match unused {
            // Unknown timestamps cannot satisfy a "not used recently" constraint.
            Some(days) => s
                .last_modified
                .max(s.last_accessed)
                .is_some_and(|t| now.saturating_sub(t) / 86_400 >= days.max(0) as u64),
            None => true,
        })
        .collect();
    let reclaimable: u64 = rows
        .iter()
        .filter(|s| s.actionable)
        .map(|s| s.size_bytes)
        .sum();
    Ok(json!({
        "summary": format!("{} suggestions, {} reclaimable", rows.len(), fmt_bytes(reclaimable)),
        "path": paths::display(&path),
        "scan_finished_ago": scan_age(&summary),
        "reclaimable": fmt_bytes(reclaimable),
        "reclaimable_bytes": reclaimable,
        "suggestions": rows.iter().map(|s| json!({
            "path": s.path,
            "category": s.category,
            "size": fmt_bytes(s.size_bytes),
            "size_bytes": s.size_bytes,
            "items": s.item_count,
            "last_used_days_ago": last_used_days(s.last_modified, s.last_accessed),
            "why_safe": s.rationale,
            "actionable": s.actionable,
        })).collect::<Vec<_>>(),
    }))
}

fn duplicates(q: &dyn SystemQueries, args: &Args, timeout: Duration) -> CoreResult<Value> {
    let path = path_arg(args, "path")?;
    let min_mb = args.number("min_size_mb")?.unwrap_or(1.0).max(0.0);
    let limit = args.uint_or("limit", 20)?.clamp(1, 100) as usize;
    let summary = ensure_scan(q, &path, false, timeout)?;
    let job = q.start_duplicate_scan(&summary.scan_id, (min_mb * 1_000_000.0) as u64)?;
    let report = q
        .wait_for_duplicates(&job, timeout)
        .map_err(|err| match err {
            SentinelError::Unavailable { .. } => SentinelError::Unavailable {
                feature: "duplicate search".into(),
                reason: "hashing is still running; ask the user to wait, then call this tool again"
                    .into(),
            },
            other => other,
        })?;
    let mut groups: Vec<DuplicateGroup> = report
        .groups
        .into_iter()
        .map(|mut g| {
            g.files
                .retain(|f| paths::is_within(Path::new(&f.path), &path));
            g
        })
        .filter(|g| g.files.len() > 1)
        .collect();
    for g in &mut groups {
        g.reclaimable_bytes = g.size_bytes * (g.files.len() as u64 - 1);
    }
    groups.sort_by_key(|g| Reverse(g.reclaimable_bytes));
    let reclaimable: u64 = groups.iter().map(|g| g.reclaimable_bytes).sum();
    let (groups, total, is_truncated) = truncated(groups, limit);
    Ok(json!({
        "summary": format!("{total} duplicate groups, {} reclaimable", fmt_bytes(reclaimable)),
        "total_groups": total,
        "truncated": is_truncated,
        "reclaimable": fmt_bytes(reclaimable),
        "groups": groups.iter().map(|g| json!({
            "size_each": fmt_bytes(g.size_bytes),
            "copies": g.files.len(),
            "reclaimable": fmt_bytes(g.reclaimable_bytes),
            "files": g.files.iter().take(10).map(|f| json!({"path": f.path, "last_used_days_ago": last_used_days(f.modified, f.accessed)})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    }))
}

fn firewall_rules(q: &dyn SystemQueries) -> CoreResult<Value> {
    let status = q.firewall_status();
    let rules = q.firewall_rules()?;
    Ok(json!({
        "summary": format!("{} Sentinel firewall rules ({} active)", rules.len(), rules.iter().filter(|r| r.active).count()),
        "firewall": {
            "backend": status.backend,
            "available": status.available,
            "enabled": status.firewall_enabled,
            "requires_elevation": status.requires_elevation,
            "note": status.note,
        },
        "rules": rules.iter().map(|r| json!({
            "id": r.id,
            "target": r.target,
            "direction": r.direction,
            "active": r.active,
            "created_days_ago": days_ago(Some(r.created_at_ms / 1000)),
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_and_durations() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(2_300_000_000), "2.3 GB");
        assert_eq!(fmt_bytes(214_000_000_000), "214 GB");
        assert_eq!(fmt_duration(3 * 3600 + 12 * 60), "3h 12m");
        assert_eq!(fmt_duration(90_000), "1d 1h");
    }

    #[test]
    fn args_coerce_strings_and_reject_unknown_or_out_of_range() {
        let args = Args::parse(
            names::LIST_PROCESSES,
            json!({"limit": "5", "sort_by": "cpu"}),
        )
        .unwrap();
        assert_eq!(args.int("limit").unwrap(), Some(5));
        let unknown = Args::parse(names::LIST_PROCESSES, json!({"kill": true})).unwrap_err();
        assert!(
            unknown.to_string().contains("no parameter `kill`"),
            "{unknown}"
        );
        assert!(Args::parse(names::LIST_PROCESSES, json!({"limit": 500})).is_err());
        assert!(Args::parse(names::LIST_PROCESSES, json!({"sort_by": "entropy"})).is_err());
        assert!(Args::parse(names::GET_PROCESS_DETAILS, json!({})).is_err());
        assert!(Args::parse(names::GET_PROCESS_DETAILS, json!({"pid": 1.5})).is_err());
        assert!(Args::parse(names::GET_PROCESS_DETAILS, Value::String("{oops".into())).is_err());
        assert!(Args::parse(names::GET_SYSTEM_OVERVIEW, Value::Null).is_ok());
    }
}
