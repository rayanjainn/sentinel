//! XNU `net.inet.{tcp,udp}.pcblist_n` sysctl records (the source `netstat -anvb` reads).
//!
//! The buffer is a sequence of 8-byte-aligned items, each starting with `u32 len, u32 kind`,
//! bracketed by `struct xinpgen`. Every PCB contributes one item per kind (inpcb, socket, rcv/snd
//! buffers, stats and, for TCP, tcpcb). Layouts follow `bsd/netinet/in_pcb.h`,
//! `bsd/sys/socketvar.h` and `bsd/netinet/tcp_var.h`, which are `#pragma pack(4)`; offsets were
//! verified against live data on macOS and are bounds-checked against each item's length.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const XSO_SOCKET: u32 = 0x001;
pub const XSO_RCVBUF: u32 = 0x002;
pub const XSO_SNDBUF: u32 = 0x004;
pub const XSO_STATS: u32 = 0x008;
pub const XSO_INPCB: u32 = 0x010;
pub const XSO_TCPCB: u32 = 0x020;

const INP_IPV4: u8 = 0x1;
const INP_IPV6: u8 = 0x2;
/// Traffic classes in `xsockstat_n.xst_tc_stats` (SO_TC_STATS_MAX).
const TC_STATS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcbRecord {
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
    /// `TCPS_*` from `tcp_fsm.h`; `None` for UDP.
    pub tcp_state: Option<i32>,
    /// Effective owner (delegating app for sockets opened on its behalf), else last user.
    pub pid: Option<u32>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Default)]
struct Partial {
    kinds: u32,
    inpcb: Option<(IpAddr, u16, IpAddr, u16)>,
    pid: Option<u32>,
    state: Option<i32>,
    rx: u64,
    tx: u64,
}

