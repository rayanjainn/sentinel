//! Well-known ports in plain words.
//!
//! Only ports whose meaning is standard are listed. `encrypted` is true where the protocol is
//! encrypted by definition, so the UI can say plainly that Sentinel cannot read the contents.

use crate::model::{Confidence, TransportProtocol};

pub(super) struct Service {
    /// Plain name: "secure web", "domain name lookup".
    pub name: &'static str,
    /// What it is for, when a port needs more than its name.
    pub purpose: Option<&'static str>,
    pub encrypted: bool,
    /// `Likely` for ports that are conventional rather than assigned.
    pub confidence: Confidence,
}

const fn known(name: &'static str, purpose: Option<&'static str>, encrypted: bool) -> Service {
    Service {
        name,
        purpose,
        encrypted,
        confidence: Confidence::Known,
    }
}

const fn likely(name: &'static str, purpose: Option<&'static str>, encrypted: bool) -> Service {
    Service {
        name,
        purpose,
        encrypted,
        confidence: Confidence::Likely,
    }
}

pub(super) fn lookup(port: u16, protocol: TransportProtocol) -> Option<Service> {
    if protocol == TransportProtocol::Udp {
        return match port {
            443 => Some(known(
                "secure web over QUIC",
                Some("web traffic using the newer QUIC protocol, which runs over UDP"),
                true,
            )),
            53 => Some(known(
                "domain name lookup",
                Some("turning a name like example.com into an address"),
                false,
            )),
            5353 => Some(known(
                "finding devices on your network",
                Some(
                    "Bonjour and mDNS: discovering printers, speakers, TVs and other computers nearby",
                ),
                false,
            )),
            123 => Some(known(
                "clock sync",
                Some("checking the time against a time server"),
                false,
            )),
            67 | 68 => Some(known(
                "network address setup",
                Some("DHCP: getting an address from your router"),
                false,
            )),
            546 | 547 => Some(known(
                "network address setup",
                Some("DHCPv6: getting an IPv6 address"),
                false,
            )),
            1900 => Some(known(
                "device discovery",
                Some("SSDP: finding media devices and routers on the local network"),
                false,
            )),
            3478 | 3479 | 19302..=19309 => Some(known(
                "voice or video call setup",
                Some("STUN: working out how two devices can reach each other directly"),
                false,
            )),
            5355 => Some(known(
                "local name lookup",
                Some("LLMNR: finding a computer by name on the local network"),
                false,
            )),
            137 | 138 => Some(known(
                "Windows name service",
                Some("NetBIOS: older Windows name and browsing service"),
                false,
            )),
            161 | 162 => Some(known(
                "device monitoring",
                Some("SNMP: reading status from network hardware"),
                false,
            )),
            514 => Some(known(
                "log forwarding",
                Some("syslog: sending log messages to another machine"),
                false,
            )),
            _ => None,
        };
    }
    match port {
        80 => Some(known(
            "web",
            Some("plain web traffic, which is not encrypted"),
            false,
        )),
        443 => Some(known(
            "secure web",
            Some("HTTPS: the usual way sites and apps talk"),
            true,
        )),
        8443 => Some(likely(
            "secure web",
            Some("HTTPS on an alternative port"),
            true,
        )),
        8080 | 8888 => Some(likely(
            "web",
            Some("a web server on an alternative port, often a local or development server"),
            false,
        )),
        53 => Some(known(
            "domain name lookup",
            Some("turning a name into an address"),
            false,
        )),
        853 => Some(known(
            "encrypted domain name lookup",
            Some("DNS over TLS, so the names being looked up are hidden"),
            true,
        )),
        22 => Some(known(
            "remote shell",
            Some("SSH: a command line or file transfer to another machine"),
            true,
        )),
        21 => Some(known(
            "file transfer",
            Some("FTP, which sends its login in the clear"),
            false,
        )),
        23 => Some(known(
            "telnet",
            Some("an old remote terminal protocol with no encryption"),
            false,
        )),
        25 => Some(known(
            "mail transfer",
            Some("SMTP between mail servers"),
            false,
        )),
        465 | 587 => Some(known(
            "sending mail",
            Some("submitting outgoing mail to a mail server"),
            true,
        )),
        143 => Some(known(
            "reading mail",
            Some("IMAP without encryption"),
            false,
        )),
        993 => Some(known("reading mail", Some("IMAP over TLS"), true)),
        110 => Some(known(
            "reading mail",
            Some("POP3 without encryption"),
            false,
        )),
        995 => Some(known("reading mail", Some("POP3 over TLS"), true)),
        3389 => Some(known(
            "remote desktop",
            Some("controlling another PC's screen"),
            true,
        )),
        5900..=5905 => Some(known(
            "screen sharing",
            Some("VNC: viewing or controlling another screen"),
            false,
        )),
        445 => Some(known(
            "file and printer sharing",
            Some("SMB: shared folders and printers"),
            false,
        )),
        139 => Some(known(
            "file sharing",
            Some("older NetBIOS-based Windows file sharing"),
            false,
        )),
        548 => Some(known(
            "file sharing",
            Some("AFP: Apple's older file sharing protocol"),
            false,
        )),
        2049 => Some(known(
            "file sharing",
            Some("NFS: Unix network file system"),
            false,
        )),
        631 => Some(known(
            "printing",
            Some("IPP: sending documents to a printer"),
            false,
        )),
        3283 => Some(known(
            "Apple Remote Desktop",
            Some("remote management of a Mac"),
            false,
        )),
        5223 => Some(known(
            "Apple push notifications",
            Some(
                "the connection Apple devices keep open so Messages, Mail and apps can be alerted",
            ),
            true,
        )),
        5228..=5230 => Some(known(
            "Google push notifications",
            Some("the connection Google services and Android apps use to receive alerts"),
            true,
        )),
        7000 => Some(likely(
            "AirPlay",
            Some("receiving AirPlay video or screen mirroring on a Mac"),
            false,
        )),
        5000 => Some(likely(
            "AirPlay or a local server",
            Some(
                "macOS uses this port for AirPlay Receiver; it is also a common port for local development servers",
            ),
            false,
        )),
        3000 | 4200 | 5173 | 8000 => Some(likely(
            "local development server",
            Some("a port commonly used by development servers while you work on a project"),
            false,
        )),
        5432 => Some(known(
            "PostgreSQL database",
            Some("an app talking to a PostgreSQL database"),
            false,
        )),
        3306 => Some(known(
            "MySQL or MariaDB database",
            Some("an app talking to a MySQL-compatible database"),
            false,
        )),
        1433 => Some(known(
            "SQL Server database",
            Some("an app talking to Microsoft SQL Server"),
            false,
        )),
        6379 => Some(known(
            "Redis",
            Some("an app using a Redis cache or queue"),
            false,
        )),
        27017 => Some(known(
            "MongoDB",
            Some("an app talking to a MongoDB database"),
            false,
        )),
        9200 | 9300 => Some(known(
            "Elasticsearch",
            Some("an app talking to an Elasticsearch server"),
            false,
        )),
        11434 => Some(known(
            "local AI model server",
            Some("Ollama's default port: an app asking a model running on this machine"),
            false,
        )),
        9418 => Some(known("Git", Some("the unencrypted Git protocol"), false)),
        6000..=6010 => Some(likely(
            "X11 display",
            Some("a program drawing on an X11 desktop"),
            false,
        )),
        389 => Some(known(
            "directory service",
            Some("LDAP: looking up users and groups"),
            false,
        )),
        636 => Some(known("directory service", Some("LDAP over TLS"), true)),
        88 => Some(known(
            "Kerberos sign-in",
            Some("getting a sign-in ticket on a managed network"),
            false,
        )),
        111 => Some(known(
            "RPC directory",
            Some("rpcbind: finding Unix network services"),
            false,
        )),
        1935 => Some(known(
            "live streaming",
            Some("RTMP: sending or receiving a live video stream"),
            false,
        )),
        _ => None,
    }
}
