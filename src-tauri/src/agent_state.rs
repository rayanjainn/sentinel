//! Agent runtime state behind the `agent_*` commands: persisted settings (no secrets), key
//! validation status, conversations, and per-turn engine/executor construction from `CoreState`.
//!
//! The engine is built from `CoreState.queries` and `CoreState.preparer` only. The committer is
//! handed to `PlanExecutor`, which runs solely from `agent_execute_plan`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tauri::{AppHandle, Emitter, Manager};

use sentinel_agent::backend::AgentBackend;
use sentinel_agent::conversation::{ConversationStore, now_ms};
use sentinel_agent::engine::{AgentEngine, CancellationToken, EngineConfig, EngineParts};
use sentinel_agent::events::{AgentEvent, AgentEventSink, AgentStreamPayload, TranscriptItem};
use sentinel_agent::executor::PlanExecutor;
use sentinel_agent::plan::{ActionDecision, Plan, PlanActionState};
use sentinel_agent::providers::ollama::{self, OllamaBackend};
use sentinel_agent::providers::{catalog, http};
use sentinel_agent::settings::{
    AgentSettings, KeyStatus, ModelInfo, OllamaStatus, ProviderDescriptor, ProviderId,
    ProviderStatus,
};
use sentinel_agent::tools::read::SystemReadTools;
use sentinel_agent::tools::write::SystemWriteTranslator;
use sentinel_core::action::Action;
use sentinel_core::events::names;
use sentinel_core::{CoreResult, ErrorPayload, SentinelError};

use crate::secrets::{Keychain, SecretStore};
use crate::state::CoreState;

const SETTINGS_FILE: &str = "agent-settings.json";
const KEY_STATUS_FILE: &str = "agent-key-status.json";
const VALIDATION_TIMEOUT: Duration = Duration::from_secs(20);
const DAEMON_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Forwards agent events to the webview as `sentinel:agent`.
pub struct TauriAgentSink {
    app: AppHandle,
}

impl AgentEventSink for TauriAgentSink {
    fn emit(&self, payload: AgentStreamPayload) {
        // Fails only while the webview is shutting down.
        let _ = self.app.emit(names::AGENT, payload);
    }
}

pub struct AgentState {
    config_dir: PathBuf,
    settings: RwLock<AgentSettings>,
    key_status: Mutex<BTreeMap<ProviderId, KeyStatus>>,
    secrets: Arc<dyn SecretStore>,
    /// Avoids re-prompting for keychain access within a session.
    key_cache: Mutex<HashMap<ProviderId, String>>,
    store: Arc<ConversationStore>,
    sink: Arc<dyn AgentEventSink>,
    turns: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> CoreResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(SentinelError::internal)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| SentinelError::io(&e, Some(&tmp)))?;
    std::fs::rename(&tmp, path).map_err(|e| SentinelError::io(&e, Some(path)))
}

async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> CoreResult<T> + Send + 'static,
) -> CoreResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(SentinelError::internal)?
}

fn has_stored_key(status: Option<&KeyStatus>) -> bool {
    matches!(
        status,
        Some(KeyStatus::Valid { .. } | KeyStatus::Unverified { .. })
    )
}

/// Frees a conversation's turn slot when the turn ends, however it ends.
struct TurnSlot {
    turns: Arc<Mutex<HashMap<String, CancellationToken>>>,
    conversation_id: String,
}

impl Drop for TurnSlot {
    fn drop(&mut self) {
        lock(&self.turns).remove(&self.conversation_id);
    }
}

impl AgentState {
    pub fn load(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = app.path().app_config_dir()?;
        Ok(Self::new(
            config_dir,
            Arc::new(Keychain::app()),
            Arc::new(TauriAgentSink { app: app.clone() }),
        )?)
    }

