//! Tool registry contract.
//!
//! Read tools execute immediately against live state. Write tools are never executed by the agent:
//! they are translated into a core [`Action`], passed through `ActionPreparer::prepare`, and queued
//! as a plan card. The model receives a tool result saying the action is awaiting user review.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use sentinel_core::CoreResult;
use sentinel_core::action::Action;

pub mod paths;
pub mod read;
pub mod specs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolAccess {
    Read,
    Write,
}

pub mod names {
    // Read
    pub const GET_SYSTEM_OVERVIEW: &str = "get_system_overview";
    pub const GET_RESOURCE_USAGE: &str = "get_resource_usage";
    pub const LIST_PROCESSES: &str = "list_processes";
    pub const GET_PROCESS_DETAILS: &str = "get_process_details";
    pub const LIST_NETWORK_SOCKETS: &str = "list_network_sockets";
    pub const FIND_PORT_OWNER: &str = "find_port_owner";
    pub const LIST_VOLUMES: &str = "list_volumes";
    pub const SCAN_STORAGE: &str = "scan_storage";
    pub const GET_STORAGE_BREAKDOWN: &str = "get_storage_breakdown";
    pub const FIND_LARGE_FILES: &str = "find_large_files";
    pub const GET_CLEANUP_SUGGESTIONS: &str = "get_cleanup_suggestions";
    pub const FIND_DUPLICATE_FILES: &str = "find_duplicate_files";
    pub const LIST_FIREWALL_RULES: &str = "list_firewall_rules";
    // Write
    pub const TERMINATE_PROCESS: &str = "terminate_process";
    pub const FORCE_KILL_PROCESS: &str = "force_kill_process";
    pub const SET_PROCESS_PRIORITY: &str = "set_process_priority";
    pub const MOVE_TO_TRASH: &str = "move_to_trash";
    pub const MOVE_PATHS: &str = "move_paths";
    pub const BLOCK_REMOTE_IP: &str = "block_remote_ip";
    pub const BLOCK_LOCAL_PORT: &str = "block_local_port";
    pub const REMOVE_FIREWALL_RULE: &str = "remove_firewall_rule";
}

/// Source of truth for tool classification. Unknown names are rejected, never guessed.
pub const TOOL_ACCESS: &[(&str, ToolAccess)] = {
    use ToolAccess::{Read, Write};
    use names::*;
    &[
        (GET_SYSTEM_OVERVIEW, Read),
        (GET_RESOURCE_USAGE, Read),
        (LIST_PROCESSES, Read),
        (GET_PROCESS_DETAILS, Read),
        (LIST_NETWORK_SOCKETS, Read),
        (FIND_PORT_OWNER, Read),
        (LIST_VOLUMES, Read),
        (SCAN_STORAGE, Read),
        (GET_STORAGE_BREAKDOWN, Read),
        (FIND_LARGE_FILES, Read),
        (GET_CLEANUP_SUGGESTIONS, Read),
        (FIND_DUPLICATE_FILES, Read),
        (LIST_FIREWALL_RULES, Read),
        (TERMINATE_PROCESS, Write),
        (FORCE_KILL_PROCESS, Write),
        (SET_PROCESS_PRIORITY, Write),
        (MOVE_TO_TRASH, Write),
        (MOVE_PATHS, Write),
        (BLOCK_REMOTE_IP, Write),
        (BLOCK_LOCAL_PORT, Write),
        (REMOVE_FIREWALL_RULE, Write),
    ]
};

pub fn access_of(name: &str) -> Option<ToolAccess> {
    TOOL_ACCESS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|(_, access)| *access)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub access: ToolAccess,
    pub input_schema: Value,
}

#[async_trait]
pub trait ReadToolExecutor: Send + Sync {
    /// Returns compact JSON for the model (large lists truncated with a count).
    async fn call(&self, name: &str, input: Value) -> CoreResult<Value>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposedWrite {
    pub action: Action,
    /// The model's stated reason, shown on the plan card.
    pub rationale: String,
}

pub trait WriteToolTranslator: Send + Sync {
    /// Maps tool input to an `Action`, resolving live identities (pid → start time) and
    /// canonical paths. Never performs the action.
    fn translate(&self, name: &str, input: Value) -> CoreResult<ProposedWrite>;
}
