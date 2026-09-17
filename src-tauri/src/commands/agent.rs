use sentinel_agent::events::TranscriptItem;
use sentinel_agent::plan::{ActionDecision, Plan};
use sentinel_agent::settings::{
    AgentSettings, KeyStatus, ModelInfo, OllamaStatus, ProviderDescriptor, ProviderId,
    ProviderStatus,
};
use sentinel_core::action::Action;

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn agent_list_providers() -> CmdResult<Vec<ProviderDescriptor>> {
    not_wired("agent_list_providers")
}

#[tauri::command]
pub async fn agent_get_settings() -> CmdResult<AgentSettings> {
    not_wired("agent_get_settings")
}

#[tauri::command]
pub async fn agent_update_settings(settings: AgentSettings) -> CmdResult<AgentSettings> {
    let _ = settings;
    not_wired("agent_update_settings")
}

#[tauri::command]
pub async fn agent_provider_status() -> CmdResult<Vec<ProviderStatus>> {
    not_wired("agent_provider_status")
}

#[tauri::command]
pub async fn agent_set_api_key(provider: ProviderId, key: String) -> CmdResult<KeyStatus> {
    let _ = (provider, key);
    not_wired("agent_set_api_key")
}

#[tauri::command]
pub async fn agent_delete_api_key(provider: ProviderId) -> CmdResult<()> {
    let _ = provider;
    not_wired("agent_delete_api_key")
}

#[tauri::command]
pub async fn agent_list_models(provider: ProviderId) -> CmdResult<Vec<ModelInfo>> {
    let _ = provider;
    not_wired("agent_list_models")
}

#[tauri::command]
pub async fn agent_ollama_status() -> CmdResult<OllamaStatus> {
    not_wired("agent_ollama_status")
}

#[tauri::command]
pub async fn agent_new_conversation() -> CmdResult<String> {
    not_wired("agent_new_conversation")
}

#[tauri::command]
pub async fn agent_send_message(conversation_id: String, text: String) -> CmdResult<()> {
    let _ = (conversation_id, text);
    not_wired("agent_send_message")
}

#[tauri::command]
pub async fn agent_cancel(conversation_id: String) -> CmdResult<()> {
    let _ = conversation_id;
    not_wired("agent_cancel")
}

#[tauri::command]
pub async fn agent_get_transcript(conversation_id: String) -> CmdResult<Vec<TranscriptItem>> {
    let _ = conversation_id;
    not_wired("agent_get_transcript")
}

#[tauri::command]
pub async fn agent_revise_plan_action(
    plan_id: String,
    action_id: String,
    action: Action,
) -> CmdResult<Plan> {
    let _ = (plan_id, action_id, action);
    not_wired("agent_revise_plan_action")
}

#[tauri::command]
pub async fn agent_execute_plan(
    plan_id: String,
    decisions: Vec<ActionDecision>,
) -> CmdResult<Plan> {
    let _ = (plan_id, decisions);
    not_wired("agent_execute_plan")
}
