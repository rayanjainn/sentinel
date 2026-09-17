//! Sockets from `/proc/net/{tcp,tcp6,udp,udp6}`, owners from the `/proc/*/fd` socket inode map.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::InterfaceCounters;
use crate::error::{CoreResult, SentinelError};
use crate::model::{Pid, RawSocket, TrafficReport, TrafficSource, TransportProtocol};
use crate::parse::procfs::{self, FdTarget};
use crate::parse::sockets::{self, ProcNetRow};
use crate::provider::NetworkProvider;

/// Full rebuilds scan every process's descriptors; do them rarely unless new inodes appear.
const MAP_MAX_AGE: Duration = Duration::from_secs(5);
const MAP_MIN_AGE: Duration = Duration::from_secs(1);

pub(crate) struct LinuxNetwork {
    interfaces: InterfaceCounters,
    inode_pids: HashMap<u64, Vec<Pid>>,
    built_at: Option<Instant>,
}

impl LinuxNetwork {
    pub fn new() -> Self {
        Self {
            interfaces: InterfaceCounters::new(),
            inode_pids: HashMap::new(),
            built_at: None,
        }
    }

    fn rebuild_inode_map(&mut self) {
        let mut map: HashMap<u64, Vec<Pid>> = HashMap::new();
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<Pid>().ok())
                else {
                    continue;
                };
                // Other users' descriptors are unreadable without privileges; skip quietly.
                let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
                    continue;
                };
                for fd in fds.flatten() {
                    if let Ok(link) = std::fs::read_link(fd.path())
                        && let FdTarget::Socket(inode) = procfs::fd_target(&link.to_string_lossy())
                    {
                        let owners = map.entry(inode).or_default();
                        if !owners.contains(&pid) {
                            owners.push(pid);
                        }
                    }
                }
            }
        }
        self.inode_pids = map;
        self.built_at = Some(Instant::now());
    }
}

fn read_table(path: &str) -> CoreResult<Vec<ProcNetRow>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(sockets::proc_net(&text)),
        // IPv6 disabled at boot removes the v6 tables.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(SentinelError::io(&err, Some(std::path::Path::new(path)))),
    }
}

impl NetworkProvider for LinuxNetwork {
    fn sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
        let mut rows = Vec::new();
        for (path, protocol) in [
            ("/proc/net/tcp", TransportProtocol::Tcp),
            ("/proc/net/tcp6", TransportProtocol::Tcp),
            ("/proc/net/udp", TransportProtocol::Udp),
            ("/proc/net/udp6", TransportProtocol::Udp),
        ] {
            rows.extend(read_table(path)?.into_iter().map(|row| (protocol, row)));
        }
        let age = self.built_at.map(|at| at.elapsed());
        let unknown = rows
            .iter()
            .any(|(_, row)| row.inode != 0 && !self.inode_pids.contains_key(&row.inode));
        if age.is_none_or(|a| a >= MAP_MAX_AGE || (unknown && a >= MAP_MIN_AGE)) {
            self.rebuild_inode_map();
        }
        let mut result = Vec::with_capacity(rows.len());
        for (protocol, row) in rows {
            let remote = (!(row.remote_addr.is_unspecified() && row.remote_port == 0))
                .then_some((row.remote_addr, row.remote_port));
            let state = match protocol {
                TransportProtocol::Tcp => Some(sockets::linux_tcp_state(row.state)),
                TransportProtocol::Udp => None,
            };
            let make = |pid: Option<Pid>| RawSocket {
                protocol,
                local_addr: normalize(row.local_addr),
                local_port: row.local_port,
                remote_addr: remote.map(|(addr, _)| normalize(addr)),
                remote_port: remote.map(|(_, port)| port),
                state,
                pid,
            };
            match self.inode_pids.get(&row.inode) {
                Some(owners) if !owners.is_empty() => {
                    result.extend(owners.iter().map(|pid| make(Some(*pid))));
                }
                _ => result.push(make(None)),
            }
        }
        Ok(result)
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        Ok(self.interfaces.totals())
    }

    fn traffic(&mut self) -> CoreResult<TrafficReport> {
        Ok(TrafficReport {
            source: TrafficSource::InterfaceOnly,
            connections: Vec::new(),
            processes: Vec::new(),
        })
    }
}

/// Dual-stack sockets report IPv4 peers as `::ffff:a.b.c.d`; show them as IPv4.
fn normalize(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(addr),
        v4 => v4,
    }
}
