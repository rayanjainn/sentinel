//! macOS catalog. Entries describe what the service does for the user, not how it works.

use super::{Entry, critical, security, service, shell, shell_ui};
use crate::model::{Confidence, ProcessCategory, ProcessRole, QuitSafety};
use crate::service::explain::Draft;
use crate::service::explain::chromium::outermost_bundle;
use crate::service::explain::process::{ProcessFacts, display_name};

pub(super) const ENTRIES: &[Entry] = &[
    critical(
        "WindowServer",
        "Draws everything on screen",
        "Every window, menu and cursor you see is drawn by this one service. Its CPU use rises with \
         animation and the number of displays.",
    ),
    critical(
        "loginwindow",
        "Your login session",
        "Owns the logged-in session and the login screen. It is the parent of the apps you open.",
    ),
    critical(
        "logind",
        "Session manager",
        "Keeps track of who is logged in and starts each session's services.",
    ),
    critical(
        "powerd",
        "Sleep, wake and battery",
        "Decides when the Mac sleeps and wakes, and manages battery and charging behaviour.",
    ),
    critical(
        "configd",
        "Network configuration",
        "Watches for network changes — joining Wi-Fi, plugging in a cable, a new address — and tells \
         the rest of the system about them.",
    ),
    critical(
        "opendirectoryd",
        "User accounts and groups",
        "Answers every question about user accounts, groups and passwords on this Mac.",
    ),
    critical(
        "hidd",
        "Keyboard, trackpad and mouse input",
        "Delivers input from the keyboard, trackpad and mouse to whichever app is in front.",
    ),
    critical(
        "notifyd",
        "Internal message delivery",
        "Passes small notifications between processes. Nearly everything on the Mac depends on it.",
    ),
    critical(
        "distnoted",
        "Internal message delivery",
        "Distributes notifications between apps and services.",
    ),
    critical(
        "cfprefsd",
        "App settings",
        "Reads and writes preferences for apps and services. Every app's settings pass through it.",
    ),
    critical(
        "diskarbitrationd",
        "Disk mounting",
        "Mounts and unmounts disks and volumes as they appear.",
    ),
    critical(
        "watchdogd",
        "Hang detection",
        "Watches for parts of the system that stop responding and restarts them.",
    ),
    critical(
        "kernelmanagerd",
        "Kernel extensions and drivers",
        "Loads the drivers and system extensions the kernel needs.",
    ),
    security(
        "securityd",
        "Keychain and certificates",
        "Guards the keychain and answers cryptography requests from apps.",
        QuitSafety::SystemCritical,
    ),
    security(
        "syspolicyd",
        "App security checks",
        "Checks that apps you open are signed and allowed to run (Gatekeeper).",
        QuitSafety::SystemCritical,
    ),
    security(
        "amfid",
        "Code signature checks",
        "Verifies the signature of every program before macOS lets it run.",
        QuitSafety::SystemCritical,
    ),
    security(
        "tccd",
        "Privacy permissions",
        "Keeps the list of which apps you allowed to reach the camera, microphone, files and screen, \
         and shows those permission prompts.",
        QuitSafety::OsService,
    ),
    security(
        "trustd",
        "Certificate checks",
        "Checks whether the certificates websites and apps present are still valid and trusted.",
        QuitSafety::OsService,
    ),
    // Spotlight and file metadata.
    service(
        "mds",
        "Spotlight indexing",
        "Builds the search index behind Spotlight. It works hardest after a large amount of new \
         data appears, then settles down.",
        Some("Spotlight search"),
    ),
    service(
        "mds_stores",
        "Spotlight index storage",
        "Writes and compacts the Spotlight index on disk.",
        Some("Spotlight search"),
    ),
    service(
        "mdworker",
        "Spotlight file reader",
        "Reads one file at a time to collect what Spotlight needs to index. Several run at once and \
         each exits when it is done.",
        Some("Spotlight indexing of new files"),
    ),
    service(
        "mdworker_shared",
        "Spotlight file reader",
        "Reads files so Spotlight can index their contents. Started as needed and exits when idle.",
        Some("Spotlight indexing of new files"),
    ),
    service(
        "mdbulkimport",
        "Spotlight bulk import",
        "Imports a large batch of files into the Spotlight index at once.",
        Some("Spotlight indexing"),
    ),
    service(
        "corespotlightd",
        "In-app search index",
        "Indexes items apps offer to search — mail, messages, notes — for Spotlight.",
        Some("searching inside apps from Spotlight"),
    ),
    service(
        "spotlightknowledged",
        "Spotlight suggestions",
        "Builds the on-device index behind Spotlight's suggestions.",
        Some("Spotlight suggestions"),
    ),
    service(
        "fseventsd",
        "File change notifications",
        "Records which folders changed so backups, Spotlight and apps can notice without rescanning \
         the disk.",
        Some("file-change notifications used by backups and apps"),
    ),
    service(
        "revisiond",
        "Document version history",
        "Stores the versions of documents that apps offer under Revert To.",
        Some("document version history"),
    ),
    // Backup, iCloud, downloads.
    service(
        "backupd",
        "Time Machine backup",
        "Runs Time Machine backups. During a backup it reads a lot of disk and can use noticeable \
         CPU.",
        Some("Time Machine backups"),
    ),
    service(
        "backupd-helper",
        "Time Machine scheduling",
        "Decides when the next Time Machine backup should start.",
        Some("scheduled Time Machine backups"),
    ),
    service(
        "cloudd",
        "iCloud sync",
        "Moves app data to and from iCloud for apps that use CloudKit.",
        Some("iCloud syncing"),
    ),
    service(
        "bird",
        "iCloud Drive",
        "Uploads and downloads iCloud Drive and Desktop & Documents files.",
        Some("iCloud Drive syncing"),
    ),
    service(
        "nsurlsessiond",
        "Background downloads and uploads",
        "Carries out transfers apps asked for in the background — App Store, Podcasts, iCloud and \
         others — so they continue after the app closes.",
        Some("background downloads and uploads"),
    ),
    service(
        "apsd",
        "Push notifications",
        "Keeps one connection to Apple open so Messages, Mail and apps can be told when something \
         arrives.",
        Some("push notifications"),
    ),
    service(
        "akd",
        "Apple ID sign-in",
        "Handles Apple ID authentication for iCloud and the App Store.",
        Some("Apple ID sign-in"),
    ),
    service(
        "identityservicesd",
        "Messages and FaceTime identity",
        "Manages your identity and device list for Messages, FaceTime and Handoff.",
        Some("Messages and FaceTime delivery"),
    ),
    service(
        "imagent",
        "Messages delivery",
        "Sends and receives Messages in the background.",
        Some("sending and receiving Messages"),
    ),
    service(
        "rapportd",
        "Continuity",
        "Lets this Mac find and talk to your other Apple devices for Handoff, Universal Clipboard \
         and unlocking with a Watch.",
        Some("Handoff and other Continuity features"),
    ),
    service(
        "sharingd",
        "AirDrop and sharing",
        "Runs AirDrop, screen sharing discovery and other sharing features.",
        Some("AirDrop and sharing"),
    ),
    service(
        "softwareupdated",
        "macOS updates",
        "Checks for and downloads macOS updates.",
        Some("checking for macOS updates"),
    ),
    service(
        "installd",
        "Installing software",
        "Installs packages and App Store apps.",
        Some("installing software"),
    ),
    service(
        "coreaudiod",
        "Sound",
        "Mixes and routes all audio on this Mac, for every app.",
        Some("all sound"),
    ),
    service(
        "bluetoothd",
        "Bluetooth",
        "Manages Bluetooth devices: keyboards, mice, headphones and Watch.",
        Some("Bluetooth"),
    ),
    service(
        "airportd",
        "Wi-Fi",
        "Scans for and joins Wi-Fi networks.",
        Some("Wi-Fi"),
    ),
    service(
        "mDNSResponder",
        "Finding names and devices on the network",
        "Looks up website names for every app, and finds printers, speakers and other devices on \
         your local network.",
        Some("looking up website names, and finding local devices"),
    ),
    service(
        "usbmuxd",
        "iPhone and iPad over USB",
        "Carries the connection to a connected iPhone or iPad.",
        Some("talking to a connected iPhone or iPad"),
    ),
    service(
        "photolibraryd",
        "Photos library",
        "Keeps the Photos library in order and does its background analysis.",
        Some("Photos library work"),
    ),
    service(
        "fileproviderd",
        "Cloud storage in Finder",
        "Coordinates cloud storage that appears in Finder — iCloud Drive, Dropbox, Google Drive and \
         similar.",
        Some("cloud folders in Finder"),
    ),
    service(
        "deleted",
        "Reclaiming disk space",
        "Asks apps to free up purgeable space when the disk gets full. The name is a coincidence: it \
         is the Cache Delete daemon, not something deleting your files.",
        Some("automatic space reclamation"),
    ),
    service(
        "analyticsd",
        "Diagnostics collection",
        "Collects the anonymous diagnostic data macOS sends to Apple if you allow it in Privacy \
         settings.",
        Some("diagnostic reporting"),
    ),
    service(
        "analyticsagent",
        "Diagnostics collection",
        "Gathers diagnostic data for your user session, sent only if you allowed it.",
        Some("diagnostic reporting"),
    ),
    service(
        "suggestd",
        "On-device suggestions",
        "Learns from mail, messages and browsing on this Mac to offer suggestions. The learning \
         stays on the device.",
        Some("Siri and Spotlight suggestions"),
    ),
    service(
        "homed",
        "Home accessories",
        "Talks to HomeKit accessories and keeps their state.",
        Some("Home accessories"),
    ),
    service(
        "passd",
        "Wallet and Apple Pay",
        "Manages passes and cards in Wallet.",
        Some("Wallet and Apple Pay"),
    ),
    service(
        "geod",
        "Maps data",
        "Fetches and caches map data for Maps and location features.",
        Some("Maps data loading"),
    ),
    service(
        "mediaremoted",
        "Play and pause keys",
        "Routes the media keys and Now Playing controls to whichever app is playing.",
        Some("media keys and Now Playing"),
    ),
    service(
        "launchservicesd",
        "Opening apps and documents",
        "Keeps the list of which app opens which file type, and helps launch apps.",
        Some("opening documents with the right app"),
    ),
    service(
        "coreservicesd",
        "Core system services",
        "A collection of basic services apps rely on to talk to the system.",
        Some("several basic system features"),
    ),
    service(
        "sharedfilelistd",
        "Recent items and favourites",
        "Keeps your recent files, favourite folders and login items.",
        Some("recent items and Finder favourites"),
    ),
    service(
        "lsd",
        "Opening apps and documents",
        "The Launch Services daemon: it resolves which app should open a document or link.",
        Some("opening documents and links"),
    ),
    service(
        "thermalmonitord",
        "Heat management",
        "Watches temperatures and asks the system to slow down before it gets too hot.",
        Some("heat management"),
    ),
    service(
        "systemstats",
        "System statistics",
        "Records CPU, memory and power statistics macOS uses for battery reports.",
        Some("battery and power reports"),
    ),
    service(
        "ReportCrash",
        "Crash reports",
        "Writes a report when an app crashes, then exits.",
        Some("crash reports"),
    ),
    service(
        "syslogd",
        "System log",
        "Collects log messages from the system and apps.",
        Some("system logging"),
    ),
    // Desktop the user sees.
    shell_ui(
        "Finder",
        "Finder — files and the desktop",
        "Shows your files, folders and the desktop icons.",
        Some("Finder windows and your desktop icons"),
    ),
    shell_ui(
        "Dock",
        "Dock, Mission Control and Launchpad",
        "Draws the Dock and runs Mission Control, Launchpad and app switching.",
        Some("the Dock, Mission Control and Launchpad"),
    ),
    shell_ui(
        "SystemUIServer",
        "Menu bar extras",
        "Draws the status items on the right of the menu bar, such as the clock and Wi-Fi.",
        Some("menu bar status items"),
    ),
    shell_ui(
        "ControlCenter",
        "Control Centre",
        "Draws Control Centre and its menu bar icons. It also advertises AirPlay to your other \
         devices, which is why it listens on ports 5000 and 7000.",
        Some("Control Centre and AirPlay Receiver"),
    ),
    shell_ui(
        "NotificationCenter",
        "Notifications",
        "Shows notification banners and Notification Centre.",
        Some("notification banners"),
    ),
    shell_ui(
        "WindowManager",
        "Stage Manager and window tidying",
        "Runs Stage Manager and the desktop-window features.",
        Some("Stage Manager"),
    ),
    shell_ui(
        "Spotlight",
        "Spotlight search window",
        "The search window you open with Command-Space.",
        Some("the Spotlight search window"),
    ),
    shell_ui(
        "TextInputMenuAgent",
        "Input menu",
        "Draws the keyboard and input-source menu.",
        Some("the input menu"),
    ),
    shell_ui(
        "UserNotificationCenter",
        "System alerts",
        "Shows the alert dialogs the system itself needs to put on screen.",
        Some("system alert dialogs"),
    ),
    shell_ui(
        "loginwindow-helper",
        "Login session helper",
        "Helps the login session start and finish.",
        None,
    ),
    // Shells and terminals.
    shell(
        "zsh",
        "zsh — command line shell",
        "A shell waiting for or running commands you typed in a terminal.",
    ),
    shell(
        "bash",
        "bash — command line shell",
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
    shell(
        "login",
        "login — starts a terminal session",
        "Sets up a terminal session and starts your shell.",
    ),
];

/// Facts a macOS path alone establishes.
pub(super) fn by_location(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let exe = facts.exe?;
    // App extensions: widgets, share sheets, settings panes. The bundle names the owning app.
    if exe.contains(".appex/") {
        let app = outermost_bundle(exe);
        let owner = app.clone().unwrap_or_else(|| "an app".to_owned());
        let draft = Draft::new(
            format!("{} — app extension", display_name(facts)),
            format!(
                "A small extension of {owner}: a widget, share option, settings panel or similar. \
                 macOS starts it when that piece is needed and stops it afterwards."
            ),
            ProcessRole::Extension,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Known);
        return Some(match app {
            Some(app) => draft.app(app),
            None => draft,
        });
    }
    // XPC services live inside the app or framework that owns them.
    if exe.contains(".xpc/") && !exe.starts_with("/System/") {
        let app = outermost_bundle(exe);
        let owner = app.clone().unwrap_or_else(|| "another program".to_owned());
        let draft = Draft::new(
            format!("{} — helper service", display_name(facts)),
            format!(
                "A helper service belonging to {owner}. It is started on demand to do one job away \
                 from the main app; Sentinel does not have a description for this particular one."
            ),
            ProcessRole::Helper,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Likely);
        return Some(match app {
            Some(app) => draft.app(app),
            None => draft,
        });
    }
    if exe.starts_with("/System/")
        || exe.starts_with("/usr/libexec/")
        || exe.starts_with("/usr/sbin/")
        || exe.starts_with("/sbin/")
        || exe.starts_with("/Library/Apple/")
    {
        return Some(
            Draft::new(
                format!("{} — macOS background service", display_name(facts)),
                format!(
                    "Part of macOS itself: it runs from {}, which only Apple's system software \
                     uses. Sentinel does not recognise this particular service, so it will not \
                     describe what it does.",
                    system_area(exe)
                ),
                ProcessRole::Daemon,
                ProcessCategory::OsService,
                QuitSafety::OsService,
            )
            .confidence(Confidence::Likely),
        );
    }
    // A regular application bundle: /Applications/Thing.app/Contents/MacOS/Thing.
    if let Some(app) = outermost_bundle(exe) {
        let sandboxed = exe.starts_with("/System/Applications/");
        return Some(
            Draft::new(
                format!("{app} — the app itself"),
                if sandboxed {
                    format!("{app}, one of the apps that come with macOS. This is the app itself, not a helper.")
                } else {
                    format!(
                        "{app}, installed as an application on this Mac. This is the app itself, \
                         not a helper."
                    )
                },
                ProcessRole::MainApp,
                ProcessCategory::UserApp,
                QuitSafety::UserApp,
            )
            .app(app)
            .confidence(Confidence::Known),
        );
    }
    if exe.starts_with("/opt/homebrew/") || exe.starts_with("/usr/local/") {
        return Some(
            Draft::new(
                format!("{} — installed command line program", display_name(facts)),
                format!(
                    "Runs from {}, where Homebrew and other command-line tools install software. \
                     Sentinel does not recognise the program itself.",
                    if exe.starts_with("/opt/homebrew/") {
                        "/opt/homebrew"
                    } else {
                        "/usr/local"
                    }
                ),
                ProcessRole::Unknown,
                ProcessCategory::Developer,
                QuitSafety::Background,
            )
            .confidence(Confidence::Likely),
        );
    }
    if exe.starts_with("/Library/") {
        return Some(
            Draft::new(
                format!(
                    "{} — background service from another program",
                    display_name(facts)
                ),
                "Installed by software outside macOS itself (it runs from /Library), and started \
                 without a window. Sentinel does not recognise it, so it will not describe what it \
                 does."
                    .to_owned(),
                ProcessRole::Daemon,
                ProcessCategory::Unknown,
                QuitSafety::Background,
            )
            .confidence(Confidence::Unknown),
        );
    }
    None
}

fn system_area(exe: &str) -> &'static str {
    if exe.starts_with("/System/Library/PrivateFrameworks/") {
        "/System/Library/PrivateFrameworks"
    } else if exe.starts_with("/System/Library/CoreServices/") {
        "/System/Library/CoreServices"
    } else if exe.starts_with("/System/Library/Frameworks/") {
        "/System/Library/Frameworks"
    } else if exe.starts_with("/System/") {
        "/System"
    } else if exe.starts_with("/usr/libexec/") {
        "/usr/libexec"
    } else if exe.starts_with("/Library/Apple/") {
        "/Library/Apple"
    } else {
        "a system-only folder"
    }
}
