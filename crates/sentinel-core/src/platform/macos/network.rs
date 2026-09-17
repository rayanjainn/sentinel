//! Sockets and per-connection byte counters from the `pcblist_n` sysctls — the same kernel source
//! as `netstat -anvb`, readable without privileges and covering every process.

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::InterfaceCounters;
use crate::error::{CoreResult, SentinelError};
use crate::model::{
    ConnectionTraffic, ProcessTraffic, RawSocket, TrafficReport, TrafficSource, TransportProtocol,
};
use crate::parse::pcblist::{self, PcbRecord};
use crate::provider::NetworkProvider;

/// `sockets()` and `traffic()` run back to back each tick; one kernel read serves both.
const CACHE_FOR: Duration = Duration::from_millis(250);

pub(crate) struct MacNetwork {
    interfaces: InterfaceCounters,
    cache: Option<(Instant, Vec<PcbRecord>, Vec<PcbRecord>)>,
    buffer: Vec<u8>,
}

impl MacNetwork {
    pub fn new() -> Self {
        Self {
            interfaces: InterfaceCounters::new(),
            cache: None,
            buffer: Vec::new(),
        }
    }

    fn records(&mut self) -> CoreResult<(&[PcbRecord], &[PcbRecord])> {
        let stale = self
            .cache
            .as_ref()
            .is_none_or(|(at, _, _)| at.elapsed() > CACHE_FOR);
        if stale {
            read_sysctl("net.inet.tcp.pcblist_n", &mut self.buffer).map_err(sysctl_error)?;
            let tcp = pcblist::parse(&self.buffer, true);
            read_sysctl("net.inet.udp.pcblist_n", &mut self.buffer).map_err(sysctl_error)?;
            let udp = pcblist::parse(&self.buffer, false);
            self.cache = Some((Instant::now(), tcp, udp));
        }
        let (_, tcp, udp) = self
            .cache
            .as_ref()
            .ok_or_else(|| SentinelError::internal("socket cache empty"))?;
        Ok((tcp, udp))
    }
}

fn sysctl_error(err: io::Error) -> SentinelError {
    SentinelError::Io {
        detail: format!("could not read the socket table: {err}"),
        path: None,
    }
}

fn read_sysctl(name: &str, buffer: &mut Vec<u8>) -> io::Result<()> {
    let cname = CString::new(name).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    for _ in 0..4 {
        let mut len: libc::size_t = 0;
        // SAFETY: a null buffer asks the kernel for the required length.
        let rc = unsafe {
            libc::sysctlbyname(
                cname.as_ptr(),
                std::ptr::null_mut(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        // Sockets come and go between the two calls; leave headroom.
        let capacity = len + len / 4 + 16 * 1024;
        buffer.clear();
        buffer.resize(capacity, 0);
        let mut filled = capacity;
        // SAFETY: buffer is writable for `filled` bytes.
        let rc = unsafe {
            libc::sysctlbyname(
                cname.as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut filled,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 {
            buffer.truncate(filled);
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ENOMEM) {
            return Err(err);
        }
    }
    Err(io::Error::from_raw_os_error(libc::ENOMEM))
}

fn remote(record: &PcbRecord) -> (Option<IpAddr>, Option<u16>) {
    if record.remote_addr.is_unspecified() && record.remote_port == 0 {
        (None, None)
    } else {
        (Some(record.remote_addr), Some(record.remote_port))
    }
}

impl NetworkProvider for MacNetwork {
    fn sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
        let (tcp, udp) = self.records()?;
        let convert = |protocol: TransportProtocol, record: &PcbRecord| {
            let (remote_addr, remote_port) = remote(record);
            RawSocket {
                protocol,
                local_addr: record.local_addr,
                local_port: record.local_port,
                remote_addr,
                remote_port,
                state: record.tcp_state.map(pcblist::bsd_tcp_state),
                pid: record.pid,
            }
        };
        Ok(tcp
            .iter()
            .map(|r| convert(TransportProtocol::Tcp, r))
            .chain(udp.iter().map(|r| convert(TransportProtocol::Udp, r)))
            .collect())
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        Ok(self.interfaces.totals())
    }

    fn traffic(&mut self) -> CoreResult<TrafficReport> {
        let (tcp, udp) = self.records()?;
        let mut per_process: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut connections = Vec::with_capacity(tcp.len() + udp.len());
        for (protocol, record) in tcp
            .iter()
            .map(|r| (TransportProtocol::Tcp, r))
            .chain(udp.iter().map(|r| (TransportProtocol::Udp, r)))
        {
            if let Some(pid) = record.pid {
                let entry = per_process.entry(pid).or_default();
                entry.0 = entry.0.saturating_add(record.rx_bytes);
                entry.1 = entry.1.saturating_add(record.tx_bytes);
            }
            let (remote_addr, remote_port) = remote(record);
            connections.push(ConnectionTraffic {
                protocol,
                local_addr: record.local_addr,
                local_port: record.local_port,
                remote_addr,
                remote_port,
                bytes_in: record.rx_bytes,
                bytes_out: record.tx_bytes,
            });
        }
        Ok(TrafficReport {
            source: TrafficSource::PerConnection,
            connections,
            processes: per_process
                .into_iter()
                .map(|(pid, (bytes_in, bytes_out))| ProcessTraffic {
                    pid,
                    bytes_in,
                    bytes_out,
                })
                .collect(),
        })
    }
}
