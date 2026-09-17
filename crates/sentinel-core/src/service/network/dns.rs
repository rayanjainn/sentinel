//! Asynchronous reverse DNS on a small bounded worker pool. Sampling only ever reads the cache;
//! lookups complete in the background and are pushed as `sentinel:host-resolved`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, TrySendError, bounded};
use parking_lot::Mutex;

use crate::events::{CoreEvent, EventSink};
use crate::model::HostResolved;

const WORKERS: usize = 4;
const QUEUE: usize = 256;
const POSITIVE_TTL: Duration = Duration::from_secs(30 * 60);
const NEGATIVE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_ENTRIES: usize = 4096;

enum Entry {
    Pending,
    Done { host: Option<String>, at: Instant },
}

pub type Resolve = dyn Fn(&IpAddr) -> Option<String> + Send + Sync;

struct Inner {
    cache: Mutex<HashMap<IpAddr, Entry>>,
    sink: Arc<dyn EventSink>,
}

pub struct ReverseDns {
    inner: Arc<Inner>,
    queue: Sender<IpAddr>,
}

pub fn system_resolve(addr: &IpAddr) -> Option<String> {
    dns_lookup::lookup_addr(addr)
        .ok()
        .map(|name| name.trim_end_matches('.').to_owned())
        .filter(|name| !name.is_empty() && name.parse::<IpAddr>().is_err())
}

impl ReverseDns {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self::with_resolver(sink, Arc::new(system_resolve))
    }

    pub fn with_resolver(sink: Arc<dyn EventSink>, resolve: Arc<Resolve>) -> Self {
        let (queue, jobs) = bounded::<IpAddr>(QUEUE);
        let inner = Arc::new(Inner {
            cache: Mutex::new(HashMap::new()),
            sink,
        });
        for index in 0..WORKERS {
            let jobs = jobs.clone();
            let inner = Arc::clone(&inner);
            let resolve = Arc::clone(&resolve);
            let _ = std::thread::Builder::new()
                .name(format!("sentinel-dns-{index}"))
                .spawn(move || {
                    // Ends when the ReverseDns (the only sender) is dropped.
                    for ip in jobs.iter() {
                        let host = resolve(&ip);
                        inner.cache.lock().insert(
                            ip,
                            Entry::Done {
                                host: host.clone(),
                                at: Instant::now(),
                            },
                        );
                        inner.sink.emit(CoreEvent::HostResolved(HostResolved {
                            ip: ip.to_string(),
                            hostname: host,
                        }));
                    }
                });
        }
        Self { inner, queue }
    }

    /// Cached hostname, scheduling a lookup when the address is unknown or its entry expired.
    pub fn lookup(&self, ip: IpAddr) -> Option<String> {
        let mut cache = self.inner.cache.lock();
        match cache.get(&ip) {
            Some(Entry::Pending) => return None,
            Some(Entry::Done { host, at }) => {
                let ttl = if host.is_some() {
                    POSITIVE_TTL
                } else {
                    NEGATIVE_TTL
                };
                if at.elapsed() < ttl {
                    return host.clone();
                }
            }
            None => {}
        }
        if cache.len() >= MAX_ENTRIES {
            let now = Instant::now();
            cache.retain(|_, entry| match entry {
                Entry::Pending => true,
                Entry::Done { at, .. } => now.duration_since(*at) < NEGATIVE_TTL,
            });
        }
        let stale_host = match cache.get(&ip) {
            Some(Entry::Done { host, .. }) => host.clone(),
            _ => None,
        };
        match self.queue.try_send(ip) {
            Ok(()) => {
                cache.insert(ip, Entry::Pending);
            }
            // Saturated: try again on a later sample.
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
        stale_host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Collect(Mutex<Vec<CoreEvent>>);

    impl EventSink for Collect {
        fn emit(&self, event: CoreEvent) {
            self.0.lock().push(event);
        }
    }

    #[test]
    fn resolves_in_background_and_caches() {
        let sink = Arc::new(Collect::default());
        let calls = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&calls);
        let dns = ReverseDns::with_resolver(
            sink.clone(),
            Arc::new(move |ip: &IpAddr| {
                *counter.lock() += 1;
                (ip.to_string() == "1.1.1.1").then(|| "one.one.one.one".to_owned())
            }),
        );
        let one: IpAddr = "1.1.1.1".parse().unwrap();
        let other: IpAddr = "9.9.9.9".parse().unwrap();
        assert_eq!(dns.lookup(one), None);
        assert_eq!(dns.lookup(other), None);
        let deadline = Instant::now() + Duration::from_secs(5);
        while sink.0.lock().len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(dns.lookup(one).as_deref(), Some("one.one.one.one"));
        assert_eq!(dns.lookup(other), None);
        assert_eq!(*calls.lock(), 2, "cached results are not looked up again");
        let events = sink.0.lock();
        assert!(events.iter().any(|e| matches!(e,
            CoreEvent::HostResolved(h) if h.ip == "1.1.1.1" && h.hostname.as_deref() == Some("one.one.one.one"))));
    }
}
