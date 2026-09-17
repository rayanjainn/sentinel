//! Platform-independent parsers for OS text formats (procfs, tool output, tz tables). Kept free of
//! OS calls so they are unit-tested on every host.

pub mod netsh;
pub mod pcblist;
pub mod procfs;
pub mod sock_diag;
pub mod sockets;
pub mod uri;
pub mod zonetab;
