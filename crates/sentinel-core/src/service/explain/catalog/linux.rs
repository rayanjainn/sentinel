//! Linux catalog.

use super::{Entry, critical, security, service, shell, shell_ui};
use crate::model::{Confidence, ProcessCategory, ProcessRole, QuitSafety};
use crate::service::explain::Draft;
use crate::service::explain::process::{ProcessFacts, display_name};

pub(super) const ENTRIES: &[Entry] = &[
    critical(
        "systemd-journald",
        "System log",
        "Collects the log messages from the kernel and every service. Stopping it loses log \
         messages written while it is gone.",
    ),
    critical(
        "systemd-udevd",
        "Device manager",
        "Notices hardware being added or removed and sets up the device files for it.",
    ),
    critical(
        "systemd-logind",
        "Login sessions",
        "Tracks who is logged in, on which seat, and handles power-button and lid events.",
    ),
    critical(
        "dbus-daemon",
        "Message bus",
        "Carries messages between desktop programs and system services. Most of the desktop depends \
         on it.",
    ),
    critical(
        "dbus-broker",
        "Message bus",
        "Carries messages between desktop programs and system services. Most of the desktop depends \
         on it.",
    ),
    critical(
        "init",
        "First process",
        "The first process the system starts; it launches and supervises everything else.",
    ),
    service(
        "systemd-resolved",
        "Website name lookups",
        "Looks up website and host names for every program on this system.",
        Some("looking up website names"),
    ),
    service(
        "systemd-timesyncd",
        "Clock sync",
        "Keeps the clock correct by checking time servers.",
        Some("automatic clock correction"),
    ),
    service(
        "systemd-oomd",
        "Out-of-memory protection",
        "Watches memory pressure and stops the worst offender before the whole system stalls.",
        Some("out-of-memory protection"),
    ),
    service(
        "NetworkManager",
        "Network connections",
        "Connects to Wi-Fi and wired networks and keeps them up.",
        Some("network connections"),
    ),
    service(
        "wpa_supplicant",
        "Wi-Fi authentication",
        "Handles joining and authenticating with Wi-Fi networks.",
        Some("Wi-Fi"),
    ),
    service(
        "ModemManager",
        "Mobile broadband",
        "Manages mobile broadband and modem hardware.",
        Some("mobile broadband"),
    ),
    service(
        "pipewire",
        "Sound and screen sharing",
        "Handles audio and video streams for every app, including screen sharing.",
        Some("all sound"),
    ),
    service(
        "wireplumber",
        "Sound routing",
        "Decides which app's sound goes to which device, on top of PipeWire.",
        Some("sound routing"),
    ),
    service(
        "pulseaudio",
        "Sound",
        "Mixes and routes audio for every app.",
        Some("all sound"),
    ),
    service(
        "cupsd",
        "Printing",
        "Accepts print jobs and sends them to printers.",
        Some("printing"),
    ),
    service(
        "cups-browsed",
        "Finding printers",
        "Finds printers shared on the network.",
        Some("finding network printers"),
    ),
    service(
        "avahi-daemon",
        "Finding devices on your network",
        "Announces this machine and finds printers, speakers and other devices on the local \
         network.",
        Some("finding local devices"),
    ),
    service(
        "udisksd",
        "Disks and USB drives",
        "Mounts disks and USB drives and reports their status.",
        Some("mounting disks"),
    ),
    service(
        "upowerd",
        "Battery and power",
        "Reports battery level and power-source changes to the desktop.",
        Some("battery reporting"),
    ),
    service(
        "packagekitd",
        "Software installation",
        "Installs and updates packages on behalf of the desktop's software tools.",
        Some("installing software from the desktop"),
    ),
    service(
        "snapd",
        "Snap packages",
        "Runs and updates software installed as snaps.",
        Some("snap apps and their updates"),
    ),
    service(
        "cron",
        "Scheduled jobs",
        "Runs commands at the times listed in the system's schedules.",
        Some("scheduled jobs"),
    ),
    service(
        "crond",
        "Scheduled jobs",
        "Runs commands at the times listed in the system's schedules.",
        Some("scheduled jobs"),
    ),
    service(
        "rsyslogd",
        "System log",
        "Writes log messages to files and forwards them if configured.",
        Some("log files"),
    ),
    service(
        "containerd",
        "Container runtime",
        "Runs and supervises containers for Docker and other tools.",
        Some("running containers"),
    ),
    service(
        "dockerd",
        "Docker",
        "Builds and runs Docker containers.",
        Some("Docker containers"),
    ),
    service(
        "irqbalance",
        "Interrupt balancing",
        "Spreads hardware interrupts across CPU cores.",
        Some("interrupt balancing"),
    ),
    security(
        "polkitd",
        "Authorization prompts",
        "Decides which actions need a password and shows those prompts.",
        QuitSafety::OsService,
    ),
    security(
        "sshd",
        "Remote login server",
        "Accepts incoming SSH logins from other machines. If you did not mean to allow remote \
         logins, this is the service to look at.",
        QuitSafety::OsService,
    ),
    security(
        "auditd",
        "Security auditing",
        "Records security-relevant events for later review.",
        QuitSafety::OsService,
    ),
    security(
        "firewalld",
        "Firewall",
        "Applies the system's firewall rules.",
        QuitSafety::OsService,
    ),
    shell_ui(
        "gnome-shell",
        "GNOME desktop",
        "Draws the desktop, top bar, activities view and every window frame.",
        Some("the desktop itself, until it restarts"),
    ),
    shell_ui(
        "plasmashell",
        "KDE Plasma desktop",
        "Draws the desktop, panels and widgets.",
        Some("the desktop panels"),
    ),
    shell_ui(
        "Xorg",
        "Display server",
        "Draws windows on screen and delivers keyboard and mouse input to them.",
        Some("the graphical session"),
    ),
    shell_ui(
        "Xwayland",
        "Support for older apps",
        "Lets programs written for X11 run inside a Wayland desktop.",
        Some("older X11 apps"),
    ),
    shell_ui(
        "gdm",
        "Login screen",
        "Shows the graphical login screen and starts your session.",
        Some("the login screen"),
    ),
    shell_ui(
        "sddm",
        "Login screen",
        "Shows the graphical login screen and starts your session.",
        Some("the login screen"),
    ),
    shell_ui(
        "gnome-keyring-daemon",
        "Saved passwords",
        "Holds your saved passwords and keys while you are logged in.",
        Some("saved passwords"),
    ),
    shell_ui(
        "tracker-miner-fs",
        "File search index",
        "Indexes your files so desktop search can find them.",
        Some("desktop file search"),
    ),
    shell(
        "bash",
        "bash — command line shell",
        "A shell waiting for or running commands you typed in a terminal.",
    ),
    shell(
        "zsh",
        "zsh — command line shell",
        "A shell waiting for or running commands you typed in a terminal.",
    ),
    shell(
        "fish",
        "fish — command line shell",
        "A shell waiting for or running commands you typed in a terminal.",
    ),
    shell(
        "sh",
        "sh — command line shell",
        "A small shell, often started by a script rather than by you.",
    ),
];

