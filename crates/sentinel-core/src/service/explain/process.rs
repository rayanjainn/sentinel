//! Process classifier: path, flags, parent chain, user and a per-OS catalog in, one plain-English
//! explanation out.

use super::{Draft, catalog, chromium, interpreter};
use crate::model::{
    Confidence, Pid, Platform, ProcessCategory, ProcessExplanation, ProcessInfo, ProcessRole,
    QuitSafety,
};

/// Everything the classifier is allowed to look at. No syscalls, no network, no file reads.
pub struct ProcessFacts<'a> {
    pub pid: Pid,
    /// Process name as the OS reports it (can be truncated on some platforms).
    pub name: &'a str,
    pub exe: Option<&'a str>,
    pub cmd: &'a [String],
    pub user: Option<&'a str>,
    pub platform: Platform,
    pub parent: Option<ParentFacts<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ParentFacts<'a> {
    pub pid: Pid,
    pub name: &'a str,
    pub exe: Option<&'a str>,
}

/// The OS Sentinel is running on. The classifier itself takes the platform as a parameter so the
/// Windows and Linux catalogs are testable from any host.
pub const fn host_platform() -> Platform {
    #[cfg(target_os = "macos")]
    {
        Platform::Macos
    }
    #[cfg(target_os = "linux")]
    {
        Platform::Linux
    }
    #[cfg(windows)]
    {
        Platform::Windows
    }
}

pub fn facts_from<'a>(
    info: &'a ProcessInfo,
    parent: Option<&'a ProcessInfo>,
    platform: Platform,
) -> ProcessFacts<'a> {
    ProcessFacts {
        pid: info.pid,
        name: &info.name,
        exe: info.exe.as_deref(),
        cmd: &info.cmd,
        user: info.user.as_deref(),
        platform,
        parent: parent.map(|p| ParentFacts {
            pid: p.pid,
            name: &p.name,
            exe: p.exe.as_deref(),
        }),
    }
}

pub fn explain_info(
    info: &ProcessInfo,
    parent: Option<&ProcessInfo>,
    platform: Platform,
) -> ProcessExplanation {
    explain_process(&facts_from(info, parent, platform))
}

pub fn explain_process(facts: &ProcessFacts<'_>) -> ProcessExplanation {
    let draft = classify(facts);
    let evidence = evidence(facts, &draft);
    let quit_note = quit_note(facts, &draft);
    ProcessExplanation {
        headline: draft.headline,
        detail: draft.detail,
        role: draft.role,
        category: draft.category,
        quit_safety: draft.quit_safety,
        quit_note,
        evidence,
        app_name: draft.app_name,
        confidence: draft.confidence,
    }
}

fn classify(facts: &ProcessFacts<'_>) -> Draft {
    if let Some(draft) = kernel_or_init(facts) {
        return draft;
    }
    if let Some(draft) = chromium::classify(facts) {
        return draft;
    }
    if let Some(draft) = interpreter::classify(facts) {
        return draft;
    }
    if let Some(draft) = catalog::lookup(facts) {
        return draft;
    }
    if let Some(draft) = catalog::by_location(facts) {
        return draft;
    }
    unrecognised(facts)
}

/// PID 0/1 and the kernel are the same idea on every OS but have different names.
fn kernel_or_init(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let name = file_name(facts.exe).unwrap_or(facts.name);
    let os = os_name(facts.platform);
    match (facts.platform, name, facts.pid) {
        (Platform::Macos, "kernel_task", _) => Some(Draft::new(
            "macOS kernel",
            "The core of macOS itself. The CPU time shown here also covers work macOS does to keep \
             the machine cool, so a high number often means the Mac is managing heat.",
            ProcessRole::Kernel,
            ProcessCategory::Kernel,
            QuitSafety::SystemCritical,
        )),
        (Platform::Linux, "kthreadd", _) => Some(Draft::new(
            "Linux kernel threads",
            "The parent of the kernel's own threads. It is part of the kernel, not a program you \
             installed.",
            ProcessRole::Kernel,
            ProcessCategory::Kernel,
            QuitSafety::SystemCritical,
        )),
        (Platform::Windows, "System", _) | (Platform::Windows, "Registry", _) => Some(Draft::new(
            format!("Windows kernel ({name})"),
            "Part of Windows itself, not a program you installed.",
            ProcessRole::Kernel,
            ProcessCategory::Kernel,
            QuitSafety::SystemCritical,
        )),
        (Platform::Macos, "launchd", 1) => Some(Draft::new(
            "launchd — starts everything else",
            "The first process macOS starts. It launches and restarts every other service on the \
             Mac.",
            ProcessRole::Daemon,
            ProcessCategory::OsService,
            QuitSafety::SystemCritical,
        )),
        (Platform::Macos, "launchd", _) => Some(Draft::new(
            "launchd — starts your login session's services",
            "One launchd runs per logged-in user and starts that user's background services.",
            ProcessRole::Daemon,
            ProcessCategory::OsService,
            QuitSafety::SystemCritical,
        )),
        (Platform::Linux, "systemd", 1) => Some(Draft::new(
            "systemd — starts everything else",
            "The first process Linux starts. It launches and restarts every other system service.",
            ProcessRole::Daemon,
            ProcessCategory::OsService,
            QuitSafety::SystemCritical,
        )),
        (Platform::Linux, "systemd", _) => Some(Draft::new(
            "systemd — starts your login session's services",
            "A per-user systemd that starts the services belonging to one logged-in user.",
            ProcessRole::Daemon,
            ProcessCategory::OsService,
            QuitSafety::SystemCritical,
        )),
        (_, _, 0) => Some(Draft::new(
            format!("{os} scheduler"),
            format!("PID 0 is part of {os} itself and cannot be stopped."),
            ProcessRole::Kernel,
            ProcessCategory::Kernel,
            QuitSafety::SystemCritical,
        )),
        _ => None,
    }
}

