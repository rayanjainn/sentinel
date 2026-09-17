//! Curated per-OS catalogs of well-known programs, plus location-based fallbacks.
//!
//! An entry is only added when its job can be stated as a fact. Anything not in a catalog falls
//! through to a location-based explanation ("part of macOS, Sentinel does not recognise this one")
//! and finally to "not recognised" — never to a plausible guess.

mod linux;
mod macos;
mod windows;

use super::Draft;
use super::process::{ProcessFacts, file_name, stem};
use crate::model::{Platform, ProcessCategory, ProcessRole, QuitSafety};

pub(super) struct Entry {
    /// Executable file name, without `.exe`.
    pub name: &'static str,
    pub headline: &'static str,
    pub detail: &'static str,
    pub role: ProcessRole,
    pub category: ProcessCategory,
    pub safety: QuitSafety,
    /// Feature that stops while it is not running, named in the quit note.
    pub stops: Option<&'static str>,
}

/// An OS service the system restarts by itself.
const fn service(
    name: &'static str,
    headline: &'static str,
    detail: &'static str,
    stops: Option<&'static str>,
) -> Entry {
    Entry {
        name,
        headline,
        detail,
        role: ProcessRole::Daemon,
        category: ProcessCategory::OsService,
        safety: QuitSafety::OsService,
        stops,
    }
}

/// A service the session or the machine cannot do without.
const fn critical(name: &'static str, headline: &'static str, detail: &'static str) -> Entry {
    Entry {
        name,
        headline,
        detail,
        role: ProcessRole::Daemon,
        category: ProcessCategory::OsService,
        safety: QuitSafety::SystemCritical,
        stops: None,
    }
}

/// A part of the desktop the user sees.
const fn shell_ui(
    name: &'static str,
    headline: &'static str,
    detail: &'static str,
    stops: Option<&'static str>,
) -> Entry {
    Entry {
        name,
        headline,
        detail,
        role: ProcessRole::MainApp,
        category: ProcessCategory::OsService,
        safety: QuitSafety::OsService,
        stops,
    }
}

/// A security-related service.
const fn security(
    name: &'static str,
    headline: &'static str,
    detail: &'static str,
    safety: QuitSafety,
) -> Entry {
    Entry {
        name,
        headline,
        detail,
        role: ProcessRole::Daemon,
        category: ProcessCategory::Security,
        safety,
        stops: None,
    }
}

/// A command-line shell.
const fn shell(name: &'static str, headline: &'static str, detail: &'static str) -> Entry {
    Entry {
        name,
        headline,
        detail,
        role: ProcessRole::Shell,
        category: ProcessCategory::Developer,
        safety: QuitSafety::Background,
        stops: None,
    }
}

impl Entry {
    fn to_draft(&self) -> Draft {
        let mut draft = Draft::new(
            self.headline,
            self.detail,
            self.role,
            self.category,
            self.safety,
        );
        if let Some(stops) = self.stops {
            draft = draft.stops(stops);
        }
        draft
    }
}

fn entries(platform: Platform) -> &'static [Entry] {
    match platform {
        Platform::Macos => macos::ENTRIES,
        Platform::Linux => linux::ENTRIES,
        Platform::Windows => windows::ENTRIES,
    }
}

pub(super) fn lookup(facts: &ProcessFacts<'_>) -> Option<Draft> {
    if facts.platform == Platform::Windows
        && let Some(draft) = windows::svchost(facts)
    {
        return Some(draft);
    }
    let table = entries(facts.platform);
    let from_exe = file_name(facts.exe).map(stem);
    let reported = stem(facts.name);
    // The executable name is authoritative; the reported name is a fallback because some platforms
    // truncate it.
    let entry = from_exe
        .and_then(|name| table.iter().find(|entry| entry.name == name))
        .or_else(|| table.iter().find(|entry| entry.name == reported))?;
    Some(entry.to_draft())
}

/// What the location of an unrecognised executable still tells us for certain.
pub(super) fn by_location(facts: &ProcessFacts<'_>) -> Option<Draft> {
    match facts.platform {
        Platform::Macos => macos::by_location(facts),
        Platform::Linux => linux::by_location(facts),
        Platform::Windows => windows::by_location(facts),
    }
}
