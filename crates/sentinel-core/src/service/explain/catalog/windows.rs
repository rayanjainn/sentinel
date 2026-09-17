//! Windows catalog, including resolving the service a `svchost.exe` is hosting.

use super::{Entry, critical, security, service, shell, shell_ui};
use crate::model::{Confidence, ProcessCategory, ProcessRole, QuitSafety};
use crate::service::explain::Draft;
use crate::service::explain::process::{ProcessFacts, display_name, file_name, stem};

pub(super) const ENTRIES: &[Entry] = &[
    critical(
        "csrss",
        "Windows session support",
        "Runs core parts of each Windows session. Windows stops if it is ended.",
    ),
    critical(
        "wininit",
        "Windows startup",
        "Starts the services and session infrastructure when Windows boots.",
    ),
    critical(
        "winlogon",
        "Sign-in and session",
        "Handles signing in, locking and switching users.",
    ),
    critical(
        "services",
        "Service manager",
        "Starts, stops and supervises every Windows service.",
    ),
    critical(
        "smss",
        "Session manager",
        "Creates each Windows session at startup.",
    ),
    critical(
        "dwm",
        "Draws the desktop",
        "Composites every window, animation and transparency effect you see.",
    ),
    security(
        "lsass",
        "Sign-in and security",
        "Checks passwords, applies security policy and stores credentials. Ending it signs you out \
         or restarts Windows.",
        QuitSafety::SystemCritical,
    ),
    security(
        "LsaIso",
        "Credential protection",
        "Keeps credentials isolated from the rest of Windows (Credential Guard).",
        QuitSafety::SystemCritical,
    ),
    security(
        "MsMpEng",
        "Microsoft Defender antivirus",
        "Scans files and programs as they are used. It is the process that uses CPU during a scan.",
        QuitSafety::OsService,
    ),
    security(
        "NisSrv",
        "Defender network inspection",
        "Watches network traffic for known attacks on behalf of Microsoft Defender.",
        QuitSafety::OsService,
    ),
    security(
        "SecurityHealthService",
        "Windows Security status",
        "Reports the state of antivirus, firewall and account protection in Windows Security.",
        QuitSafety::OsService,
    ),
    shell_ui(
        "explorer",
        "Desktop, taskbar and File Explorer",
        "Draws the taskbar, Start menu and desktop, and shows your files.",
        Some("the taskbar and desktop, until Windows restarts it"),
    ),
    shell_ui(
        "ShellExperienceHost",
        "Parts of the Windows shell",
        "Draws pieces of the Start menu, notification area and other shell surfaces.",
        Some("parts of the Start menu"),
    ),
    shell_ui(
        "StartMenuExperienceHost",
        "Start menu",
        "Draws the Start menu.",
        Some("the Start menu"),
    ),
    shell_ui(
        "SearchHost",
        "Windows Search box",
        "Draws the search box and results next to Start.",
        Some("the search box"),
    ),
    shell_ui(
        "sihost",
        "Shell infrastructure",
        "Runs shell features your session needs, such as notification handling.",
        Some("some shell features"),
    ),
    shell_ui(
        "ctfmon",
        "Text input",
        "Handles keyboard layouts, handwriting and speech input.",
        Some("alternative text input"),
    ),
    service(
        "SearchIndexer",
        "Windows Search indexing",
        "Builds the index behind Windows Search. It works hardest after many new files appear.",
        Some("Windows Search"),
    ),
    service(
        "RuntimeBroker",
        "App permission broker",
        "Checks that a Store app only reaches what you allowed. One runs per app.",
        Some("permission checks for that app"),
    ),
    service(
        "fontdrvhost",
        "Font handling",
        "Loads and rasterises fonts away from the kernel for safety.",
        Some("font rendering"),
    ),
    service(
        "audiodg",
        "Sound processing",
        "Applies audio effects and mixing. It is separate so a faulty effect cannot crash Windows \
         audio.",
        Some("sound effects and mixing"),
    ),
    service(
        "spoolsv",
        "Printing",
        "Queues documents and sends them to printers.",
        Some("printing"),
    ),
    service(
        "taskhostw",
        "Scheduled tasks",
        "Hosts scheduled tasks that run as a DLL rather than their own program.",
        Some("some scheduled tasks"),
    ),
    service(
        "dllhost",
        "COM component host",
        "Hosts a Windows component on behalf of another program, such as a thumbnail generator.",
        Some("whatever component it was hosting"),
    ),
    service(
        "WmiPrvSE",
        "Management queries",
        "Answers WMI queries about this PC's hardware and configuration.",
        Some("management and inventory queries"),
    ),
    service(
        "conhost",
        "Console window",
        "Draws the window for a command-line program.",
        Some("that console window"),
    ),
    service(
        "WUDFHost",
        "User-mode driver host",
        "Runs a device driver outside the kernel so a fault cannot take Windows down.",
        Some("that device"),
    ),
    service(
        "TrustedInstaller",
        "Windows component installer",
        "Installs and repairs Windows components and updates.",
        Some("installing Windows updates"),
    ),
    service(
        "MoUsoCoreWorker",
        "Update orchestrator",
        "Coordinates checking for, downloading and installing Windows updates.",
        Some("Windows Update"),
    ),
    service(
        "SgrmBroker",
        "System integrity monitor",
        "Watches for tampering with Windows' own protections.",
        Some("runtime integrity monitoring"),
    ),
    shell(
        "cmd",
        "Command Prompt",
        "A command line waiting for or running commands you typed.",
    ),
    shell(
        "powershell",
        "PowerShell",
        "A PowerShell session waiting for or running commands.",
    ),
    shell(
        "pwsh",
        "PowerShell",
        "A PowerShell session waiting for or running commands.",
    ),
];