fn u16_at(buf: &[u8], offset: usize) -> Option<u16> {
    buf.get(offset..offset + 2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
}

fn u32_at(buf: &[u8], offset: usize) -> Option<u32> {
    buf.get(offset..offset + 4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_at(buf: &[u8], offset: usize) -> Option<u64> {
    buf.get(offset..offset + 8).map(|b| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(b);
        u64::from_ne_bytes(bytes)
    })
}

fn addr(item: &[u8], offset: usize, vflag: u8) -> Option<IpAddr> {
    let raw = item.get(offset..offset + 16)?;
    if vflag & INP_IPV6 != 0 {
        let mut octets = [0u8; 16];
        octets.copy_from_slice(raw);
        // KAME-derived stacks embed the interface scope in bytes 2..4 of link-local addresses.
        if octets[0] == 0xfe && octets[1] & 0xc0 == 0x80 {
            octets[2] = 0;
            octets[3] = 0;
        }
        Some(IpAddr::V6(Ipv6Addr::from(octets)))
    } else {
        // in_addr_4in6: three padding words, then the IPv4 address in network order.
        Some(IpAddr::V4(Ipv4Addr::new(
            raw[12], raw[13], raw[14], raw[15],
        )))
    }
}

/// `struct xinpcb_n` (packed to 4): ports at 16/18 (network order), vflag at 44, foreign/local
/// address unions at 48/64.
fn parse_inpcb(item: &[u8]) -> Option<(IpAddr, u16, IpAddr, u16)> {
    let fport = u16::from_be(u16_at(item, 16)?);
    let lport = u16::from_be(u16_at(item, 18)?);
    let vflag = *item.get(44)?;
    if vflag & (INP_IPV4 | INP_IPV6) == 0 {
        return None;
    }
    let faddr = addr(item, 48, vflag)?;
    let laddr = addr(item, 64, vflag)?;
    Some((laddr, lport, faddr, fport))
}

/// `struct xsocket_n` (packed to 4): so_last_pid at 68, so_e_pid at 72.
fn parse_socket_pid(item: &[u8]) -> Option<u32> {
    let last = u32_at(item, 68)?;
    let effective = u32_at(item, 72)?;
    [effective, last].into_iter().find(|pid| *pid != 0)
}

/// `struct xsockstat_n`: `data_stats { rxpackets, rxbytes, txpackets, txbytes }` × 4 from offset 8.
fn parse_stats(item: &[u8]) -> Option<(u64, u64)> {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for class in 0..TC_STATS {
        let base = 8 + class * 32;
        rx = rx.saturating_add(u64_at(item, base + 8)?);
        tx = tx.saturating_add(u64_at(item, base + 24)?);
    }
    Some((rx, tx))
}

/// `struct xtcpcb_n`: t_segq (8), t_dupacks (16), t_timer[4] (20), t_state (36).
fn parse_tcp_state(item: &[u8]) -> Option<i32> {
    u32_at(item, 36).map(|v| v as i32)
}

pub fn parse(buf: &[u8], tcp: bool) -> Vec<PcbRecord> {
    let required = XSO_SOCKET
        | XSO_RCVBUF
        | XSO_SNDBUF
        | XSO_STATS
        | XSO_INPCB
        | if tcp { XSO_TCPCB } else { 0 };
    let mut records = Vec::new();
    let Some(first_len) = u32_at(buf, 0) else {
        return records;
    };
    let mut offset = round8(first_len as usize);
    let mut partial = Partial::default();
    while offset + 8 <= buf.len() {
        let Some(len) = u32_at(buf, offset).map(|l| l as usize) else {
            break;
        };
        // The trailing xinpgen has the header's length; anything shorter than a header is corrupt.
        if len <= first_len as usize || offset + len > buf.len() {
            break;
        }
        let kind = u32_at(buf, offset + 4).unwrap_or(0);
        let item = &buf[offset..offset + len];
        match kind {
            XSO_INPCB => partial.inpcb = parse_inpcb(item),
            XSO_SOCKET => partial.pid = parse_socket_pid(item),
            XSO_STATS => {
                if let Some((rx, tx)) = parse_stats(item) {
                    partial.rx = rx;
                    partial.tx = tx;
                }
            }
            XSO_TCPCB => partial.state = parse_tcp_state(item),
            _ => {}
        }
        partial.kinds |= kind;
        if partial.kinds & required == required {
            if let Some((local_addr, local_port, remote_addr, remote_port)) = partial.inpcb {
                records.push(PcbRecord {
                    local_addr,
                    local_port,
                    remote_addr,
                    remote_port,
                    tcp_state: if tcp { partial.state } else { None },
                    pid: partial.pid,
                    rx_bytes: partial.rx,
                    tx_bytes: partial.tx,
                });
            }
            partial = Partial::default();
        }
        offset += round8(len);
    }
    records
}

fn round8(n: usize) -> usize {
    (n + 7) & !7
}

/// BSD `TCPS_*` numbering (`netinet/tcp_fsm.h`).
pub fn bsd_tcp_state(state: i32) -> crate::model::TcpState {
    use crate::model::TcpState;
    match state {
        0 => TcpState::Closed,
        1 => TcpState::Listen,
        2 => TcpState::SynSent,
        3 => TcpState::SynReceived,
        4 => TcpState::Established,
        5 => TcpState::CloseWait,
        6 => TcpState::FinWait1,
        7 => TcpState::Closing,
        8 => TcpState::LastAck,
        9 => TcpState::FinWait2,
        10 => TcpState::TimeWait,
        _ => TcpState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: u32, len: usize, fill: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut bytes = vec![0u8; len];
        bytes[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        bytes[4..8].copy_from_slice(&kind.to_ne_bytes());
        fill(&mut bytes);
        bytes.resize(round8(len), 0);
        bytes
    }

    fn put_u32(buf: &mut [u8], at: usize, v: u32) {
        buf[at..at + 4].copy_from_slice(&v.to_ne_bytes());
    }

    fn put_u64(buf: &mut [u8], at: usize, v: u64) {
        buf[at..at + 8].copy_from_slice(&v.to_ne_bytes());
    }

    fn pcb(tcp: bool, v6: bool) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(item(XSO_SOCKET, 104, |b| {
            put_u32(b, 68, 321);
            put_u32(b, 72, 0);
        }));
        out.extend(item(XSO_RCVBUF, 32, |_| {}));
        out.extend(item(XSO_SNDBUF, 32, |_| {}));
        out.extend(item(XSO_STATS, 8 + 32 * 4, |b| {
            put_u64(b, 8 + 8, 1000);
            put_u64(b, 8 + 24, 50);
            put_u64(b, 8 + 32 * 2 + 8, 24);
            put_u64(b, 8 + 32 * 3 + 24, 6);
        }));
        out.extend(item(XSO_INPCB, 104, |b| {
            b[16..18].copy_from_slice(&443u16.to_be_bytes());
            b[18..20].copy_from_slice(&54321u16.to_be_bytes());
            if v6 {
                b[44] = INP_IPV6;
                b[48..64].copy_from_slice(&"2606:4700::1111".parse::<Ipv6Addr>().unwrap().octets());
                b[64..80].copy_from_slice(&Ipv6Addr::LOCALHOST.octets());
            } else {
                b[44] = INP_IPV4;
                b[60..64].copy_from_slice(&[1, 1, 1, 1]);
                b[76..80].copy_from_slice(&[192, 168, 1, 20]);
            }
        }));
        if tcp {
            out.extend(item(XSO_TCPCB, 204, |b| put_u32(b, 36, 4)));
        }
        out
    }

    fn wrap(body: Vec<u8>) -> Vec<u8> {
        let mut gen_header = vec![0u8; 24];
        gen_header[0..4].copy_from_slice(&24u32.to_ne_bytes());
        let mut out = gen_header.clone();
        out.extend(body);
        out.extend(gen_header);
        out
    }

    #[test]
    fn parses_tcp_v4_and_v6_records() {
        let mut body = pcb(true, false);
        body.extend(pcb(true, true));
        let records = parse(&wrap(body), true);
        assert_eq!(records.len(), 2);
        let v4 = &records[0];
        assert_eq!(v4.local_addr, "192.168.1.20".parse::<IpAddr>().unwrap());
        assert_eq!(v4.local_port, 54321);
        assert_eq!(v4.remote_addr, "1.1.1.1".parse::<IpAddr>().unwrap());
        assert_eq!(v4.remote_port, 443);
        assert_eq!(v4.tcp_state, Some(4));
        assert_eq!(v4.pid, Some(321));
        assert_eq!((v4.rx_bytes, v4.tx_bytes), (1024, 56));
        assert_eq!(
            records[1].remote_addr,
            "2606:4700::1111".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn parses_udp_without_tcpcb_and_stops_on_garbage() {
        let records = parse(&wrap(pcb(false, false)), false);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tcp_state, None);
        assert!(parse(&[1, 2, 3], true).is_empty());
        let mut truncated = wrap(pcb(true, false));
        truncated.truncate(200);
        assert!(parse(&truncated, true).is_empty());
    }
}
