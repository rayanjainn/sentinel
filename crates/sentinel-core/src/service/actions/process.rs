use std::time::{Duration, Instant};

use super::{ActionService, Draft, Execution, metric};
use crate::action::OutcomeStatus;
use crate::action::{
    Action, ActionPreview, ActionRisk, ItemOutcome, MetricUnit, PreviewTarget, Reversibility,
};
use crate::error::{CoreResult, SentinelError};
use crate::model::{Platform, ProcessIdentity, ProcessInfo, ProcessStatus, TerminateMethod};
use crate::util::{format_bytes, guard_process_target};

fn label(info: &ProcessInfo) -> String {
    format!("{} (PID {})", info.name, info.pid)
}

/// Live process for `target`, refusing if the PID now belongs to a different process.
fn resolve(service: &ActionService, target: &ProcessIdentity) -> CoreResult<ProcessInfo> {
    guard_process_target(target.pid)?;
    let info = service.context.lookup_process(target.pid)?;
    if info.start_time != target.start_time {
        return Err(SentinelError::ProcessChanged { pid: target.pid });
    }
    Ok(info)
}

fn os_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "macOS",
        Platform::Windows => "Windows",
        Platform::Linux => "Linux",
    }
}

fn is_system_process(info: &ProcessInfo, platform: Platform) -> bool {
    let user = info.user.as_deref().unwrap_or_default();
    let exe = info.exe.as_deref().unwrap_or_default();
    match platform {
        Platform::Macos => {
            user == "root"
                || user.starts_with('_')
                || ["/System/", "/usr/libexec/", "/usr/sbin/", "/sbin/"]
                    .iter()
                    .any(|prefix| exe.starts_with(prefix))
        }
        Platform::Linux => {
            user == "root"
                || ["/usr/lib/systemd/", "/lib/systemd/", "/sbin/"]
                    .iter()
                    .any(|p| exe.starts_with(p))
        }
        Platform::Windows => {
            let lower = exe.to_ascii_lowercase();
            ["SYSTEM", "LOCAL SERVICE", "NETWORK SERVICE"]
                .iter()
                .any(|account| {
                    user.eq_ignore_ascii_case(account) || user.ends_with(&format!("\\{account}"))
                })
                || lower.starts_with(r"c:\windows\")
        }
    }
}

fn owner_warnings(service: &ActionService, info: &ProcessInfo, warnings: &mut Vec<String>) -> bool {
    let own_user = service
        .context
        .lookup_process(std::process::id())
        .ok()
        .and_then(|own| own.user);
    let foreign = match (&info.user, &own_user) {
        (Some(owner), Some(own)) => owner != own,
        (None, _) => true,
        _ => false,
    };
    let platform = service.config.platform;
    if is_system_process(info, platform) {
        warnings.push(format!(
            "System process — {} relies on it and may become unstable until it restarts.",
            os_name(platform)
        ));
    }
    if foreign && !service.config.running_elevated {
        warnings.push(format!(
            "Owned by {} — Sentinel is not running as administrator, so the system will likely \
             refuse this.",
            info.user.as_deref().unwrap_or("another user")
        ));
        return true;
    }
    false
}

fn process_target(info: &ProcessInfo) -> PreviewTarget {
    let detail = info
        .exe
        .clone()
        .or_else(|| (!info.cmd.is_empty()).then(|| info.cmd.join(" ")))
        .map(|text| {
            if text.chars().count() > 240 {
                format!("{}…", text.chars().take(240).collect::<String>())
            } else {
                text
            }
        });
    PreviewTarget {
        label: label(info),
        detail,
        size_bytes: None,
        problem: None,
    }
}

pub(super) fn preview_stop(
    service: &ActionService,
    target: ProcessIdentity,
    force: bool,
) -> CoreResult<Draft> {
    let info = resolve(service, &target)?;
    let name = label(&info);
    let mut warnings = Vec::new();
    let requires_elevation = owner_warnings(service, &info, &mut warnings);
    let platform = service.config.platform;
    let has_window = !force && service.process_control.has_window(target.pid);

    let (title, description, risk) = if force {
        warnings.push(format!("Unsaved work in {} will be lost.", info.name));
        let how = match platform {
            Platform::Windows => "TerminateProcess",
            _ => "SIGKILL",
        };
        (
            format!("Force kill {name}"),
            format!(
                "Ends {name} immediately with {how}. The process gets no chance to save work or \
                 clean up."
            ),
            ActionRisk::High,
        )
    } else if has_window {
        let description = match platform {
            Platform::Macos => format!(
                "Asks {name} to quit, the same as choosing Quit from its menu. It may ask you to \
                 save changes first."
            ),
            _ => format!(
                "Sends a close request to {name}'s windows. It may ask you to save changes first."
            ),
        };
        (format!("Quit {name}"), description, ActionRisk::Moderate)
    } else if platform == Platform::Windows {
        warnings.push(
            "Windows cannot ask a background process to exit; it will be ended immediately, like a \
             force kill."
                .to_owned(),
        );
        (
            format!("End {name}"),
            format!("Ends {name} with TerminateProcess because it has no windows to close."),
            ActionRisk::High,
        )
    } else {
        (
            format!("Terminate {name}"),
            format!(
                "Sends SIGTERM to {name}, asking it to exit. Well-behaved programs save state and \
                 shut down cleanly."
            ),
            ActionRisk::Moderate,
        )
    };

    Ok(Draft {
        action: if force {
            Action::ForceKillProcess { target }
        } else {
            Action::TerminateProcess { target }
        },
        title,
        description,
        targets: vec![process_target(&info)],
        impact: vec![
            metric(
                "memoryRss",
                "Memory in use",
                info.memory_rss as f64,
                MetricUnit::Bytes,
            ),
            metric(
                "cpuPercentAvg",
                "Average CPU",
                f64::from(info.cpu_percent_avg),
                MetricUnit::Percent,
            ),
        ],
        estimated_bytes_freed: None,
        risk,
        reversibility: Reversibility::Irreversible,
        warnings,
        requires_elevation,
    })
}

pub(super) fn preview_priority(
    service: &ActionService,
    target: ProcessIdentity,
    nice: i32,
) -> CoreResult<Draft> {
    if !(-20..=19).contains(&nice) {
        return Err(SentinelError::invalid(format!(
            "nice value {nice} is outside the supported range -20 (highest priority) to 19 (lowest)"
        )));
    }
    let info = resolve(service, &target)?;
    let name = label(&info);
    let mut warnings = Vec::new();
    let mut requires_elevation = owner_warnings(service, &info, &mut warnings);
    let current = info.nice;
    let raising = current.is_some_and(|c| nice < c);
    if raising && service.config.platform != Platform::Windows && !service.config.running_elevated {
        warnings.push(
            "Raising priority requires administrator privileges; the system will refuse unless \
             Sentinel runs as administrator."
                .to_owned(),
        );
        requires_elevation = true;
    }
    if service.config.platform == Platform::Windows {
        warnings.push(format!(
            "Windows uses priority classes; nice {nice} maps to the {} class.",
            windows_class_name(nice)
        ));
    }
    let direction = match current {
        Some(c) if nice < c => "Raises",
        Some(c) if nice > c => "Lowers",
        _ => "Sets",
    };
    let mut impact = Vec::new();
    if let Some(current) = current {
        impact.push(metric(
            "niceBefore",
            "Current nice",
            f64::from(current),
            MetricUnit::Nice,
        ));
    }
    impact.push(metric(
        "niceAfter",
        "New nice",
        f64::from(nice),
        MetricUnit::Nice,
    ));
    Ok(Draft {
        action: Action::SetProcessPriority { target, nice },
        title: format!("Change priority of {name} to nice {nice}"),
        description: format!(
            "{direction} the scheduling priority of {name} ({}). Lower nice values get more CPU \
             time when the system is busy.",
            match current {
                Some(c) => format!("nice {c} → {nice}"),
                None => format!("new nice {nice}"),
            }
        ),
        targets: vec![process_target(&info)],
        impact,
        estimated_bytes_freed: None,
        risk: ActionRisk::Moderate,
        reversibility: match current {
            Some(c) => Reversibility::Undoable {
                how: format!("Set the priority of {name} back to nice {c}"),
            },
            None => Reversibility::Undoable {
                how: format!("Set the priority of {name} back to nice 0"),
            },
        },
        warnings,
        requires_elevation,
    })
}

fn windows_class_name(nice: i32) -> &'static str {
    match nice {
        i32::MIN..=-11 => "High",
        -10..=-1 => "Above normal",
        0 => "Normal",
        1..=10 => "Below normal",
        _ => "Idle",
    }
}