/// Plain names for the Windows services most often seen inside `svchost.exe`.
const SERVICES: &[(&str, &str, &str)] = &[
    (
        "Dnscache",
        "DNS client",
        "Looks up website names for every program and caches the answers.",
    ),
    (
        "Dhcp",
        "DHCP client",
        "Gets this PC's network address from the router.",
    ),
    ("W32Time", "Windows Time", "Keeps the clock correct."),
    (
        "EventLog",
        "Windows Event Log",
        "Collects the logs Event Viewer shows.",
    ),
    (
        "Schedule",
        "Task Scheduler",
        "Runs tasks at the times they are scheduled for.",
    ),
    (
        "Themes",
        "Themes",
        "Applies the visual style to windows and controls.",
    ),
    (
        "Audiosrv",
        "Windows Audio",
        "Plays sound for every program.",
    ),
    (
        "AudioEndpointBuilder",
        "Audio device manager",
        "Detects speakers, headphones and microphones.",
    ),
    (
        "BITS",
        "Background transfers",
        "Downloads updates and app data quietly in the background.",
    ),
    (
        "wuauserv",
        "Windows Update",
        "Checks for and installs Windows updates.",
    ),
    (
        "WSearch",
        "Windows Search",
        "Indexes files so search can find them.",
    ),
    ("Spooler", "Print spooler", "Queues documents for printers."),
    (
        "LanmanServer",
        "File and printer sharing",
        "Shares this PC's files and printers with others.",
    ),
    (
        "LanmanWorkstation",
        "Network file access",
        "Opens files on other computers' shared folders.",
    ),
    (
        "TermService",
        "Remote Desktop",
        "Accepts incoming Remote Desktop connections.",
    ),
    (
        "CryptSvc",
        "Cryptographic services",
        "Manages certificates and verifies signatures.",
    ),
    (
        "BFE",
        "Firewall engine",
        "Applies firewall and network filtering rules.",
    ),
    (
        "MpsSvc",
        "Windows Defender Firewall",
        "Blocks and allows network traffic by rule.",
    ),
    (
        "Winmgmt",
        "Windows Management",
        "Answers questions about this PC's configuration.",
    ),
    (
        "ProfSvc",
        "User profiles",
        "Loads and saves your user profile at sign-in and sign-out.",
    ),
    (
        "SysMain",
        "SysMain",
        "Preloads programs you use often to make them start faster.",
    ),
    (
        "RpcSs",
        "Remote Procedure Call",
        "Lets Windows components talk to each other. Nearly everything depends on it.",
    ),
    (
        "DcomLaunch",
        "DCOM server launcher",
        "Starts Windows components when they are needed.",
    ),
    ("WlanSvc", "Wi-Fi", "Finds and connects to Wi-Fi networks."),
    (
        "Netman",
        "Network connections",
        "Manages the list of network connections.",
    ),
    (
        "netprofm",
        "Network list",
        "Tracks which networks this PC has joined.",
    ),
    (
        "iphlpsvc",
        "IP helper",
        "Provides IPv6 transition and tunnelling support.",
    ),
    (
        "DiagTrack",
        "Diagnostics and telemetry",
        "Collects the diagnostic data Windows sends to Microsoft.",
    ),
    (
        "WpnService",
        "Push notifications",
        "Delivers notifications to apps.",
    ),
    (
        "TokenBroker",
        "Sign-in tokens",
        "Holds the sign-in tokens apps use for Microsoft accounts.",
    ),
    (
        "StateRepository",
        "App state database",
        "Stores the state of installed apps.",
    ),
    (
        "StorSvc",
        "Storage service",
        "Reports disk space and storage settings.",
    ),
    (
        "TimeBrokerSvc",
        "Background task timer",
        "Wakes background app tasks at the right time.",
    ),
    (
        "UsoSvc",
        "Update Orchestrator",
        "Schedules the steps of a Windows update.",
    ),
    (
        "CDPSvc",
        "Connected devices",
        "Links this PC with your other devices for shared features.",
    ),
    (
        "DeviceAssociationService",
        "Device pairing",
        "Pairs and remembers devices such as Bluetooth accessories.",
    ),
    (
        "UserManager",
        "User session manager",
        "Sets up each signed-in user's session.",
    ),
    (
        "ShellHWDetection",
        "Hardware events",
        "Notices inserted disks and media and reacts to them.",
    ),
    (
        "SENS",
        "System event notification",
        "Tells programs about sign-in, network and power events.",
    ),
    (
        "WinHttpAutoProxySvc",
        "Proxy discovery",
        "Works out which proxy server to use.",
    ),
    (
        "nsi",
        "Network store",
        "Keeps network configuration for other components to read.",
    ),
    (
        "PlugPlay",
        "Plug and Play",
        "Detects new hardware and sets it up.",
    ),
    (
        "Power",
        "Power management",
        "Applies power plans and battery behaviour.",
    ),
];

