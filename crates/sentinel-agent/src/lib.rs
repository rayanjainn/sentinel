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
//!
//! The engine's parts compile with a preparer:
//!
//! ```
//! use std::sync::Arc;
//! use sentinel_agent::engine::EngineParts;
//! use sentinel_core::action::ActionPreparer;
//! fn rebuild(parts: EngineParts, preparer: Arc<dyn ActionPreparer>) -> EngineParts {
//!     EngineParts { preparer, ..parts }
//! }
//! ```
//!
//! …but there is no field for a committer:
//!
//! ```compile_fail
//! use std::sync::Arc;
//! use sentinel_agent::engine::EngineParts;
//! use sentinel_core::action::ActionCommitter;
//! fn smuggle(parts: EngineParts, committer: Arc<dyn ActionCommitter>) -> EngineParts {
//!     EngineParts { committer, ..parts }
//! }
//! ```
//!
//! …and a committer cannot stand in for the preparer:
//!
//! ```compile_fail
//! use std::sync::Arc;
//! use sentinel_agent::engine::EngineParts;
//! use sentinel_core::action::ActionCommitter;
//! fn smuggle(parts: EngineParts, preparer: Arc<dyn ActionCommitter>) -> EngineParts {
//!     EngineParts { preparer, ..parts }
//! }
//! ```

pub mod backend;
pub mod conversation;
pub mod engine;
pub mod events;
pub mod executor;
pub mod plan;
pub mod prompt;
pub mod providers;
pub mod settings;
pub mod tools;