/// Polls until the process is gone (or became a zombie / the PID was reused).
fn wait_for_exit(service: &ActionService, target: &ProcessIdentity, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match service.context.lookup_process(target.pid) {
            Err(SentinelError::ProcessNotFound { .. }) => return true,
            Ok(info)
                if info.start_time != target.start_time
                    || matches!(info.status, ProcessStatus::Zombie | ProcessStatus::Dead) =>
            {
                return true;
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn execute_stop(
    service: &ActionService,
    preview: &ActionPreview,
    target: &ProcessIdentity,
    force: bool,
) -> Execution {
    let target_label = preview
        .targets
        .first()
        .map(|t| t.label.clone())
        .unwrap_or_else(|| format!("PID {}", target.pid));
    let info = match resolve(service, target) {
        Ok(info) => info,
        Err(err) => return Execution::failed(target_label, err, Vec::new()),
    };
    let before = vec![
        metric("processRunning", "Process running", 1.0, MetricUnit::Count),
        metric(
            "memoryRss",
            "Memory in use",
            info.memory_rss as f64,
            MetricUnit::Bytes,
        ),
    ];
    let result = if force {
        service.process_control.force_kill(target).map(|_| None)
    } else {
        service.process_control.terminate(target).map(Some)
    };
    let method = match result {
        Ok(method) => method,
        Err(err) => return Execution::failed(target_label, err, before),
    };
    let timeout = if force {
        Duration::from_secs(2)
    } else {
        Duration::from_secs(5)
    };
    let exited = wait_for_exit(service, target, timeout);
    let after = vec![metric(
        "processRunning",
        "Process running",
        if exited { 0.0 } else { 1.0 },
        MetricUnit::Count,
    )];
    let freed = format_bytes(info.memory_rss);
    let summary = match (method, exited) {
        (None, true) => format!("Force killed {target_label}; it released {freed} of memory."),
        (None, false) => format!(
            "Sent a force kill to {target_label}, but it is still listed (it may be stuck in the \
             kernel)."
        ),
        (Some(TerminateMethod::WindowClose), true) => {
            format!("{target_label} quit and released {freed} of memory.")
        }
        (Some(TerminateMethod::WindowClose), false) => format!(
            "Asked {target_label} to quit; it is still running and may be waiting for you to save \
             changes."
        ),
        (Some(TerminateMethod::Signal), true) => {
            format!("Terminated {target_label} with SIGTERM; it released {freed} of memory.")
        }
        (Some(TerminateMethod::Signal), false) => format!(
            "Sent SIGTERM to {target_label}; it has not exited yet. Use force kill if it does not \
             respond."
        ),
        (Some(TerminateMethod::TerminateProcess), true) => {
            format!("Ended {target_label}; it released {freed} of memory.")
        }
        (Some(TerminateMethod::TerminateProcess), false) => {
            format!("Ended {target_label}, but it is still listed while Windows cleans it up.")
        }
    };
    Execution {
        status: OutcomeStatus::Succeeded,
        items: vec![ItemOutcome {
            label: target_label,
            success: true,
            error: None,
        }],
        before,
        after,
        summary,
        bytes_freed: None,
        affected_paths: Vec::new(),
    }
}

pub(super) fn execute_priority(
    service: &ActionService,
    preview: &ActionPreview,
    target: &ProcessIdentity,
    nice: i32,
) -> Execution {
    let target_label = preview
        .targets
        .first()
        .map(|t| t.label.clone())
        .unwrap_or_else(|| format!("PID {}", target.pid));
    let info = match resolve(service, target) {
        Ok(info) => info,
        Err(err) => return Execution::failed(target_label, err, Vec::new()),
    };
    let before: Vec<_> = info
        .nice
        .map(|n| metric("nice", "Nice", f64::from(n), MetricUnit::Nice))
        .into_iter()
        .collect();
    if let Err(err) = service.process_control.set_priority(target, nice) {
        return Execution::failed(target_label, err, before);
    }
    let measured = service
        .context
        .lookup_process(target.pid)
        .ok()
        .and_then(|info| info.nice);
    let after: Vec<_> = measured
        .map(|n| metric("nice", "Nice", f64::from(n), MetricUnit::Nice))
        .into_iter()
        .collect();
    let summary = match (info.nice, measured) {
        (Some(from), Some(to)) => {
            format!("Changed priority of {target_label} from nice {from} to nice {to}.")
        }
        (_, Some(to)) => format!("{target_label} now runs at nice {to}."),
        _ => format!("Requested nice {nice} for {target_label}."),
    };
    Execution {
        status: OutcomeStatus::Succeeded,
        items: vec![ItemOutcome {
            label: target_label,
            success: true,
            error: None,
        }],
        before,
        after,
        summary,
        bytes_freed: None,
        affected_paths: Vec::new(),
    }
}
