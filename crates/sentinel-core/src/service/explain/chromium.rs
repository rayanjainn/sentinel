//! Chromium, Electron and WebKit helper processes.
//!
//! A browser splits itself into dozens of processes. The command line says what each one is for
//! (`--type=renderer`, `--utility-sub-type=network.mojom.NetworkService`, …) and the executable
//! path says which app owns it, so these explanations are read from facts rather than guessed.
//!
//! The one thing the OS never reports is *which tab or site* a renderer is drawing. That is stated
//! plainly instead of invented.

use super::Draft;
use super::process::{ProcessFacts, capitalize, file_name, flag_value, has_flag, stem};
use crate::model::{Confidence, Platform, ProcessCategory, ProcessRole, QuitSafety};

const CRASHPAD: &str = "crashpad_handler";

pub(super) fn classify(facts: &ProcessFacts<'_>) -> Option<Draft> {
    if let Some(draft) = webkit_service(facts) {
        return Some(draft);
    }
    let app = owning_app(facts);
    if file_name(facts.exe).is_some_and(|name| stem(name).ends_with(CRASHPAD)) {
        let app = app.unwrap_or_else(|| "its application".to_owned());
        return Some(
            Draft::new(
                format!("{app} — crash reporter"),
                format!(
                    "Waits in the background to collect a report if {app} crashes. It uses almost \
                     no resources while nothing is wrong."
                ),
                ProcessRole::Utility,
                ProcessCategory::AppHelper,
                QuitSafety::AppHelper,
            )
            .app(app),
        );
    }

    let kind = flag_value(facts.cmd, "--type")?;
    let app = app?;
    let helper = match kind {
        "renderer" if has_flag(facts.cmd, "--extension-process") => Draft::new(
            format!("{app} — browser extension"),
            format!(
                "Runs one of {app}'s extensions. The operating system does not say which \
                 extension, so Sentinel does not name one."
            ),
            ProcessRole::Extension,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "renderer" => Draft::new(
            format!("{app} — web page"),
            format!(
                "Draws one web page or site for {app}. One browser tab or site is using this \
                 process; the operating system does not say which one, and Sentinel never guesses. \
                 Its memory and CPU belong to that page."
            ),
            ProcessRole::Renderer,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "gpu-process" => Draft::new(
            format!("{app} — graphics"),
            format!(
                "Draws {app}'s windows and video using the graphics chip, for every tab at once."
            ),
            ProcessRole::Gpu,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "zygote" => Draft::new(
            format!("{app} — process launcher"),
            format!(
                "A template process {app} clones whenever it needs a new tab or helper. It does no \
                 work of its own."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "broker" => Draft::new(
            format!("{app} — sandbox broker"),
            format!(
                "Starts {app}'s sandboxed helpers and passes them the access they are allowed."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "utility" => utility(facts, &app),
        other => Draft::new(
            format!("{app} — helper ({other})"),
            format!(
                "A helper process inside {app}. Sentinel does not recognise the helper type \
                 \"{other}\", which is shown below exactly as {app} reported it."
            ),
            ProcessRole::Helper,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Likely),
    };
    Some(helper.app(app))
}

/// `--type=utility --utility-sub-type=<mojo service>`: the sub-type names the job exactly.
fn utility(facts: &ProcessFacts<'_>, app: &str) -> Draft {
    let Some(sub) = flag_value(facts.cmd, "--utility-sub-type") else {
        return Draft::new(
            format!("{app} — background service"),
            format!(
                "A background service inside {app}. It did not say which service, so Sentinel does \
                 not name one."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Likely);
    };
    match sub {
        "network.mojom.NetworkService" => Draft::new(
            format!("{app} — network"),
            format!(
                "Makes every network request for {app}: loading pages, downloads and updates all \
                 go through this one process, which is why it can show traffic when no tab seems \
                 busy."
            ),
            ProcessRole::NetworkService,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "audio.mojom.AudioService" => Draft::new(
            format!("{app} — sound"),
            format!("Plays and records sound for {app}, including video and calls in any tab."),
            ProcessRole::AudioService,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "storage.mojom.StorageService" => Draft::new(
            format!("{app} — site storage"),
            format!(
                "Reads and writes the data sites keep on this machine for {app}: cookies, local \
                 storage and databases."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "video_capture.mojom.VideoCaptureService" => Draft::new(
            format!("{app} — camera"),
            format!(
                "Handles camera access for {app}. It runs whenever a site or the browser has the \
                 camera open."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "media.mojom.CdmServiceBroker" | "media.mojom.CdmService" => Draft::new(
            format!("{app} — protected video"),
            format!(
                "Plays video that is protected against copying, such as a streaming service, for \
                 {app}."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "data_decoder.mojom.DataDecoderService" => Draft::new(
            format!("{app} — decoding data"),
            format!(
                "Decodes images and other untrusted data away from the rest of {app}, so a broken \
                 file cannot affect your tabs."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "proxy_resolver.mojom.ProxyResolverFactory" => Draft::new(
            format!("{app} — proxy settings"),
            format!("Works out which proxy server {app} should use for a given address."),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "node.mojom.NodeService" => Draft::new(
            format!("{app} — Node.js background work"),
            format!(
                "Runs JavaScript outside the page for {app} — in an Electron app this is usually \
                 part of the app itself rather than a website."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        "tracing.mojom.TracedProcess" => Draft::new(
            format!("{app} — performance tracing"),
            format!("Collects timing information inside {app}, normally only while debugging."),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        ),
        other => Draft::new(
            format!("{app} — background service"),
            format!(
                "Looks like a background service inside {app}. Sentinel does not recognise the \
                 service name \"{other}\", which is shown below exactly as {app} reported it."
            ),
            ProcessRole::Utility,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Likely),
    }
}

/// Safari's engine (WebKit) splits the same way, as XPC services shared by every app that embeds a
/// web view. The system starts them from launchd, so the owning app is usually not knowable.
fn webkit_service(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let exe = facts.exe?;
    if facts.platform != Platform::Macos || !exe.contains("com.apple.WebKit.") {
        return None;
    }
    let owner = facts
        .parent
        .filter(|parent| parent.pid > 1)
        .map(|parent| parent.name.to_owned());
    let for_app = owner
        .as_deref()
        .map_or_else(|| "an app".to_owned(), |name| name.to_owned());
    let draft = if exe.contains("com.apple.WebKit.WebContent") {
        Draft::new(
            "Web page — Safari's web engine",
            format!(
                "Draws one web page for {for_app} using the web engine built into macOS — Safari, \
                 Mail, Help and any app with a web view all use it. The system does not say which \
                 page, and Sentinel never guesses one."
            ),
            ProcessRole::Renderer,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
    } else if exe.contains("com.apple.WebKit.Networking") {
        Draft::new(
            "Web requests — Safari's web engine",
            format!("Makes the network requests for web pages shown by {for_app}."),
            ProcessRole::NetworkService,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
    } else if exe.contains("com.apple.WebKit.GPU") {
        Draft::new(
            "Web graphics — Safari's web engine",
            format!("Draws and composites web content for {for_app} using the graphics chip."),
            ProcessRole::Gpu,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
    } else {
        Draft::new(
            "Safari's web engine — helper",
            "Part of the web engine built into macOS. Sentinel does not recognise this particular \
             helper; its path is shown below.",
            ProcessRole::Helper,
            ProcessCategory::AppHelper,
            QuitSafety::AppHelper,
        )
        .confidence(Confidence::Likely)
    };
    Some(match owner {
        Some(app) => draft.app(app),
        None => draft,
    })
}

/// The application a helper belongs to.
///
/// On macOS every helper lives inside its app bundle, so the outermost `.app` in the path names the
/// owner exactly (`/Applications/Brave Browser.app/…/Brave Browser Helper (Renderer)` → "Brave
/// Browser"). Elsewhere Chromium helpers are the same binary as the browser, so the parent's name
/// or the executable's own name is used.
fn owning_app(facts: &ProcessFacts<'_>) -> Option<String> {
    if let Some(exe) = facts.exe
        && let Some(app) = outermost_bundle(exe)
    {
        return Some(app);
    }
    if let Some(parent) = facts.parent
        && parent.exe.is_some()
        && parent.exe == facts.exe
    {
        return Some(capitalize(stem(parent.name)));
    }
    let own = file_name(facts.exe).unwrap_or(facts.name);
    Some(capitalize(stem(own)))
}

/// Name of the first `.app` bundle in a macOS path.
pub(super) fn outermost_bundle(exe: &str) -> Option<String> {
    exe.split('/')
        .find_map(|part| part.strip_suffix(".app"))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}
