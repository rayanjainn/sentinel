pub mod actions;
pub mod agent;
pub mod firewall;
pub mod network;
pub mod process;
pub mod storage;
pub mod system;

use sentinel_core::ErrorPayload;

pub type CmdResult<T> = Result<T, ErrorPayload>;
