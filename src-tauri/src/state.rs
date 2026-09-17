//! Shared state handed to Tauri commands. Workstream A constructs it during setup; the agent
//! workstream consumes it, so the agent reaches exactly the same runtime as the UI.

use std::sync::Arc;

use sentinel_core::action::{ActionCommitter, ActionPreparer};
use sentinel_core::audit::AuditStore;
use sentinel_core::service::SystemQueries;

/// Registered with `app.manage(CoreState { .. })` in `lib.rs` setup.
#[allow(dead_code)] // Constructed once the core runtime is wired in setup.
#[derive(Clone)]
pub struct CoreState {
    pub queries: Arc<dyn SystemQueries>,
    pub preparer: Arc<dyn ActionPreparer>,
    pub committer: Arc<dyn ActionCommitter>,
    pub audit: Arc<dyn AuditStore>,
}
