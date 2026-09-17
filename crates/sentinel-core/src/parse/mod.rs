//! Platform-independent parsers for OS text formats (procfs, tool output, tz tables). Kept free of
//! OS calls so they are unit-tested on every host.

pub mod netsh;
pub mod procfs;
