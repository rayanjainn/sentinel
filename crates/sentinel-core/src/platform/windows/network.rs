//! Sockets from `GetExtendedTcpTable` / `GetExtendedUdpTable` (owner PID variants, IPv4 + IPv6);
//! per-connection bytes from TCP extended statistics when Sentinel runs elevated.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv6Addr};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, GetPerTcp6ConnectionEStats,
    GetPerTcpConnectionEStats, MIB_TCP_STATE, MIB_TCP6ROW, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_LH,
    MIB_TCPROW_LH_0, MIB_TCPROW_OWNER_PID, MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID,
    SetPerTcp6ConnectionEStats, SetPerTcpConnectionEStats, TCP_ESTATS_DATA_ROD_v0,
    TCP_ESTATS_DATA_RW_v0, TCP_TABLE_OWNER_PID_ALL, TcpConnectionEstatsData, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, IN6_ADDR, IN6_ADDR_0};

use super::resources::InterfaceCounters;
use crate::error::{CoreResult, SentinelError};
use crate::model::{
    ConnectionTraffic, Pid, ProcessTraffic, RawSocket, TrafficReport, TrafficSource,
    TransportProtocol,
};
use crate::parse::sockets::{windows_ipv4, windows_port, windows_tcp_state};
use crate::provider::NetworkProvider;

const MIB_TCP_STATE_ESTAB: u32 = 5;

type ConnKey = (IpAddr, u16, IpAddr, u16);

pub(crate) struct WindowsNetwork {
    interfaces: InterfaceCounters,
    /// Connections whose extended statistics collection Sentinel already switched on.
    collecting: HashSet<ConnKey>,
}