/// `svchost.exe -k <group> -p -s <ServiceName>`: the command line names the hosted service exactly.
pub(super) fn svchost(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let name = file_name(facts.exe).map(stem).unwrap_or(stem(facts.name));
    if !name.eq_ignore_ascii_case("svchost") {
        return None;
    }
    let hosted = arg_after(facts, "-s");
    let group = arg_after(facts, "-k");
    let Some(hosted) = hosted else {
        let detail = match group {
            Some(group) => format!(
                "Windows runs many of its services inside svchost.exe. This one hosts the \"{group}\" \
                 group but does not name a single service on its command line, so Sentinel cannot \
                 say which service it is."
            ),
            None => String::from(
                "Windows runs many of its services inside svchost.exe. This one does not name a \
                 service on its command line, so Sentinel cannot say which service it is.",
            ),
        };
        return Some(
            Draft::new(
                "Windows service host",
                detail,
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Unknown),
        );
    };
    match SERVICES
        .iter()
        .find(|(key, _, _)| key.eq_ignore_ascii_case(hosted))
    {
        Some((key, label, detail)) => {
            let safety = if matches!(*key, "RpcSs" | "DcomLaunch" | "PlugPlay") {
                QuitSafety::SystemCritical
            } else {
                QuitSafety::OsService
            };
            let mut draft = Draft::new(
                format!("{label} — Windows service"),
                format!("{detail} It runs inside svchost.exe, which hosts many Windows services."),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                safety,
            );
            if safety == QuitSafety::OsService {
                draft = draft.stops(label.to_string());
            }
            Some(draft)
        }
        None => Some(
            Draft::new(
                format!("Windows service host — {hosted}"),
                format!(
                    "Windows runs many of its services inside svchost.exe. Its command line says \
                     this one hosts the service named \"{hosted}\", which Sentinel does not \
                     recognise, so it will not describe what that service does."
                ),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        ),
    }
}

fn arg_after<'a>(facts: &ProcessFacts<'a>, flag: &str) -> Option<&'a str> {
    let mut args = facts.cmd.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        if arg.eq_ignore_ascii_case(flag) {
            return args.next();
        }
    }
    None
}

pub(super) fn by_location(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let exe = facts.exe?;
    let lower = exe.to_ascii_lowercase();
    if lower.contains(r"\windows\system32\") || lower.contains(r"\windows\syswow64\") {
        return Some(
            Draft::new(
                format!("{} — Windows system program", display_name(facts)),
                "Part of Windows itself: it runs from the Windows system folder. Sentinel does not \
                 recognise this particular program, so it will not describe what it does."
                    .to_owned(),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        );
    }
    if lower.contains(r"\windows\") {
        return Some(
            Draft::new(
                format!("{} — part of Windows", display_name(facts)),
                "Runs from inside the Windows folder. Sentinel does not recognise it, so it will \
                 not describe what it does."
                    .to_owned(),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        );
    }
    if lower.contains(r"\program files") {
        return Some(
            Draft::new(
                format!("{} — installed program", display_name(facts)),
                "An application installed on this PC. Sentinel does not recognise it, so it will \
                 not describe what it does."
                    .to_owned(),
                ProcessRole::MainApp,
                ProcessCategory::UserApp,
                QuitSafety::UserApp,
            )
            .confidence(Confidence::Likely),
        );
    }
    if lower.contains(r"\appdata\") {
        return Some(
            Draft::new(
                format!("{} — program installed for your account", display_name(facts)),
                "Installed under your own user folder rather than for the whole PC, which is normal \
                 for apps that update themselves. Sentinel does not recognise it."
                    .to_owned(),
                ProcessRole::Unknown,
                ProcessCategory::Unknown,
                QuitSafety::Background,
            )
            .confidence(Confidence::Unknown),
        );
    }
    None
}
