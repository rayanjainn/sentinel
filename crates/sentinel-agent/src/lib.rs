//! Agent layer.
//!
//! Safety structure:
//! - `AgentEngine` (conversation loop) is constructed with an `AgentBackend`, a `ReadToolExecutor`,
//!   a `WriteToolTranslator`, and an `ActionPreparer`. It is never given an `ActionCommitter`, so it
//!   cannot execute a write regardless of what the model asks for.
//! - A write tool call is refused (returned to the model as a tool error) unless at least one read
//!   tool ran earlier in the same turn — proposals must be grounded in live state.
//! - `PlanExecutor` holds the `ActionCommitter` and runs only from the `agent_execute_plan` Tauri
//!   command with explicit per-action decisions from the user.
//! - The flow is identical for every provider.

pub mod backend;
pub mod conversation;
pub mod engine;
pub mod events;
pub mod plan;
pub mod prompt;
pub mod providers;
pub mod settings;
pub mod tools;