pub(super) fn by_location(facts: &ProcessFacts<'_>) -> Option<Draft> {
    // Kernel threads have no executable at all and are shown in brackets by other tools.
    if facts.exe.is_none() && facts.name.starts_with('[') {
        return Some(Draft::new(
            format!("{} — kernel thread", facts.name.trim_matches(['[', ']'])),
            "Part of the Linux kernel rather than a program on disk. It has no executable path for \
             that reason.",
            ProcessRole::Kernel,
            ProcessCategory::Kernel,
            QuitSafety::SystemCritical,
        ));
    }
    let exe = facts.exe?;
    if exe.starts_with("/usr/lib/systemd/") || exe.starts_with("/lib/systemd/") {
        return Some(
            Draft::new(
                format!("{} — system service", display_name(facts)),
                "Part of systemd, which starts and supervises the system's services. Sentinel does \
                 not recognise this particular one."
                    .to_owned(),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        );
    }
    if exe.starts_with("/usr/sbin/") || exe.starts_with("/sbin/") {
        return Some(
            Draft::new(
                format!("{} — system program", display_name(facts)),
                "Runs from a folder reserved for system administration programs. Sentinel does not \
                 recognise it, so it will not describe what it does."
                    .to_owned(),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        );
    }
    if exe.starts_with("/usr/bin/") || exe.starts_with("/bin/") || exe.starts_with("/usr/libexec/")
    {
        return Some(
            Draft::new(
                format!("{} — installed program", display_name(facts)),
                "Part of the software installed by this system's package manager. Sentinel does not \
                 recognise the program itself."
                    .to_owned(),
                ProcessRole::Unknown,
                ProcessCategory::Unknown,
                QuitSafety::Background,
            )
            .confidence(Confidence::Likely),
        );
    }
    if exe.starts_with("/snap/")
        || exe.starts_with("/var/lib/flatpak/")
        || exe.contains("/.local/share/flatpak/")
    {
        return Some(
            Draft::new(
                format!("{} — sandboxed app", display_name(facts)),
                "Installed as a snap or Flatpak, which run apps in a sandbox. Sentinel does not \
                 recognise the app itself."
                    .to_owned(),
                ProcessRole::MainApp,
                ProcessCategory::UserApp,
                QuitSafety::UserApp,
            )
            .confidence(Confidence::Likely),
        );
    }
    None
}
