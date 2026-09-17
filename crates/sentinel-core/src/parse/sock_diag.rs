//! Linux `NETLINK_SOCK_DIAG` replies: `inet_diag_msg` records with an `INET_DIAG_INFO` attribute
//! carrying `struct tcp_info`, whose `tcpi_bytes_acked` / `tcpi_bytes_received` give per-connection
//! byte counters without privileges.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const NLMSG_ERROR: u16 = 2;
pub const NLMSG_DONE: u16 = 3;
pub const SOCK_DIAG_BY_FAMILY: u16 = 20;
pub const INET_DIAG_INFO: u16 = 2;
const NLMSG_HDR: usize = 16;
const INET_DIAG_MSG: usize = 72;
const TCP_INFO_BYTES_ACKED: usize = 120;
const TCP_INFO_BYTES_RECEIVED: usize = 128;
const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagSocket {
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
    pub inode: u32,
    pub bytes_out: u64,
    pub bytes_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Batch {
    /// More datagrams follow.
    More(Vec<DiagSocket>),
    Done(Vec<DiagSocket>),
    /// Kernel error (negative errno).
    Error(i32),
}

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

fn u16_ne(buf: &[u8], at: usize) -> Option<u16> {
    buf.get(at..at + 2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
}

fn u32_ne(buf: &[u8], at: usize) -> Option<u32> {
    buf.get(at..at + 4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_ne(buf: &[u8], at: usize) -> Option<u64> {
    buf.get(at..at + 8).map(|b| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(b);
        u64::from_ne_bytes(bytes)
    })
}

fn address(family: u8, raw: &[u8]) -> Option<IpAddr> {
    match family {
        AF_INET => Some(IpAddr::V4(Ipv4Addr::new(raw[0], raw[1], raw[2], raw[3]))),
        AF_INET6 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(raw.get(..16)?);
            let v6 = Ipv6Addr::from(octets);
            Some(
                v6.to_ipv4_mapped()
                    .map(IpAddr::V4)
                    .unwrap_or(IpAddr::V6(v6)),
            )
        }
        _ => None,
    }
}

/// `struct inet_diag_msg` followed by route attributes.
fn parse_record(payload: &[u8]) -> Option<DiagSocket> {
    if payload.len() < INET_DIAG_MSG {
        return None;
    }
    let family = payload[0];
    // inet_diag_sockid starts at 4: sport(be16) dport(be16) src[16] dst[16] if(u32) cookie[8].
    let sport = u16::from_be_bytes([payload[4], payload[5]]);
    let dport = u16::from_be_bytes([payload[6], payload[7]]);
    let local = address(family, &payload[8..24])?;
    let remote = address(family, &payload[24..40])?;
    let inode = u32_ne(payload, 68)?;
    let mut offset = INET_DIAG_MSG;
    let mut counters = None;
    while offset + 4 <= payload.len() {
        let len = u16_ne(payload, offset)? as usize;
        let kind = u16_ne(payload, offset + 2)?;
        if len < 4 || offset + len > payload.len() {
            break;
        }
        if kind == INET_DIAG_INFO {
            let info = &payload[offset + 4..offset + len];
            if let (Some(acked), Some(received)) = (
                u64_ne(info, TCP_INFO_BYTES_ACKED),
                u64_ne(info, TCP_INFO_BYTES_RECEIVED),
            ) {
                counters = Some((acked, received));
            }
        }
        offset += align4(len);
    }
    let (bytes_out, bytes_in) = counters?;
    Some(DiagSocket {
        local_addr: local,
        local_port: sport,
        remote_addr: remote,
        remote_port: dport,
        inode,
        bytes_out,
        bytes_in,
    })
}

/// Parses one `recv` datagram of a dump.
pub fn parse_datagram(buf: &[u8]) -> Batch {
    let mut sockets = Vec::new();
    let mut offset = 0;
    while offset + NLMSG_HDR <= buf.len() {
        let Some(len) = u32_ne(buf, offset).map(|l| l as usize) else {
            break;
        };
        let kind = u16_ne(buf, offset + 4).unwrap_or(0);
        if len < NLMSG_HDR || offset + len > buf.len() {
            break;
        }
        let payload = &buf[offset + NLMSG_HDR..offset + len];
        match kind {
            NLMSG_DONE => return Batch::Done(sockets),
            NLMSG_ERROR => {
                let errno = payload
                    .get(..4)
                    .map(|b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
                    .unwrap_or(-1);
                return Batch::Error(errno);
            }
            SOCK_DIAG_BY_FAMILY => sockets.extend(parse_record(payload)),
            _ => {}
        }
        offset += align4(len);
    }
    Batch::More(sockets)
}

/// `nlmsghdr` + `inet_diag_req_v2` dumping every TCP socket of `family` with `tcp_info`.
pub fn dump_request(family: u8, sequence: u32) -> Vec<u8> {
    const NLM_F_REQUEST: u16 = 0x1;
    const NLM_F_DUMP: u16 = 0x300;
    const IPPROTO_TCP: u8 = 6;
    let body_len = 56;
    let total = NLMSG_HDR + body_len;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&(total as u32).to_ne_bytes());
    out.extend_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    out.extend_from_slice(&(NLM_F_REQUEST | NLM_F_DUMP).to_ne_bytes());
    out.extend_from_slice(&sequence.to_ne_bytes());
    out.extend_from_slice(&0u32.to_ne_bytes());
    // inet_diag_req_v2: family, protocol, ext bitmask, pad, states, then a zeroed sockid.
    out.push(family);
    out.push(IPPROTO_TCP);
    out.push(1 << (INET_DIAG_INFO - 1));
    out.push(0);
    out.extend_from_slice(&u32::MAX.to_ne_bytes());
    out.resize(total, 0);
    out
}

pub fn af_inet() -> u8 {
    AF_INET
}

pub fn af_inet6() -> u8 {
    AF_INET6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(family: u8, local: &[u8], remote: &[u8], acked: u64, received: u64) -> Vec<u8> {
        let mut msg = vec![0u8; INET_DIAG_MSG];
        msg[0] = family;
        msg[4..6].copy_from_slice(&50000u16.to_be_bytes());
        msg[6..8].copy_from_slice(&443u16.to_be_bytes());
        msg[8..8 + local.len()].copy_from_slice(local);
        msg[24..24 + remote.len()].copy_from_slice(remote);
        msg[68..72].copy_from_slice(&4242u32.to_ne_bytes());
        let mut info = vec![0u8; 232];
        info[TCP_INFO_BYTES_ACKED..TCP_INFO_BYTES_ACKED + 8].copy_from_slice(&acked.to_ne_bytes());
        info[TCP_INFO_BYTES_RECEIVED..TCP_INFO_BYTES_RECEIVED + 8]
            .copy_from_slice(&received.to_ne_bytes());
        let attr_len = 4 + info.len();
        msg.extend_from_slice(&(attr_len as u16).to_ne_bytes());
        msg.extend_from_slice(&INET_DIAG_INFO.to_ne_bytes());
        msg.extend_from_slice(&info);
        msg.resize(align4(msg.len()), 0);
        let mut out = Vec::new();
        out.extend_from_slice(&((NLMSG_HDR + msg.len()) as u32).to_ne_bytes());
        out.extend_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
        out.extend_from_slice(&[0u8; 10]);
        out.extend_from_slice(&msg);
        out
    }

    #[test]
    fn parses_tcp_info_counters() {
        let mut datagram = record(AF_INET, &[192, 168, 1, 5], &[1, 1, 1, 1], 900, 12_000);
        let mapped: Vec<u8> = Ipv4Addr::new(10, 0, 0, 7)
            .to_ipv6_mapped()
            .octets()
            .to_vec();
        datagram.extend(record(
            AF_INET6,
            &Ipv6Addr::LOCALHOST.octets(),
            &mapped,
            1,
            2,
        ));
        let Batch::More(sockets) = parse_datagram(&datagram) else {
            panic!("expected more");
        };
        assert_eq!(sockets.len(), 2);
        assert_eq!(
            sockets[0].local_addr,
            "192.168.1.5".parse::<IpAddr>().unwrap()
        );
        assert_eq!(sockets[0].remote_port, 443);
        assert_eq!((sockets[0].bytes_out, sockets[0].bytes_in), (900, 12_000));
        assert_eq!(sockets[0].inode, 4242);
        assert_eq!(
            sockets[1].remote_addr,
            "10.0.0.7".parse::<IpAddr>().unwrap()
        );

        let mut done = vec![0u8; NLMSG_HDR + 4];
        done[0..4].copy_from_slice(&((NLMSG_HDR + 4) as u32).to_ne_bytes());
        done[4..6].copy_from_slice(&NLMSG_DONE.to_ne_bytes());
        assert_eq!(parse_datagram(&done), Batch::Done(vec![]));
    }

    #[test]
    fn builds_dump_request() {
        let request = dump_request(AF_INET6, 7);
        assert_eq!(request.len(), 72);
        assert_eq!(u16_ne(&request, 4), Some(SOCK_DIAG_BY_FAMILY));
        assert_eq!(request[16], AF_INET6);
        assert_eq!(request[17], 6);
        assert_eq!(request[18], 0b10);
        assert_eq!(u32_ne(&request, 20), Some(u32::MAX));
    }
}
