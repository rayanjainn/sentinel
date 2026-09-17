//! Agent commands. Thin wrappers over `AgentState`; see `agent_state.rs` for behavior.

use sentinel_agent::events::TranscriptItem;
use sentinel_agent::plan::{ActionDecision, Plan};
use sentinel_agent::settings::{
    AgentSettings, KeyStatus, ModelInfo, OllamaStatus, ProviderDescriptor, ProviderId,
    ProviderStatus,
};
use sentinel_core::action::Action;
use tauri::State;

use super::CmdResult;
use crate::agent_state::AgentState;
use crate::state::CoreState;

#[tauri::command]
pub async fn agent_list_providers(
    agent: State<'_, AgentState>,
) -> CmdResult<Vec<ProviderDescriptor>> {
    Ok(agent.descriptors())
}

#[tauri::command]
pub async fn agent_get_settings(agent: State<'_, AgentState>) -> CmdResult<AgentSettings> {
    Ok(agent.settings())
}

#[tauri::command]
pub async fn agent_update_settings(
    agent: State<'_, AgentState>,
    settings: AgentSettings,
) -> CmdResult<AgentSettings> {
    Ok(agent.update_settings(settings)?)
}

#[tauri::command]
pub async fn agent_provider_status(agent: State<'_, AgentState>) -> CmdResult<Vec<ProviderStatus>> {
    Ok(agent.provider_status().await)
}

#[tauri::command]
pub async fn agent_set_api_key(
    agent: State<'_, AgentState>,
    provider: ProviderId,
    key: String,
) -> CmdResult<KeyStatus> {
    Ok(agent.set_api_key(provider, key).await?)
}

#[tauri::command]
pub async fn agent_delete_api_key(
    agent: State<'_, AgentState>,
    provider: ProviderId,
) -> CmdResult<()> {
    Ok(agent.delete_api_key(provider).await?)
}

#[tauri::command]
pub async fn agent_list_models(
    agent: State<'_, AgentState>,
    provider: ProviderId,
) -> CmdResult<Vec<ModelInfo>> {
    Ok(agent.list_models(provider).await?)
}

#[tauri::command]
pub async fn agent_ollama_status(agent: State<'_, AgentState>) -> CmdResult<OllamaStatus> {
    Ok(agent.ollama_status().await)
}

#[tauri::command]
pub async fn agent_new_conversation(agent: State<'_, AgentState>) -> CmdResult<String> {
    Ok(agent.new_conversation())
}

#[tauri::command]
pub async fn agent_send_message(
    agent: State<'_, AgentState>,
    core: State<'_, CoreState>,
    conversation_id: String,
    text: String,
) -> CmdResult<()> {
    Ok(agent.send_message(&core, conversation_id, text).await?)
}

#[tauri::command]
pub async fn agent_cancel(agent: State<'_, AgentState>, conversation_id: String) -> CmdResult<()> {
    agent.cancel(&conversation_id);
    Ok(())
}

#[tauri::command]
pub async fn agent_get_transcript(
    agent: State<'_, AgentState>,
    conversation_id: String,
) -> CmdResult<Vec<TranscriptItem>> {
    Ok(agent.transcript(&conversation_id)?)
}

#[tauri::command]
pub async fn agent_revise_plan_action(
    agent: State<'_, AgentState>,
    core: State<'_, CoreState>,
    plan_id: String,
    action_id: String,
    action: Action,
) -> CmdResult<Plan> {
    Ok(agent
        .revise_plan_action(&core, &plan_id, &action_id, action)
        .await?)
}

#[tauri::command]
pub async fn agent_execute_plan(
    agent: State<'_, AgentState>,
    core: State<'_, CoreState>,
    plan_id: String,
    decisions: Vec<ActionDecision>,
) -> CmdResult<Plan> {
    Ok(agent.execute_plan(&core, &plan_id, decisions).await?)
}