    pub fn new(
        config_dir: PathBuf,
        secrets: Arc<dyn SecretStore>,
        sink: Arc<dyn AgentEventSink>,
    ) -> CoreResult<Self> {
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| SentinelError::io(&e, Some(&config_dir)))?;
        let settings = read_json::<AgentSettings>(&config_dir.join(SETTINGS_FILE))
            .and_then(|s| catalog::sanitize_settings(s).ok())
            .unwrap_or_default();
        let key_status = read_json(&config_dir.join(KEY_STATUS_FILE)).unwrap_or_default();
        Ok(Self {
            config_dir,
            settings: RwLock::new(settings),
            key_status: Mutex::new(key_status),
            secrets,
            key_cache: Mutex::new(HashMap::new()),
            store: ConversationStore::new(),
            sink,
            turns: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // -- settings ------------------------------------------------------------------------------

    pub fn settings(&self) -> AgentSettings {
        self.settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn update_settings(&self, settings: AgentSettings) -> CoreResult<AgentSettings> {
        let clean = catalog::sanitize_settings(settings)?;
        write_json(&self.config_dir.join(SETTINGS_FILE), &clean)?;
        *self
            .settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = clean.clone();
        Ok(clean)
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        catalog::descriptors(&self.settings())
    }

    // -- keys ----------------------------------------------------------------------------------

    fn record_status(&self, provider: ProviderId, status: Option<KeyStatus>) -> CoreResult<()> {
        let snapshot = {
            let mut map = lock(&self.key_status);
            match status {
                Some(status) => map.insert(provider, status),
                None => map.remove(&provider),
            };
            map.clone()
        };
        write_json(&self.config_dir.join(KEY_STATUS_FILE), &snapshot)
    }

    pub fn key_status(&self, provider: ProviderId) -> KeyStatus {
        if !catalog::requires_api_key(provider) {
            return KeyStatus::NotRequired;
        }
        lock(&self.key_status)
            .get(&provider)
            .cloned()
            .unwrap_or(KeyStatus::Missing)
    }

    async fn api_key(&self, provider: ProviderId) -> CoreResult<Option<String>> {
        if !catalog::requires_api_key(provider) {
            return Ok(None);
        }
        if let Some(key) = lock(&self.key_cache).get(&provider) {
            return Ok(Some(key.clone()));
        }
        let secrets = Arc::clone(&self.secrets);
        let key = run_blocking(move || secrets.get(provider)).await?;
        match &key {
            Some(k) => {
                lock(&self.key_cache).insert(provider, k.clone());
            }
            None if has_stored_key(lock(&self.key_status).get(&provider)) => {
                // Removed outside Sentinel.
                self.record_status(provider, None)?;
            }
            None => {}
        }
        Ok(key)
    }

    /// Validates the key against the provider before storing it. Rejected keys are never stored;
    /// keys that cannot be checked (offline) are stored as unverified.
    pub async fn set_api_key(&self, provider: ProviderId, key: String) -> CoreResult<KeyStatus> {
        if !catalog::requires_api_key(provider) {
            return Err(SentinelError::invalid(format!(
                "{} does not use an API key",
                provider.display_name()
            )));
        }
        let key = key.trim().to_owned();
        if key.is_empty() || key.len() > 4096 || key.chars().any(char::is_whitespace) {
            return Err(SentinelError::invalid(
                "paste the API key exactly as issued",
            ));
        }
        let backend = catalog::build_backend(provider, Some(key.clone()), &self.settings())?;
        let checked = tokio::time::timeout(VALIDATION_TIMEOUT, backend.validate_credentials())
            .await
            .unwrap_or_else(|_| {
                Err(SentinelError::Network {
                    detail: format!("{} did not answer in time", provider.display_name()),
                })
            });
        let status = match checked {
            Err(err) if http::is_auth_rejection(&err) => {
                let status = KeyStatus::Invalid {
                    message: err.to_string(),
                    checked_at_ms: now_ms(),
                };
                if !has_stored_key(lock(&self.key_status).get(&provider)) {
                    self.record_status(provider, Some(status.clone()))?;
                }
                return Ok(status);
            }
            Ok(()) => KeyStatus::Valid {
                checked_at_ms: now_ms(),
            },
            Err(err) => KeyStatus::Unverified {
                message: format!("Saved, but it could not be checked right now: {err}"),
            },
        };
        let secrets = Arc::clone(&self.secrets);
        let secret = key.clone();
        run_blocking(move || secrets.set(provider, &secret)).await?;
        lock(&self.key_cache).insert(provider, key);
        self.record_status(provider, Some(status.clone()))?;
        Ok(status)
    }

    pub async fn delete_api_key(&self, provider: ProviderId) -> CoreResult<()> {
        if !catalog::requires_api_key(provider) {
            return Ok(());
        }
        let secrets = Arc::clone(&self.secrets);
        run_blocking(move || secrets.delete(provider)).await?;
        lock(&self.key_cache).remove(&provider);
        self.record_status(provider, None)
    }

    fn local_ollama(&self) -> OllamaBackend {
        OllamaBackend::local(self.settings().ollama_base_url)
    }

    pub async fn provider_status(&self) -> Vec<ProviderStatus> {
        let mut out = Vec::with_capacity(ProviderId::ALL.len());
        for provider in ProviderId::ALL {
            let key = self.key_status(provider);
            let ready = match provider {
                ProviderId::Ollama => {
                    tokio::time::timeout(DAEMON_PROBE_TIMEOUT, self.local_ollama().version())
                        .await
                        .is_ok_and(|r| r.is_ok())
                }
                _ => matches!(key, KeyStatus::Valid { .. } | KeyStatus::Unverified { .. }),
            };
            out.push(ProviderStatus {
                id: provider,
                key,
                ready,
            });
        }
        out
    }

    /// Live model list. Cloud providers fall back to suggestions when no key is saved or the
    /// provider is unreachable; a rejected key and a stopped Ollama daemon are reported as errors.
    pub async fn list_models(&self, provider: ProviderId) -> CoreResult<Vec<ModelInfo>> {
        if provider == ProviderId::Ollama {
            return self.local_ollama().list_models().await;
        }
        let Some(key) = self.api_key(provider).await? else {
            return Ok(catalog::suggested_models(provider));
        };
        let backend = catalog::build_backend(provider, Some(key), &self.settings())?;
        match tokio::time::timeout(VALIDATION_TIMEOUT, backend.list_models()).await {
            Ok(Ok(models)) if !models.is_empty() => Ok(models),
            Ok(Err(err)) if http::is_auth_rejection(&err) => Err(err),
            _ => Ok(catalog::suggested_models(provider)),
        }
    }

    pub async fn ollama_status(&self) -> OllamaStatus {
        let backend = self.local_ollama();
        let version = match tokio::time::timeout(DAEMON_PROBE_TIMEOUT, backend.version()).await {
            Ok(Ok(version)) => Some(version),
            _ => None,
        };
        let running = version.is_some();
        let models = if running {
            backend.list_models().await.unwrap_or_default()
        } else {
            Vec::new()
        };
        OllamaStatus {
            installed: running || ollama::find_installation().is_some(),
            running,
            version,
            base_url: backend.base_url().to_owned(),
            models,
        }
    }

    // -- conversations -------------------------------------------------------------------------

    pub fn new_conversation(&self) -> String {
        self.store.create()
    }

    pub fn transcript(&self, conversation_id: &str) -> CoreResult<Vec<TranscriptItem>> {
        self.store.transcript(conversation_id)
    }

    pub fn cancel(&self, conversation_id: &str) {
        if let Some(token) = lock(&self.turns).get(conversation_id) {
            token.cancel();
        }
    }

    fn reserve_turn(&self, conversation_id: &str) -> CoreResult<(TurnSlot, CancellationToken)> {
        let mut turns = lock(&self.turns);
        if turns.contains_key(conversation_id) {
            return Err(SentinelError::invalid(
                "Sentinel is still answering in this conversation. Wait for it to finish or stop it first.",
            ));
        }
        let token = CancellationToken::new();
        turns.insert(conversation_id.to_owned(), token.clone());
        Ok((
            TurnSlot {
                turns: Arc::clone(&self.turns),
                conversation_id: conversation_id.to_owned(),
            },
            token,
        ))
    }

    async fn engine_for_turn(&self, core: &CoreState) -> CoreResult<AgentEngine> {
        let settings = self.settings();
        let provider = settings.active_provider;
        let model = catalog::selected_model(&settings, provider);
        let key = self.api_key(provider).await?;
        if catalog::requires_api_key(provider) && key.is_none() {
            return Err(catalog::missing_key_error(provider));
        }
        let backend: Arc<dyn AgentBackend> =
            catalog::prepare_backend(provider, key, &settings, &model).await?;
        Ok(AgentEngine::new(EngineParts {
            backend,
            reads: Arc::new(SystemReadTools::new(Arc::clone(&core.queries))),
            writes: Arc::new(SystemWriteTranslator::new(Arc::clone(&core.queries))),
            preparer: Arc::clone(&core.preparer),
            store: Arc::clone(&self.store),
            sink: Arc::clone(&self.sink),
            config: EngineConfig {
                model,
                max_tool_rounds: settings.max_tool_rounds,
                max_tokens: catalog::MAX_RESPONSE_TOKENS,
            },
        }))
    }

    /// Validates the request and the provider synchronously, then streams the turn in the
    /// background through `sentinel:agent`.
    pub async fn send_message(
        &self,
        core: &CoreState,
        conversation_id: String,
        text: String,
    ) -> CoreResult<()> {
        if !self.store.exists(&conversation_id) {
            return Err(SentinelError::invalid(
                "this conversation no longer exists; start a new one",
            ));
        }
        if text.trim().is_empty() {
            return Err(SentinelError::invalid("type a message first"));
        }
        let (slot, token) = self.reserve_turn(&conversation_id)?;
        let engine = self.engine_for_turn(core).await?;
        tauri::async_runtime::spawn(async move {
            let _slot = slot;
            // Errors were already emitted as `AgentEvent::Error`.
            let _ = engine.send_message(&conversation_id, &text, token).await;
        });
        Ok(())
    }

    fn executor(&self, core: &CoreState) -> PlanExecutor {
        PlanExecutor::new(
            Arc::clone(&core.committer),
            Arc::clone(&core.preparer),
            Arc::clone(&self.store),
            Arc::clone(&self.sink),
        )
    }

    pub async fn revise_plan_action(
        &self,
        core: &CoreState,
        plan_id: &str,
        action_id: &str,
        action: Action,
    ) -> CoreResult<Plan> {
        self.executor(core).revise(plan_id, action_id, action).await
    }

    /// Runs the user's decisions, then lets the model report the measured results.
    pub async fn execute_plan(
        &self,
        core: &CoreState,
        plan_id: &str,
        decisions: Vec<ActionDecision>,
    ) -> CoreResult<Plan> {
        let plan = self.executor(core).execute(plan_id, &decisions).await?;
        let anything_ran = plan
            .actions
            .iter()
            .any(|a| !matches!(a.state, PlanActionState::Rejected));
        if anything_ran {
            self.report_in_background(core, plan.clone()).await;
        }
        Ok(plan)
    }

    async fn report_in_background(&self, core: &CoreState, plan: Plan) {
        let conversation_id = plan.conversation_id.clone();
        let prepared = match self.reserve_turn(&conversation_id) {
            Ok((slot, token)) => self
                .engine_for_turn(core)
                .await
                .map(|engine| (slot, token, engine)),
            Err(err) => Err(err),
        };
        let (slot, token, engine) = match prepared {
            Ok(parts) => parts,
            Err(err) => {
                self.sink.emit(AgentStreamPayload {
                    conversation_id,
                    event: AgentEvent::Error {
                        error: ErrorPayload::from(SentinelError::Unavailable {
                            feature: "result summary".into(),
                            reason: format!(
                                "the plan ran, but the agent could not summarize the results: {err}"
                            ),
                        }),
                    },
                });
                return;
            }
        };
        tauri::async_runtime::spawn(async move {
            let _slot = slot;
            let _ = engine.report_execution(&plan, token).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemorySecrets(Mutex<HashMap<ProviderId, String>>);

    impl SecretStore for MemorySecrets {
        fn get(&self, provider: ProviderId) -> CoreResult<Option<String>> {
            Ok(lock(&self.0).get(&provider).cloned())
        }
        fn set(&self, provider: ProviderId, secret: &str) -> CoreResult<()> {
            lock(&self.0).insert(provider, secret.to_owned());
            Ok(())
        }
        fn delete(&self, provider: ProviderId) -> CoreResult<()> {
            lock(&self.0).remove(&provider);
            Ok(())
        }
    }

    struct NullSink;
    impl AgentEventSink for NullSink {
        fn emit(&self, _payload: AgentStreamPayload) {}
    }

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("sentinel-agent-state-{}", uuid_like()))
    }

    fn uuid_like() -> String {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{}-{n}", std::process::id(), now_ms())
    }

    fn state(dir: &Path, secrets: Arc<MemorySecrets>) -> AgentState {
        AgentState::new(dir.to_path_buf(), secrets, Arc::new(NullSink)).unwrap()
    }

    #[test]
    fn settings_persist_without_secrets() {
        let dir = temp_dir();
        let secrets = Arc::new(MemorySecrets::default());
        let first = state(&dir, Arc::clone(&secrets));
        let mut settings = first.settings();
        settings.active_provider = ProviderId::Gemini;
        settings
            .selected_models
            .insert(ProviderId::Gemini, "gemini-3.5-flash-lite".into());
        first.update_settings(settings).unwrap();

        let reloaded = state(&dir, secrets);
        assert_eq!(reloaded.settings().active_provider, ProviderId::Gemini);
        let raw = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        assert!(!raw.to_lowercase().contains("key"), "{raw}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn keys_are_required_and_removable() {
        let dir = temp_dir();
        let secrets = Arc::new(MemorySecrets::default());
        let s = state(&dir, Arc::clone(&secrets));
        assert_eq!(s.key_status(ProviderId::Ollama), KeyStatus::NotRequired);
        assert_eq!(s.key_status(ProviderId::Anthropic), KeyStatus::Missing);
        assert!(s.set_api_key(ProviderId::Ollama, "x".into()).await.is_err());
        assert!(
            s.set_api_key(ProviderId::Openai, "  ".into())
                .await
                .is_err()
        );

        // A key stored earlier is found, and deleting it clears both store and status.
        secrets.set(ProviderId::Openai, "sk-test").unwrap();
        s.record_status(
            ProviderId::Openai,
            Some(KeyStatus::Valid { checked_at_ms: 1 }),
        )
        .unwrap();
        assert_eq!(
            s.api_key(ProviderId::Openai).await.unwrap().as_deref(),
            Some("sk-test")
        );
        s.delete_api_key(ProviderId::Openai).await.unwrap();
        assert_eq!(s.key_status(ProviderId::Openai), KeyStatus::Missing);
        assert_eq!(secrets.get(ProviderId::Openai).unwrap(), None);
        let status_file = std::fs::read_to_string(dir.join(KEY_STATUS_FILE)).unwrap();
        assert!(!status_file.contains("sk-test"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_conversation_runs_one_turn_at_a_time() {
        let dir = temp_dir();
        let s = state(&dir, Arc::new(MemorySecrets::default()));
        let id = s.new_conversation();
        let (slot, token) = s.reserve_turn(&id).unwrap();
        assert!(s.reserve_turn(&id).is_err());
        s.cancel(&id);
        assert!(token.is_cancelled());
        drop(slot);
        assert!(s.reserve_turn(&id).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
