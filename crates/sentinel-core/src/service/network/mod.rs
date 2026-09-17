//! OS-agnostic socket enrichment: owning process names, address scope, cached reverse DNS, local
//! geolocation, per-connection byte counters and rates.

pub mod dns;
#[cfg(feature = "native")]
pub mod download;
pub mod geo;
pub mod home;
pub mod scope;

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use crate::error::CoreResult;
use crate::model::{
    AddrScope, NetworkSnapshot, Pid, ProcessRole, RawSocket, SocketEntry, TimestampSecs,
    TrafficSource, TransportProtocol,
};
use crate::provider::NetworkProvider;
use crate::service::explain::{ConnectionFacts, explain_connection};
use crate::util::{RateCounter, now_ms};

use self::dns::ReverseDns;
use self::geo::GeoDb;

/// Owner name, start time and role of the process behind a socket, as much as the caller could
/// resolve at sample time. `start_time`/`role` are `None` when only a name-only fallback scan ran
/// (the process snapshot stream was not subscribed); callers must not treat that as "not a
/// browser" — it means the role is simply unknown.
#[derive(Debug, Clone)]
pub struct ProcessOwner {
    pub name: String,
    pub app_name: Option<String>,
    pub start_time: Option<TimestampSecs>,
    pub role: Option<ProcessRole>,
}

type ConnKey = (TransportProtocol, IpAddr, u16, Option<IpAddr>, Option<u16>);

struct Tracked {
    first_seen_ms: u64,
    rate: RateCounter,
    seen_at: Instant,
}

pub struct NetworkMonitor {
    provider: Box<dyn NetworkProvider>,
    throughput: RateCounter,
    tracked: HashMap<String, Tracked>,
    dns: Arc<ReverseDns>,
    geo: Arc<GeoDb>,
}

pub fn socket_id(socket: &RawSocket) -> String {
    let proto = match socket.protocol {
        TransportProtocol::Tcp => "tcp",
        TransportProtocol::Udp => "udp",
    };
    let remote = match (socket.remote_addr, socket.remote_port) {
        (Some(addr), Some(port)) => format!("{addr}:{port}"),
        _ => "*".to_owned(),
    };
    let pid = socket
        .pid
        .map(|p| p.to_string())
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "{proto}|{}:{}|{remote}|{pid}",
        socket.local_addr, socket.local_port
    )
}

fn key(
    protocol: TransportProtocol,
    local: IpAddr,
    lport: u16,
    remote: Option<IpAddr>,
    rport: Option<u16>,
) -> ConnKey {
    (protocol, local, lport, remote, rport)
}

/// Unbound UDP sockets (`*:*`) carry no information for the user.
fn is_noise(socket: &RawSocket) -> bool {
    socket.protocol == TransportProtocol::Udp
        && socket.local_port == 0
        && socket.local_addr.is_unspecified()
        && socket.remote_addr.is_none()
}

impl NetworkMonitor {
    pub fn new(provider: Box<dyn NetworkProvider>, dns: Arc<ReverseDns>, geo: Arc<GeoDb>) -> Self {
        Self {
            provider,
            throughput: RateCounter::default(),
            tracked: HashMap::new(),
            dns,
            geo,
        }
    }

