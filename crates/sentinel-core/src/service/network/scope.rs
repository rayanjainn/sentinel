use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::model::{AddrScope, IpFamily};

pub fn family(addr: &IpAddr) -> IpFamily {
    match addr {
        IpAddr::V4(_) => IpFamily::V4,
        IpAddr::V6(_) => IpFamily::V6,
    }
}

pub fn classify(addr: &IpAddr) -> AddrScope {
    match addr {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(addr: &Ipv4Addr) -> AddrScope {
    let [a, b, c, _] = addr.octets();
    if addr.is_unspecified() {
        AddrScope::Unspecified
    } else if addr.is_loopback() {
        AddrScope::Loopback
    } else if addr.is_link_local() {
        AddrScope::LinkLocal
    } else if addr.is_multicast() || addr.is_broadcast() {
        AddrScope::Multicast
    } else if addr.is_private()
        // Carrier-grade NAT, "this network", IETF protocol assignments, documentation,
        // benchmarking and reserved ranges are not routable on the public internet.
        || (a == 100 && (64..128).contains(&b))
        || a == 0
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (18..20).contains(&b))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 240
    {
        AddrScope::Private
    } else {
        AddrScope::Public
    }
}

fn classify_v6(addr: &Ipv6Addr) -> AddrScope {
    if let Some(v4) = addr.to_ipv4_mapped() {
        return classify_v4(&v4);
    }
    let segments = addr.segments();
    if addr.is_unspecified() {
        AddrScope::Unspecified
    } else if addr.is_loopback() {
        AddrScope::Loopback
    } else if addr.is_multicast() {
        AddrScope::Multicast
    } else if segments[0] & 0xffc0 == 0xfe80 {
        AddrScope::LinkLocal
    } else if segments[0] & 0xfe00 == 0xfc00
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || segments[0] & 0xffc0 == 0xfec0
    {
        AddrScope::Private
    } else if segments[0] == 0x0064 && segments[1] == 0xff9b {
        // NAT64: the embedded IPv4 address decides.
        let octets = addr.octets();
        classify_v4(&Ipv4Addr::new(
            octets[12], octets[13], octets[14], octets[15],
        ))
    } else if segments[0] & 0xe000 == 0x2000 {
        AddrScope::Public
    } else {
        AddrScope::Private
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(s: &str) -> AddrScope {
        classify(&s.parse().unwrap())
    }

    #[test]
    fn classifies_ipv4() {
        assert_eq!(scope("127.0.0.1"), AddrScope::Loopback);
        assert_eq!(scope("0.0.0.0"), AddrScope::Unspecified);
        assert_eq!(scope("192.168.29.1"), AddrScope::Private);
        assert_eq!(scope("100.72.0.1"), AddrScope::Private);
        assert_eq!(scope("169.254.10.1"), AddrScope::LinkLocal);
        assert_eq!(scope("224.0.0.251"), AddrScope::Multicast);
        assert_eq!(scope("17.242.13.5"), AddrScope::Public);
        assert_eq!(scope("203.0.113.9"), AddrScope::Private);
    }

    #[test]
    fn classifies_ipv6() {
        assert_eq!(scope("::1"), AddrScope::Loopback);
        assert_eq!(scope("::"), AddrScope::Unspecified);
        assert_eq!(scope("fe80::1c2:3"), AddrScope::LinkLocal);
        assert_eq!(scope("fd12:3456::1"), AddrScope::Private);
        assert_eq!(scope("ff02::fb"), AddrScope::Multicast);
        assert_eq!(scope("2606:4700::1111"), AddrScope::Public);
        assert_eq!(scope("::ffff:10.0.0.1"), AddrScope::Private);
        assert_eq!(scope("::ffff:8.8.8.8"), AddrScope::Public);
        assert_eq!(scope("64:ff9b::808:808"), AddrScope::Public);
        assert_eq!(scope("2001:db8::1"), AddrScope::Private);
    }
}
