pub mod firewall;
pub mod network;
pub mod permissions;
pub mod process;
pub mod resources;
pub mod storage;

pub use firewall::*;
pub use network::*;
pub use permissions::*;
pub use process::*;
pub use resources::*;
pub use storage::*;

pub type Pid = u32;

/// Milliseconds since the Unix epoch.
pub type TimestampMs = u64;

/// Seconds since the Unix epoch.
pub type TimestampSecs = u64;
