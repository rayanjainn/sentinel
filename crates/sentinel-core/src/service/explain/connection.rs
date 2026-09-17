//! Connection classifier: port table + bundled host catalog + address scope + owning process.
//!
//! The OS attributes a socket to a process, never to a browser tab or a URL. When the owning
//! process is a browser renderer or extension, this says so in plain words instead of inventing a
//! tab. HTTPS and other encrypted ports always carry an explicit sentence that the contents cannot
//! be read — Sentinel does not capture packets and could not read them if it did.

use std::net::IpAddr;

use super::hosts::{self, Endpoint};
use super::ports::{self, Service};
use crate::model::{AddrScope, Confidence, ConnectionExplanation, ProcessRole, TransportProtocol};

pub struct ConnectionFacts<'a> {
    pub protocol: TransportProtocol,
    /// `None` for a listening socket.
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
    /// Reverse-DNS name, when one has resolved.
    pub remote_host: Option<&'a str>,
    pub remote_scope: Option<AddrScope>,
    pub local_port: u16,
    /// Role of the owning process, so a browser renderer or extension gets the "one browser tab
    /// or site" wording rather than a fabricated tab.
    pub owner_role: Option<ProcessRole>,
}

pub fn explain_connection(facts: &ConnectionFacts<'_>) -> ConnectionExplanation {
    let port = facts
        .remote_port
        .and_then(|port| ports::lookup(port, facts.protocol));

    let Some(addr) = facts.remote_addr else {
        return listening(facts, port);
    };

    let scope = facts.remote_scope.unwrap_or(AddrScope::Public);
    match scope {
        AddrScope::Loopback => loopback(facts, port),
        AddrScope::Private | AddrScope::LinkLocal => local_network(facts, port, scope),
        AddrScope::Multicast => broadcast(facts, port),
        AddrScope::Unspecified => unspecified(facts, port),
        AddrScope::Public => public(facts, addr, port),
    }
}

fn rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::Known => 2,
        Confidence::Likely => 1,
        Confidence::Unknown => 0,
    }
}

/// The weaker of two confidences: an explanation is only as sure as its least certain part.
fn combine(a: Confidence, b: Confidence) -> Confidence {
    if rank(a) <= rank(b) { a } else { b }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn encryption_sentence(encrypted: bool) -> &'static str {
    if encrypted {
        "The connection is encrypted, so Sentinel cannot read what is inside it — and does not \
         capture any packets to try."
    } else {
        "This connection is not encrypted."
    }
}

fn browser_tab_sentence(role: Option<ProcessRole>) -> Option<&'static str> {
    match role {
        Some(ProcessRole::Renderer) => Some(
            "One browser tab or site is using this connection. The browser does not tell the \
             operating system which tab, so Sentinel cannot say either.",
        ),
        Some(ProcessRole::Extension) => Some(
            "One browser extension is using this connection; the operating system does not say \
             which one.",
        ),
        _ => None,
    }
}

fn port_purpose_sentence(port: &Service, port_number: u16, protocol: TransportProtocol) -> String {
    let proto = match protocol {
        TransportProtocol::Tcp => "TCP",
        TransportProtocol::Udp => "UDP",
    };
    match port.purpose {
        Some(purpose) => format!("Port {port_number} ({proto}) is normally used for {purpose}."),
        None => format!(
            "Port {port_number} ({proto}) is normally used for {}.",
            port.name
        ),
    }
}

fn listening(facts: &ConnectionFacts<'_>, port: Option<Service>) -> ConnectionExplanation {
    let mut detail = String::from(
        "Nothing is connected right now — this only means the program is ready to accept a \
         connection on this port.",
    );
    let headline = match &port {
        Some(p) => {
            detail.push(' ');
            detail.push_str(&port_purpose_sentence(p, facts.local_port, facts.protocol));
            capitalize(p.name)
        }
        None => {
            detail.push_str(
                " Sentinel does not recognise this port, so it cannot say what the service \
                 normally does.",
            );
            format!("Listening on port {}", facts.local_port)
        }
    };
    let encrypted = port.as_ref().is_some_and(|p| p.encrypted);
    if encrypted {
        detail.push(' ');
        detail.push_str(encryption_sentence(true));
    }
    ConnectionExplanation {
        headline,
        detail,
        service: port.as_ref().map(|p| p.name.to_owned()),
        purpose: port.as_ref().and_then(|p| p.purpose).map(str::to_owned),
        confidence: port.as_ref().map_or(Confidence::Unknown, |p| p.confidence),
        encrypted,
    }
}

fn loopback(facts: &ConnectionFacts<'_>, port: Option<Service>) -> ConnectionExplanation {
    let mut detail = String::from(
        "Stays on this computer — one program talking to another. Nothing leaves the machine.",
    );
    let headline = match &port {
        Some(p) => {
            detail.push(' ');
            let proto = proto_label(facts.protocol);
            detail.push_str(&format!(
                "Port {} ({proto}) is normally used for {}.",
                facts.remote_port.unwrap_or_default(),
                p.purpose.unwrap_or(p.name)
            ));
            capitalize(p.name)
        }
        None => "Local connection".to_owned(),
    };
    detail.push(' ');
    detail.push_str(encryption_sentence(
        port.as_ref().is_some_and(|p| p.encrypted),
    ));
    if let Some(sentence) = browser_tab_sentence(facts.owner_role) {
        detail.push(' ');
        detail.push_str(sentence);
    }
    ConnectionExplanation {
        headline,
        detail,
        service: port.as_ref().map(|p| p.name.to_owned()),
        purpose: port.as_ref().and_then(|p| p.purpose).map(str::to_owned),
        confidence: port.as_ref().map_or(Confidence::Likely, |p| p.confidence),
        encrypted: port.as_ref().is_some_and(|p| p.encrypted),
    }
}

