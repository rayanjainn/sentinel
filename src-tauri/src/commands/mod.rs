pub mod actions;
pub mod agent;
pub mod firewall;
pub mod network;
pub mod process;
pub mod storage;
pub mod system;

use sentinel_core::{ErrorPayload, SentinelError};

pub type CmdResult<T> = Result<T, ErrorPayload>;

/// Contract-phase scaffolding so the frontend can compile and render error states before a backend
/// lands. Every call site is replaced by a real implementation; CI fails if any remain
/// (`grep -r not_wired src-tauri/src/commands`).
pub(crate) fn not_wired<T>(command: &str) -> CmdResult<T> {
    Err(SentinelError::Unavailable {
        feature: command.to_owned(),
        reason: "backend not wired yet".to_owned(),
    }
    .into())
}
