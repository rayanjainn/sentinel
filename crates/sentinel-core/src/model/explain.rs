//! Plain-language explanation types.
//!
//! Every field here is derived from facts the OS already gave us — executable path, command-line
//! flags, parent process, owning user — plus a curated catalog of well-known programs. Nothing is
//! guessed silently: [`Confidence`] states how sure the explanation is, and `evidence` carries the
//! raw facts it was built from so a reader can judge it.
//!
//! Two limits are stated in the copy rather than guessed around:
//! 1. The OS attributes a connection to a process, never to a browser tab.
//! 2. Encrypted payloads cannot be read, and Sentinel captures no packets.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a process does inside its application or the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessRole {
    /// The application itself — the window you interact with.
    MainApp,
    /// Draws one web page or site (a browser tab, an embedded web view).
    Renderer,
    /// Runs a browser extension.
    Extension,
    /// Draws graphics for an app on its behalf.
    Gpu,
    /// Makes the network requests for an app.
    NetworkService,
    /// Plays or records sound for an app.
    AudioService,
    /// A named background service inside an app (storage, video capture, …).
    Utility,
    /// A helper belonging to an app whose specific job is not known.
    Helper,
    /// An operating-system background service.
    Daemon,
    /// The kernel itself or a kernel thread.
    Kernel,
    /// A command-line shell.
    Shell,
    /// An interpreter (node, python, java, …) running a script or program.
    /// The script or entry point is named in the headline and the evidence.
    Interpreter,
    Unknown,
}

/// Which family a process belongs to, for grouping and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessCategory {
    Browser,
    UserApp,
    /// Part of another application, which `app_name` names.
    AppHelper,
    OsService,
    Kernel,
    Developer,
    Security,
    Unknown,
}

/// What happens if this process is stopped. Drives the "Is it safe to quit?" copy in the table,
/// the detail drawer and the confirm dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum QuitSafety {
    /// The system needs it; quitting can freeze or restart the machine.
    SystemCritical,
    /// The OS restarts it, but the feature it provides stops until then.
    OsService,
    /// Part of an app; quitting affects that app only.
    AppHelper,
    /// A program the user started; unsaved work would be lost.
    UserApp,
    /// Runs in the background with no window; consequences are limited but specific.
    Background,
    /// Not recognised — the evidence is shown instead of a guess.
    Unknown,
}

/// How much Sentinel actually knows about an explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Confidence {
    /// Read directly from the system, or an exact match in the catalog. Reads as fact.
    Known,
    /// Inferred from a pattern (a path, a flag, a hostname). Reads as "looks like …".
    Likely,
    /// Not recognised. Says so, and shows the raw evidence.
    Unknown,
}

/// Compact explanation carried by every row of the 1 Hz process snapshot: enough for the table's
/// plain subtitle, app grouping and safety hint. The full [`ProcessExplanation`] (with detail,
/// quit note and evidence) comes with `ProcessDetail`, on demand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessSummary {
    /// One short line in ordinary English: "Brave Browser — web page".
    pub headline: String,
    /// Owning application for helpers, so 40 helpers group under one app.
    pub app_name: Option<String>,
    pub role: ProcessRole,
    pub category: ProcessCategory,
    pub quit_safety: QuitSafety,
    pub confidence: Confidence,
}

/// Full explanation of one process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessExplanation {
    /// "Brave Browser — web page".
    pub headline: String,
    /// One or two plain sentences. Inferences start with "Looks like"; unknowns say so.
    pub detail: String,
    pub role: ProcessRole,
    pub category: ProcessCategory,
    pub quit_safety: QuitSafety,
    /// What actually happens if you quit it, and the better alternative when there is one.
    pub quit_note: String,
    /// The raw facts this was built from: flags, bundle, parent, user, path.
    pub evidence: Vec<String>,
    pub app_name: Option<String>,
    pub confidence: Confidence,
}

impl ProcessExplanation {
    pub fn summary(&self) -> ProcessSummary {
        ProcessSummary {
            headline: self.headline.clone(),
            app_name: self.app_name.clone(),
            role: self.role,
            category: self.category,
            quit_safety: self.quit_safety,
            confidence: self.confidence,
        }
    }
}

/// Explanation of one socket: who the other end is, what that endpoint is usually for, and an
/// explicit statement about encryption. Never names a browser tab or a URL, because the OS does
/// not attribute a connection to either.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionExplanation {
    /// "Secure web traffic to Apple", "Domain name lookup".
    pub headline: String,
    /// Plain sentences: who owns the other end, what this is usually for, and — for encrypted
    /// ports — that the contents cannot be read.
    pub detail: String,
    /// Plain name of the service on the port: "secure web", "domain name lookup".
    pub service: Option<String>,
    /// What that destination is usually used for, when the catalog recognises it.
    pub purpose: Option<String>,
    pub confidence: Confidence,
    /// True when the port is one that is encrypted by definition (HTTPS, QUIC, SSH, IMAPS, …).
    pub encrypted: bool,
}