fn local_network(
    facts: &ConnectionFacts<'_>,
    port: Option<Service>,
    scope: AddrScope,
) -> ConnectionExplanation {
    let network = if scope == AddrScope::LinkLocal {
        "found automatically on your local network"
    } else {
        "on your Wi-Fi or local network"
    };
    let mut detail = format!(
        "Talks to another device {network} — not the internet. Sentinel does not know which \
         device beyond its address."
    );
    let headline = match &port {
        Some(p) => {
            detail.push(' ');
            detail.push_str(&port_purpose_sentence(
                p,
                facts.remote_port.unwrap_or_default(),
                facts.protocol,
            ));
            format!("{} on your network", capitalize(p.name))
        }
        None => "Device on your local network".to_owned(),
    };
    let encrypted = port.as_ref().is_some_and(|p| p.encrypted);
    detail.push(' ');
    detail.push_str(encryption_sentence(encrypted));
    if let Some(sentence) = browser_tab_sentence(facts.owner_role) {
        detail.push(' ');
        detail.push_str(sentence);
    }
    ConnectionExplanation {
        headline,
        detail,
        service: port.as_ref().map(|p| p.name.to_owned()),
        purpose: port.as_ref().and_then(|p| p.purpose).map(str::to_owned),
        confidence: port.as_ref().map_or(Confidence::Likely, |p| p.confidence),
        encrypted,
    }
}

fn broadcast(_facts: &ConnectionFacts<'_>, port: Option<Service>) -> ConnectionExplanation {
    ConnectionExplanation {
        headline: "Sent to several devices at once".to_owned(),
        detail: "A multicast address: this is sent to every listening device on the local network \
                  at once, rather than to one destination. It never reaches the internet."
            .to_owned(),
        service: port.as_ref().map(|p| p.name.to_owned()),
        purpose: port.as_ref().and_then(|p| p.purpose).map(str::to_owned),
        confidence: Confidence::Known,
        encrypted: false,
    }
}

fn unspecified(_facts: &ConnectionFacts<'_>, _port: Option<Service>) -> ConnectionExplanation {
    ConnectionExplanation {
        headline: "No destination yet".to_owned(),
        detail: "This socket has not connected anywhere yet, so there is nothing to explain about \
                  the other end."
            .to_owned(),
        service: None,
        purpose: None,
        confidence: Confidence::Known,
        encrypted: false,
    }
}

fn public(
    facts: &ConnectionFacts<'_>,
    addr: IpAddr,
    port: Option<Service>,
) -> ConnectionExplanation {
    let endpoint: Option<&'static Endpoint> = facts
        .remote_host
        .and_then(hosts::by_hostname)
        .or_else(|| hosts::by_address(addr));

    let host_label = facts
        .remote_host
        .map(str::to_owned)
        .unwrap_or_else(|| addr.to_string());

    let (headline, mut detail, mut confidence) = match (&endpoint, &port) {
        (Some(endpoint), _) => (
            endpoint.owner.to_owned(),
            format!("This is {}.", endpoint.purpose),
            Confidence::Known,
        ),
        (None, Some(p)) => (
            capitalize(p.name),
            format!(
                "Sentinel does not recognise {host_label}, but port {} is normally used for {}.",
                facts.remote_port.unwrap_or_default(),
                p.purpose.unwrap_or(p.name)
            ),
            combine(Confidence::Likely, p.confidence),
        ),
        (None, None) => (
            host_label.clone(),
            format!("Sentinel doesn't recognise {host_label}."),
            Confidence::Unknown,
        ),
    };

    if let Some(endpoint) = endpoint {
        if endpoint.telemetry {
            detail.push_str(" This looks like usage, diagnostics or advertising data collection.");
        }
        if let Some(p) = &port {
            detail.push(' ');
            detail.push_str(&port_purpose_sentence(
                p,
                facts.remote_port.unwrap_or_default(),
                facts.protocol,
            ));
            confidence = combine(confidence, p.confidence);
        }
    }

    let encrypted = port.as_ref().is_some_and(|p| p.encrypted);
    detail.push(' ');
    detail.push_str(encryption_sentence(encrypted));
    if let Some(sentence) = browser_tab_sentence(facts.owner_role) {
        detail.push(' ');
        detail.push_str(sentence);
    }

    ConnectionExplanation {
        headline,
        detail,
        service: port.as_ref().map(|p| p.name.to_owned()),
        purpose: endpoint
            .map(|e| e.purpose.to_owned())
            .or_else(|| port.as_ref().and_then(|p| p.purpose).map(str::to_owned)),
        confidence,
        encrypted,
    }
}

fn proto_label(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::Tcp => "TCP",
        TransportProtocol::Udp => "UDP",
    }
}
