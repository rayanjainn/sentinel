//! Socket table formats: Linux `/proc/net/{tcp,tcp6,udp,udp6}` and Windows IP Helper rows.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::model::TcpState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcNetRow {
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
    pub state: u8,
    pub inode: u64,
}

/// Addresses are the kernel's in-memory words printed in host byte order.
fn hex_addr(text: &str) -> Option<IpAddr> {
    match text.len() {
        8 => {
            let word = u32::from_str_radix(text, 16).ok()?;
            Some(IpAddr::V4(Ipv4Addr::from(word.to_ne_bytes())))
        }
        32 => {
            let mut octets = [0u8; 16];
            for (index, chunk) in octets.chunks_mut(4).enumerate() {
                let word = u32::from_str_radix(text.get(index * 8..index * 8 + 8)?, 16).ok()?;
                chunk.copy_from_slice(&word.to_ne_bytes());
            }
            Some(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        _ => None,
    }
}

fn endpoint(text: &str) -> Option<(IpAddr, u16)> {
    let (addr, port) = text.split_once(':')?;
    Some((hex_addr(addr)?, u16::from_str_radix(port, 16).ok()?))
}

pub fn proc_net(text: &str) -> Vec<ProcNetRow> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (local_addr, local_port) = endpoint(fields.get(1)?)?;
            let (remote_addr, remote_port) = endpoint(fields.get(2)?)?;
            let state = u8::from_str_radix(fields.get(3)?, 16).ok()?;
            let inode = fields.get(9)?.parse().ok()?;
            Some(ProcNetRow {
                local_addr,
                local_port,
                remote_addr,
                remote_port,
                state,
                inode,
            })
        })
        .collect()
}

/// `tcp_state` numbering used by `/proc/net/tcp` (`TCP_ESTABLISHED` = 1 …).
pub fn linux_tcp_state(state: u8) -> TcpState {
    match state {
        1 => TcpState::Established,
        2 => TcpState::SynSent,
        3 => TcpState::SynReceived,
        4 => TcpState::FinWait1,
        5 => TcpState::FinWait2,
        6 => TcpState::TimeWait,
        7 => TcpState::Closed,
        8 => TcpState::CloseWait,
        9 => TcpState::LastAck,
        10 => TcpState::Listen,
        11 => TcpState::Closing,
        _ => TcpState::Unknown,
    }
}

/// `MIB_TCP_STATE`.
pub fn windows_tcp_state(state: u32) -> TcpState {
    match state {
        1 => TcpState::Closed,
        2 => TcpState::Listen,
        3 => TcpState::SynSent,
        4 => TcpState::SynReceived,
        5 => TcpState::Established,
        6 => TcpState::FinWait1,
        7 => TcpState::FinWait2,
        8 => TcpState::CloseWait,
        9 => TcpState::Closing,
        10 => TcpState::LastAck,
        11 => TcpState::TimeWait,
        12 => TcpState::DeleteTcb,
        _ => TcpState::Unknown,
    }
}

/// IP Helper stores ports in network byte order in the low 16 bits of a DWORD.
pub fn windows_port(raw: u32) -> u16 {
    u16::from_be((raw & 0xffff) as u16)
}

/// IP Helper stores IPv4 addresses as network-order bytes inside a DWORD.
pub fn windows_ipv4(raw: u32) -> Ipv4Addr {
    Ipv4Addr::from(raw.to_ne_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_endian = "little")]
    fn parses_proc_net_tcp() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 34567\n\
   1: 1D00A8C0:C350 22D8B85D:01BB 01 00000000:00000000 02:000001F4 00000000  1000        0 34990\n";
        let rows = proc_net(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].local_addr, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].local_port, 8080);
        assert_eq!(linux_tcp_state(rows[0].state), TcpState::Listen);
        assert_eq!(rows[0].inode, 34567);
        assert_eq!(
            rows[1].local_addr,
            "192.168.0.29".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            rows[1].remote_addr,
            "93.184.216.34".parse::<IpAddr>().unwrap()
        );
        assert_eq!(rows[1].remote_port, 443);
    }

    #[test]
    #[cfg(target_endian = "little")]
    fn parses_proc_net_tcp6_and_udp() {
        let text = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops\n\
 1234: 00000000000000000000000001000000:0035 00000000000000000000000000000000:0000 07 00000000:00000000 00:00000000 00000000   101        0 20001 2 0000000000000000 0\n";
        let rows = proc_net(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].local_addr, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].local_port, 53);
        assert_eq!(rows[0].inode, 20001);
    }

    #[test]
    #[cfg(target_endian = "little")]
    fn decodes_windows_rows() {
        assert_eq!(windows_port(0x0000_901F), 8080);
        assert_eq!(windows_ipv4(0x0100_007F), Ipv4Addr::LOCALHOST);
        assert_eq!(windows_tcp_state(5), TcpState::Established);
        assert_eq!(windows_tcp_state(2), TcpState::Listen);
    }
}
