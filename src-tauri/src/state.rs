//! Shared state handed to Tauri commands. Workstream A constructs it during setup; the agent
//! workstream consumes it, so the agent reaches exactly the same runtime as the UI.

use std::sync::Arc;

use sentinel_core::action::{ActionCommitter, ActionPreparer};
use sentinel_core::audit::AuditStore;
use sentinel_core::events::{CoreEvent, EventSink};
use sentinel_core::runtime::{CoreRuntime, RuntimeConfig};
use sentinel_core::service::SystemQueries;
use sentinel_core::service::actions::{ActionContext, ActionService, ActionServiceConfig};
use sentinel_core::service::audit_sqlite::SqliteAuditStore;
use sentinel_core::{CoreResult, SentinelError};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::CmdResult;

/// Registered with `app.manage(CoreState { .. })` in `lib.rs` setup.
#[derive(Clone)]
pub struct CoreState {
    pub queries: Arc<dyn SystemQueries>,
    pub preparer: Arc<dyn ActionPreparer>,
    pub committer: Arc<dyn ActionCommitter>,
    pub audit: Arc<dyn AuditStore>,
    /// Runtime controls that are not part of the read facade (sampling subscriptions, settings).
    pub runtime: Arc<CoreRuntime>,
}

/// Forwards core events to the webview under the names in `sentinel_core::events::names`.
struct TauriEventSink {
    app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: CoreEvent) {
        let name = event.name();
        // Emission only fails when the webview is gone (shutdown); there is nobody to tell.
        let _ = match event {
            CoreEvent::Resources(payload) => self.app.emit(name, payload),
            CoreEvent::Processes(payload) => self.app.emit(name, payload),
            CoreEvent::Network(payload) => self.app.emit(name, payload),
            CoreEvent::HostResolved(payload) => self.app.emit(name, payload),
            CoreEvent::ScanProgress(payload) => self.app.emit(name, payload),
            CoreEvent::ScanPartial(payload) => self.app.emit(name, payload),
            CoreEvent::ScanComplete(payload) => self.app.emit(name, payload),
            CoreEvent::DuplicateProgress(payload) => self.app.emit(name, payload),
            CoreEvent::DuplicateComplete(payload) => self.app.emit(name, payload),
            CoreEvent::GeoDb(payload) => self.app.emit(name, payload),
        };
    }
}

pub fn build(app: &AppHandle) -> Result<CoreState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let runtime = CoreRuntime::start(
        RuntimeConfig {
            data_dir: data_dir.clone(),
        },
        Arc::new(TauriEventSink { app: app.clone() }),
    )?;
    let audit: Arc<dyn AuditStore> =
        Arc::new(SqliteAuditStore::open(&data_dir.join("audit.sqlite3"))?);
    let permissions = runtime.permission_status();
    let actions = Arc::new(ActionService::new(
        Arc::clone(&runtime) as Arc<dyn ActionContext>,
        runtime.process_control(),
        Arc::clone(&audit),
        ActionServiceConfig {
            platform: permissions.platform,
            running_elevated: permissions.running_elevated,
        },
    ));
    Ok(CoreState {
        queries: Arc::clone(&runtime) as Arc<dyn SystemQueries>,
        preparer: Arc::clone(&actions) as Arc<dyn ActionPreparer>,
        committer: actions,
        audit,
        runtime,
    })
}

/// Runs blocking core work off the async executor and maps errors to the wire payload.
pub async fn blocking<T, F>(work: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CoreResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| SentinelError::internal(format!("background task failed: {err}")))?
        .map_err(Into::into)
}