    /// Raw rows, for callers that only need ownership (e.g. `find_port_owner`).
    pub fn raw_sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
        self.provider.sockets()
    }

    /// `owners` maps the owning PIDs present in this sample to what is known about them.
    pub fn sample(
        &mut self,
        owners: &mut dyn FnMut(&[Pid]) -> HashMap<Pid, ProcessOwner>,
    ) -> CoreResult<NetworkSnapshot> {
        let now = Instant::now();
        let ts_ms = now_ms();
        let raw: Vec<RawSocket> = self
            .provider
            .sockets()?
            .into_iter()
            .filter(|s| !is_noise(s))
            .collect();
        let traffic = self.provider.traffic().ok();
        let throughput = match self.provider.interface_counters() {
            Ok((rx, tx)) => self.throughput.update(rx, tx),
            Err(_) => self.throughput.last(),
        };
        let traffic_source = traffic
            .as_ref()
            .map(|t| t.source)
            .unwrap_or(TrafficSource::InterfaceOnly);
        let per_connection: HashMap<ConnKey, (u64, u64)> = match &traffic {
            Some(report) if report.source == TrafficSource::PerConnection => report
                .connections
                .iter()
                .map(|c| {
                    (
                        key(
                            c.protocol,
                            c.local_addr,
                            c.local_port,
                            c.remote_addr,
                            c.remote_port,
                        ),
                        (c.bytes_in, c.bytes_out),
                    )
                })
                .collect(),
            _ => HashMap::new(),
        };

        let pids: Vec<Pid> = raw
            .iter()
            .filter_map(|s| s.pid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let owners = owners(&pids);

        let mut seen_ids: HashMap<String, u32> = HashMap::new();
        let mut sockets = Vec::with_capacity(raw.len());
        for socket in raw {
            let mut id = socket_id(&socket);
            let duplicates = seen_ids.entry(id.clone()).or_insert(0);
            *duplicates += 1;
            if *duplicates > 1 {
                id = format!("{id}#{duplicates}");
            }
            let remote_scope = socket.remote_addr.as_ref().map(scope::classify);
            let remote_host = match (socket.remote_addr, remote_scope) {
                (Some(addr), Some(scope))
                    if !matches!(
                        scope,
                        AddrScope::Loopback | AddrScope::Unspecified | AddrScope::Multicast
                    ) =>
                {
                    self.dns.lookup(addr)
                }
                _ => None,
            };
            let geo = match (socket.remote_addr, remote_scope) {
                (Some(addr), Some(AddrScope::Public)) => self.geo.lookup(addr),
                _ => None,
            };
            let tracked = self.tracked.entry(id.clone()).or_insert_with(|| Tracked {
                first_seen_ms: ts_ms,
                rate: RateCounter::default(),
                seen_at: now,
            });
            tracked.seen_at = now;
            let counters = per_connection
                .get(&key(
                    socket.protocol,
                    socket.local_addr,
                    socket.local_port,
                    socket.remote_addr,
                    socket.remote_port,
                ))
                .copied();
            let (bytes_in, bytes_out, rx_bps, tx_bps) = match counters {
                Some((rx, tx)) => {
                    let rate = tracked.rate.update_at(now, rx, tx);
                    (Some(rx), Some(tx), Some(rate.rx_bps), Some(rate.tx_bps))
                }
                None => (None, None, None, None),
            };
            let owner = socket.pid.and_then(|pid| owners.get(&pid));
            let remote_host_str = remote_host.as_deref();
            let explanation = explain_connection(&ConnectionFacts {
                protocol: socket.protocol,
                remote_addr: socket.remote_addr,
                remote_port: socket.remote_port,
                remote_host: remote_host_str,
                remote_scope,
                local_port: socket.local_port,
                owner_role: owner.and_then(|o| o.role),
            });
            sockets.push(SocketEntry {
                id,
                protocol: socket.protocol,
                family: scope::family(&socket.local_addr),
                local_addr: socket.local_addr.to_string(),
                local_port: socket.local_port,
                remote_addr: socket.remote_addr.map(|a| a.to_string()),
                remote_port: socket.remote_port,
                remote_scope,
                state: socket.state,
                pid: socket.pid,
                process_name: owner.map(|o| o.name.clone()),
                app_name: owner.and_then(|o| o.app_name.clone()),
                process_start_time: owner.and_then(|o| o.start_time),
                remote_host,
                geo,
                bytes_in,
                bytes_out,
                rx_bps,
                tx_bps,
                first_seen_ms: tracked.first_seen_ms,
                explanation,
            });
        }
        self.tracked.retain(|_, t| t.seen_at == now);
        sockets.sort_by(|a, b| a.pid.cmp(&b.pid).then_with(|| a.id.cmp(&b.id)));
        Ok(NetworkSnapshot {
            ts_ms,
            sockets,
            throughput,
            traffic_source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{CoreEvent, EventSink};
    use crate::model::{ConnectionTraffic, TcpState, TrafficReport};

    struct NullSink;
    impl EventSink for NullSink {
        fn emit(&self, _event: CoreEvent) {}
    }

    struct FakeNet {
        rx: u64,
    }

    impl NetworkProvider for FakeNet {
        fn sockets(&mut self) -> CoreResult<Vec<RawSocket>> {
            Ok(vec![
                RawSocket {
                    protocol: TransportProtocol::Tcp,
                    local_addr: "192.168.1.2".parse().unwrap(),
                    local_port: 50000,
                    remote_addr: Some("127.0.0.1".parse().unwrap()),
                    remote_port: Some(443),
                    state: Some(TcpState::Established),
                    pid: Some(42),
                },
                RawSocket {
                    protocol: TransportProtocol::Tcp,
                    local_addr: "0.0.0.0".parse().unwrap(),
                    local_port: 8080,
                    remote_addr: None,
                    remote_port: None,
                    state: Some(TcpState::Listen),
                    pid: Some(42),
                },
                RawSocket {
                    protocol: TransportProtocol::Udp,
                    local_addr: "0.0.0.0".parse().unwrap(),
                    local_port: 0,
                    remote_addr: None,
                    remote_port: None,
                    state: None,
                    pid: Some(7),
                },
            ])
        }
        fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
            Ok((self.rx, 10))
        }
        fn traffic(&mut self) -> CoreResult<TrafficReport> {
            self.rx += 1000;
            Ok(TrafficReport {
                source: TrafficSource::PerConnection,
                connections: vec![ConnectionTraffic {
                    protocol: TransportProtocol::Tcp,
                    local_addr: "192.168.1.2".parse().unwrap(),
                    local_port: 50000,
                    remote_addr: Some("127.0.0.1".parse().unwrap()),
                    remote_port: Some(443),
                    bytes_in: self.rx,
                    bytes_out: 5,
                }],
                processes: vec![],
            })
        }
    }

    #[test]
    fn enriches_and_tracks_connections() {
        let dir = tempfile::tempdir().unwrap();
        let mut monitor = NetworkMonitor::new(
            Box::new(FakeNet { rx: 0 }),
            Arc::new(ReverseDns::with_resolver(
                Arc::new(NullSink),
                Arc::new(|_| None),
            )),
            Arc::new(GeoDb::open(dir.path())),
        );
        let mut owners = |pids: &[Pid]| {
            pids.iter()
                .map(|p| {
                    (
                        *p,
                        ProcessOwner {
                            name: format!("proc{p}"),
                            app_name: None,
                            start_time: Some(1_700_000_000),
                            role: None,
                        },
                    )
                })
                .collect::<HashMap<_, _>>()
        };
        let first = monitor.sample(&mut owners).unwrap();
        assert_eq!(first.sockets.len(), 2, "unbound UDP socket filtered");
        assert_eq!(first.traffic_source, TrafficSource::PerConnection);
        let conn = first
            .sockets
            .iter()
            .find(|s| s.local_port == 50000)
            .unwrap();
        assert_eq!(conn.id, "tcp|192.168.1.2:50000|127.0.0.1:443|42");
        assert_eq!(conn.process_name.as_deref(), Some("proc42"));
        assert_eq!(conn.process_start_time, Some(1_700_000_000));
        assert_eq!(conn.remote_scope, Some(AddrScope::Loopback));
        assert_eq!(conn.bytes_in, Some(1000));
        assert!(!conn.explanation.headline.is_empty());
        let listener = first.sockets.iter().find(|s| s.local_port == 8080).unwrap();
        assert_eq!(listener.remote_scope, None);
        assert_eq!(listener.bytes_in, None);

        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = monitor.sample(&mut owners).unwrap();
        let conn2 = second
            .sockets
            .iter()
            .find(|s| s.local_port == 50000)
            .unwrap();
        assert_eq!(conn2.first_seen_ms, conn.first_seen_ms);
        assert!(conn2.rx_bps.unwrap() > 0);
        assert!(second.throughput.rx_bps > 0);
    }
}