fn unrecognised(facts: &ProcessFacts<'_>) -> Draft {
    let mut detail = String::from(
        "Sentinel does not recognise this program, so it will not describe what it does. The facts \
         below are everything the system reports about it.",
    );
    if let Some(parent) = facts.parent
        && parent.pid > 1
    {
        detail.push_str(&format!(" It was started by {}.", parent.name));
    }
    Draft::new(
        format!("{} — not recognised", display_name(facts)),
        detail,
        ProcessRole::Unknown,
        ProcessCategory::Unknown,
        QuitSafety::Unknown,
    )
    .confidence(Confidence::Unknown)
}

/// The raw facts the explanation was built from, so a reader can check it.
fn evidence(facts: &ProcessFacts<'_>, draft: &Draft) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(flag) = type_flag_evidence(facts.cmd) {
        out.push(flag);
    }
    if let Some(exe) = facts.exe {
        out.push(format!("Runs {exe}"));
        if let Some(app) = &draft.app_name
            && exe.contains(&format!("{app}.app"))
        {
            out.push(format!("Inside {app}.app"));
        }
    } else {
        out.push("The executable path is not readable for this process".to_owned());
    }
    if let Some(parent) = facts.parent {
        out.push(format!("Started by {} (PID {})", parent.name, parent.pid));
    }
    if let Some(user) = facts.user {
        out.push(format!("Runs as the user {user}"));
    }
    out
}

/// Chromium-style `--type=` / `--utility-sub-type=` flags, quoted exactly as they appear.
fn type_flag_evidence(cmd: &[String]) -> Option<String> {
    let flags: Vec<&str> = cmd
        .iter()
        .map(String::as_str)
        .filter(|arg| {
            arg.starts_with("--type=")
                || arg.starts_with("--utility-sub-type=")
                || *arg == "--extension-process"
        })
        .collect();
    (!flags.is_empty()).then(|| format!("Flag {}", flags.join(" ")))
}

/// What actually happens if this process is stopped, and the better alternative when there is one.
fn quit_note(facts: &ProcessFacts<'_>, draft: &Draft) -> String {
    let os = os_name(facts.platform);
    let machine = machine_name(facts.platform);
    match draft.quit_safety {
        QuitSafety::SystemCritical => {
            format!("Don't quit this. {os} needs it; quitting can freeze or restart {machine}.")
        }
        QuitSafety::OsService => {
            let stops = draft.stops.as_deref().map_or_else(
                || {
                    String::from(
                        "Sentinel cannot say which feature stops in the meantime, because it does \
                         not recognise this service.",
                    )
                },
                |stops| format!("{stops} stops until then."),
            );
            format!("Quitting is possible; {os} will start it again. {stops}")
        }
        QuitSafety::AppHelper => {
            let app = draft.app_name.as_deref().unwrap_or("its application");
            let effect = match draft.role {
                ProcessRole::Renderer => {
                    "Quitting closes the page it is drawing; other tabs keep working."
                }
                ProcessRole::Extension => {
                    "Quitting stops that extension; the rest of the browser keeps working."
                }
                ProcessRole::Gpu => {
                    "Quitting makes the app draw its own graphics again, usually after a flicker."
                }
                ProcessRole::NetworkService => {
                    "Quitting interrupts the app's network requests; pages may need reloading."
                }
                ProcessRole::AudioService => {
                    "Quitting stops its sound until the app starts it again."
                }
                _ => "Quitting stops that one piece; the app usually starts it again when needed.",
            };
            format!("This is part of {app}. {effect}")
        }
        QuitSafety::UserApp => String::from(
            "Safe to quit. Unsaved work in it will be lost — quit from the app itself when you can.",
        ),
        QuitSafety::Background => {
            let starter = facts
                .parent
                .filter(|parent| parent.pid > 1)
                .map(|parent| parent.name);
            match starter {
                Some(parent) => format!(
                    "No window, so nothing visible closes. Quitting stops whatever it was doing \
                     for {parent}, which may start it again."
                ),
                None => String::from(
                    "No window, so nothing visible closes. Quitting stops whatever it was doing in \
                     the background; the program that installed it may start it again.",
                ),
            }
        }
        QuitSafety::Unknown => String::from(
            "Sentinel does not recognise this program, so it cannot say what quitting will do. \
             Check the path, the user and the program that started it above first.",
        ),
    }
}

pub(super) fn os_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "macOS",
        Platform::Windows => "Windows",
        Platform::Linux => "Linux",
    }
}

pub(super) fn machine_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "your Mac",
        Platform::Windows => "your PC",
        Platform::Linux => "your computer",
    }
}

/// Best short name for the program: the executable's file name, else the reported name.
pub(super) fn display_name<'a>(facts: &ProcessFacts<'a>) -> &'a str {
    file_name(facts.exe).unwrap_or(facts.name)
}

pub(super) fn file_name(path: Option<&str>) -> Option<&str> {
    let path = path?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    (!name.is_empty()).then_some(name)
}

/// File name without a trailing `.exe`, for Windows catalog lookups.
pub(super) fn stem(name: &str) -> &str {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
}

/// Value of a `--flag=value` style argument.
pub(super) fn flag_value<'a>(cmd: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    cmd.iter().find_map(|arg| arg.strip_prefix(prefix.as_str()))
}

pub(super) fn has_flag(cmd: &[String], flag: &str) -> bool {
    cmd.iter().any(|arg| arg == flag)
}

/// Capitalises the first character, so an executable stem reads as a name in a sentence.
pub(super) fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
