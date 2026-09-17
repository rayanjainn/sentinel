//! Sockets from `GetExtendedTcpTable` / `GetExtendedUdpTable` (owner PID variants, IPv4 + IPv6).

use std::net::{IpAddr, Ipv6Addr};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

use super::resources::InterfaceCounters;
use crate::error::{CoreResult, SentinelError};
use crate::model::{RawSocket, TrafficReport, TrafficSource, TransportProtocol};
use crate::parse::sockets::{windows_ipv4, windows_port, windows_tcp_state};
use crate::provider::NetworkProvider;

pub(crate) struct WindowsNetwork {
    interfaces: InterfaceCounters,
}

impl WindowsNetwork {
    pub fn new() -> Self {
        Self {
            interfaces: InterfaceCounters,
        }
    }
}

/// Calls an IP Helper table function until the buffer is large enough; returns the raw table.
fn fetch(
    mut call: impl FnMut(Option<*mut core::ffi::c_void>, &mut u32) -> u32,
) -> CoreResult<Vec<u8>> {
    let mut size = 0u32;
    let mut buffer: Vec<u8> = Vec::new();
    for _ in 0..5 {
        let pointer = if buffer.is_empty() {
            None
        } else {
            Some(buffer.as_mut_ptr().cast())
        };
        let status = call(pointer, &mut size);
        if status == NO_ERROR.0 && !buffer.is_empty() {
            return Ok(buffer);
        }
        if status == ERROR_INSUFFICIENT_BUFFER.0 || (status == NO_ERROR.0 && buffer.is_empty()) {
            // Connections appear between calls; over-allocate a little.
            buffer = vec![0u8; size as usize + 4096];
            size = buffer.len() as u32;
            continue;
        }
        return Err(SentinelError::internal(format!(
            "reading the socket table failed with error {status}"
        )));
    }
    Err(SentinelError::internal(
        "socket table kept growing while being read",
    ))
}

/// Rows following the `dwNumEntries` header, aligned for `T`.
fn rows<T: Copy>(buffer: &[u8]) -> Vec<T> {
    if buffer.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let align = std::mem::align_of::<T>().max(4);
    let start = 4usize.div_ceil(align) * align;
    let size = std::mem::size_of::<T>();
    (0..count)
        .map_while(|index| {
            let offset = start + index * size;
            buffer.get(offset..offset + size).map(|bytes| {
                // SAFETY: `bytes` holds exactly size_of::<T>() bytes of a plain-old-data row.
                unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<T>()) }
            })
        })
        .collect()
}

fn remote(addr: IpAddr, port: u16) -> (Option<IpAddr>, Option<u16>) {
    if addr.is_unspecified() && port == 0 {
        (None, None)
    } else {
        (Some(addr), Some(port))
    }
}

impl NetworkProvider for WindowsNetwork {
    fn sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
        let mut sockets = Vec::new();

        let tcp4 = fetch(|table, size| {
            // SAFETY: table/size describe a caller-owned buffer (or a size query).
            unsafe {
                GetExtendedTcpTable(
                    table,
                    size,
                    false,
                    u32::from(AF_INET.0),
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            }
        })?;
        for row in rows::<MIB_TCPROW_OWNER_PID>(&tcp4) {
            let (remote_addr, remote_port) = remote(
                IpAddr::V4(windows_ipv4(row.dwRemoteAddr)),
                windows_port(row.dwRemotePort),
            );
            sockets.push(RawSocket {
                protocol: TransportProtocol::Tcp,
                local_addr: IpAddr::V4(windows_ipv4(row.dwLocalAddr)),
                local_port: windows_port(row.dwLocalPort),
                remote_addr,
                remote_port,
                state: Some(windows_tcp_state(row.dwState)),
                pid: Some(row.dwOwningPid),
            });
        }

        let tcp6 = fetch(|table, size| {
            // SAFETY: as above.
            unsafe {
                GetExtendedTcpTable(
                    table,
                    size,
                    false,
                    u32::from(AF_INET6.0),
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            }
        })?;
        for row in rows::<MIB_TCP6ROW_OWNER_PID>(&tcp6) {
            let (remote_addr, remote_port) = remote(
                IpAddr::V6(Ipv6Addr::from(row.ucRemoteAddr)),
                windows_port(row.dwRemotePort),
            );
            sockets.push(RawSocket {
                protocol: TransportProtocol::Tcp,
                local_addr: IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                local_port: windows_port(row.dwLocalPort),
                remote_addr,
                remote_port,
                state: Some(windows_tcp_state(row.dwState)),
                pid: Some(row.dwOwningPid),
            });
        }

        let udp4 = fetch(|table, size| {
            // SAFETY: as above.
            unsafe {
                GetExtendedUdpTable(
                    table,
                    size,
                    false,
                    u32::from(AF_INET.0),
                    UDP_TABLE_OWNER_PID,
                    0,
                )
            }
        })?;
        for row in rows::<MIB_UDPROW_OWNER_PID>(&udp4) {
            sockets.push(RawSocket {
                protocol: TransportProtocol::Udp,
                local_addr: IpAddr::V4(windows_ipv4(row.dwLocalAddr)),
                local_port: windows_port(row.dwLocalPort),
                remote_addr: None,
                remote_port: None,
                state: None,
                pid: Some(row.dwOwningPid),
            });
        }

        let udp6 = fetch(|table, size| {
            // SAFETY: as above.
            unsafe {
                GetExtendedUdpTable(
                    table,
                    size,
                    false,
                    u32::from(AF_INET6.0),
                    UDP_TABLE_OWNER_PID,
                    0,
                )
            }
        })?;
        for row in rows::<MIB_UDP6ROW_OWNER_PID>(&udp6) {
            sockets.push(RawSocket {
                protocol: TransportProtocol::Udp,
                local_addr: IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                local_port: windows_port(row.dwLocalPort),
                remote_addr: None,
                remote_port: None,
                state: None,
                pid: Some(row.dwOwningPid),
            });
        }
        Ok(sockets)
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        self.interfaces.totals()
    }

    fn traffic(&mut self) -> CoreResult<TrafficReport> {
        Ok(TrafficReport {
            source: TrafficSource::InterfaceOnly,
            connections: Vec::new(),
            processes: Vec::new(),
        })
    }
}
