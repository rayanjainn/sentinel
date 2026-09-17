//! Plain-language explanations for processes and connections.
//!
//! Everything here is derived from facts the OS already reported — executable path, command-line
//! flags, parent process, owning user, socket address and port — plus curated catalogs of
//! well-known programs and endpoints. There are no network lookups and no heuristics that invent
//! detail: an unrecognised program produces an "unrecognised" explanation carrying the raw
//! evidence, never a plausible-sounding description.

mod catalog;
mod chromium;
mod connection;
mod hosts;
mod interpreter;
mod ports;
mod process;

#[cfg(test)]
mod tests;

pub use connection::{ConnectionFacts, explain_connection};
pub use process::{
    ParentFacts, ProcessFacts, explain_info, explain_process, facts_from, host_platform,
    summary_info,
};

use crate::model::{Confidence, ProcessCategory, ProcessRole, QuitSafety};

/// A classified process, before evidence and the quit note are attached.
pub(super) struct Draft {
    pub headline: String,
    pub detail: String,
    pub role: ProcessRole,
    pub category: ProcessCategory,
    pub quit_safety: QuitSafety,
    pub app_name: Option<String>,
    pub confidence: Confidence,
    /// What stops working while it is gone ("Spotlight search"), for the quit note.
    pub stops: Option<String>,
}

impl Draft {
    pub(super) fn new(
        headline: impl Into<String>,
        detail: impl Into<String>,
        role: ProcessRole,
        category: ProcessCategory,
        quit_safety: QuitSafety,
    ) -> Self {
        Self {
            headline: headline.into(),
            detail: detail.into(),
            role,
            category,
            quit_safety,
            app_name: None,
            confidence: Confidence::Known,
            stops: None,
        }
    }

    pub(super) fn app(mut self, app: impl Into<String>) -> Self {
        self.app_name = Some(app.into());
        self
    }

    pub(super) fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub(super) fn stops(mut self, stops: impl Into<String>) -> Self {
        self.stops = Some(stops.into());
        self
    }
}