impl WindowsNetwork {
    pub fn new() -> Self {
        Self {
            interfaces: InterfaceCounters,
            collecting: HashSet::new(),
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

fn tcp4_rows() -> CoreResult<Vec<MIB_TCPROW_OWNER_PID>> {
    let table = fetch(|table, size| {
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
    Ok(rows(&table))
}

fn tcp6_rows() -> CoreResult<Vec<MIB_TCP6ROW_OWNER_PID>> {
    let table = fetch(|table, size| {
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
    Ok(rows(&table))
}

fn remote(addr: IpAddr, port: u16) -> (Option<IpAddr>, Option<u16>) {
    if addr.is_unspecified() && port == 0 {
        (None, None)
    } else {
        (Some(addr), Some(port))
    }
}

fn as_bytes<T>(value: &T) -> &[u8] {
    // SAFETY: reading the object representation of a plain-old-data struct.
    unsafe {
        std::slice::from_raw_parts((value as *const T).cast::<u8>(), std::mem::size_of::<T>())
    }
}

fn read_rod(bytes: &[u8]) -> TCP_ESTATS_DATA_ROD_v0 {
    // SAFETY: the buffer is exactly one TCP_ESTATS_DATA_ROD_v0 filled by the API.
    unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<TCP_ESTATS_DATA_ROD_v0>()) }
}

impl NetworkProvider for WindowsNetwork {
    fn sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
        let mut sockets = Vec::new();
        for row in tcp4_rows()? {
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
        for row in tcp6_rows()? {
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
        let interface_only = TrafficReport {
            source: TrafficSource::InterfaceOnly,
            connections: Vec::new(),
            processes: Vec::new(),
        };
        // Enabling extended statistics collection requires administrator rights.
        if !super::permissions::is_elevated() {
            return Ok(interface_only);
        }
        let enable = TCP_ESTATS_DATA_RW_v0 {
            EnableCollection: true,
        };
        let mut rod = vec![0u8; std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>()];
        let mut connections = Vec::new();
        let mut per_process: HashMap<Pid, (u64, u64)> = HashMap::new();
        let mut seen = HashSet::new();

        for owner in tcp4_rows()?
            .into_iter()
            .filter(|r| r.dwState == MIB_TCP_STATE_ESTAB)
        {
            let local = IpAddr::V4(windows_ipv4(owner.dwLocalAddr));
            let remote_ip = IpAddr::V4(windows_ipv4(owner.dwRemoteAddr));
            let key = (
                local,
                windows_port(owner.dwLocalPort),
                remote_ip,
                windows_port(owner.dwRemotePort),
            );
            let row = MIB_TCPROW_LH {
                Anonymous: MIB_TCPROW_LH_0 {
                    dwState: owner.dwState,
                },
                dwLocalAddr: owner.dwLocalAddr,
                dwLocalPort: owner.dwLocalPort,
                dwRemoteAddr: owner.dwRemoteAddr,
                dwRemotePort: owner.dwRemotePort,
            };
            seen.insert(key);
            if self.collecting.insert(key) {
                // SAFETY: row and rw buffer are valid for the call.
                unsafe {
                    SetPerTcpConnectionEStats(
                        &row,
                        TcpConnectionEstatsData,
                        as_bytes(&enable),
                        0,
                        0,
                    )
                };
            }
            // SAFETY: rod buffer is sized for TCP_ESTATS_DATA_ROD_v0.
            let status = unsafe {
                GetPerTcpConnectionEStats(
                    &row,
                    TcpConnectionEstatsData,
                    None,
                    0,
                    None,
                    0,
                    Some(&mut rod),
                    0,
                )
            };
            if status == NO_ERROR.0 {
                let data = read_rod(&rod);
                let entry = per_process.entry(owner.dwOwningPid).or_default();
                entry.0 += data.DataBytesIn;
                entry.1 += data.DataBytesOut;
                connections.push(ConnectionTraffic {
                    protocol: TransportProtocol::Tcp,
                    local_addr: key.0,
                    local_port: key.1,
                    remote_addr: Some(key.2),
                    remote_port: Some(key.3),
                    bytes_in: data.DataBytesIn,
                    bytes_out: data.DataBytesOut,
                });
            }
        }

        for owner in tcp6_rows()?
            .into_iter()
            .filter(|r| r.dwState == MIB_TCP_STATE_ESTAB)
        {
            let key = (
                IpAddr::V6(Ipv6Addr::from(owner.ucLocalAddr)),
                windows_port(owner.dwLocalPort),
                IpAddr::V6(Ipv6Addr::from(owner.ucRemoteAddr)),
                windows_port(owner.dwRemotePort),
            );
            let row = MIB_TCP6ROW {
                State: MIB_TCP_STATE(owner.dwState as i32),
                LocalAddr: IN6_ADDR {
                    u: IN6_ADDR_0 {
                        Byte: owner.ucLocalAddr,
                    },
                },
                dwLocalScopeId: owner.dwLocalScopeId,
                dwLocalPort: owner.dwLocalPort,
                RemoteAddr: IN6_ADDR {
                    u: IN6_ADDR_0 {
                        Byte: owner.ucRemoteAddr,
                    },
                },
                dwRemoteScopeId: owner.dwRemoteScopeId,
                dwRemotePort: owner.dwRemotePort,
            };
            seen.insert(key);
            if self.collecting.insert(key) {
                // SAFETY: row and rw buffer are valid for the call.
                unsafe {
                    SetPerTcp6ConnectionEStats(
                        &row,
                        TcpConnectionEstatsData,
                        as_bytes(&enable),
                        0,
                        0,
                    )
                };
            }
            // SAFETY: rod buffer is sized for TCP_ESTATS_DATA_ROD_v0.
            let status = unsafe {
                GetPerTcp6ConnectionEStats(
                    &row,
                    TcpConnectionEstatsData,
                    None,
                    0,
                    None,
                    0,
                    Some(&mut rod),
                    0,
                )
            };
            if status == NO_ERROR.0 {
                let data = read_rod(&rod);
                let entry = per_process.entry(owner.dwOwningPid).or_default();
                entry.0 += data.DataBytesIn;
                entry.1 += data.DataBytesOut;
                connections.push(ConnectionTraffic {
                    protocol: TransportProtocol::Tcp,
                    local_addr: key.0,
                    local_port: key.1,
                    remote_addr: Some(key.2),
                    remote_port: Some(key.3),
                    bytes_in: data.DataBytesIn,
                    bytes_out: data.DataBytesOut,
                });
            }
        }

        self.collecting.retain(|key| seen.contains(key));
        if connections.is_empty() && !seen.is_empty() {
            return Ok(interface_only);
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
