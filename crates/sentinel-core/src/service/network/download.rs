//! User-triggered download of the DB-IP City Lite database (CC BY 4.0) into the app data dir.

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;

use super::geo::{GeoDb, download_urls};
use crate::error::{CoreResult, SentinelError};
use crate::events::{CoreEvent, EventSink};
use crate::util::now_secs;

const PROGRESS_EVERY: Duration = Duration::from_millis(250);

struct Counting<R, F: FnMut(u64)> {
    inner: R,
    read: u64,
    on_progress: F,
}

impl<R: Read, F: FnMut(u64)> Read for Counting<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        (self.on_progress)(self.read);
        Ok(n)
    }
}

fn network_error(url: &str, err: impl std::fmt::Display) -> SentinelError {
    SentinelError::Network {
        detail: format!("downloading {url} failed: {err}"),
    }
}

/// Blocking; emits `sentinel:geo-db` progress and a final Ready or Failed status.
pub fn download(geo: &GeoDb, sink: &dyn EventSink) -> CoreResult<()> {
    let result = run(geo, sink);
    match &result {
        Ok(()) => {}
        Err(err) => geo.set_failed(err.to_string()),
    }
    sink.emit(CoreEvent::GeoDb(geo.status()));
    result
}

fn run(geo: &GeoDb, sink: &dyn EventSink) -> CoreResult<()> {
    std::fs::create_dir_all(geo.dir()).map_err(|err| SentinelError::io(&err, Some(geo.dir())))?;
    geo.set_downloading(0, None);
    sink.emit(CoreEvent::GeoDb(geo.status()));

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut not_found = Vec::new();
    for url in download_urls(now_secs()) {
        let response = match agent.get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => {
                not_found.push(url);
                continue;
            }
            Err(err) => return Err(network_error(&url, err)),
        };
        let total = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let part = geo.dir().join(format!("{}.part", super::geo::DB_FILE));
        let mut last_emit = Instant::now();
        let reader = Counting {
            inner: response
                .into_body()
                .into_with_config()
                .limit(u64::MAX)
                .reader(),
            read: 0,
            on_progress: |read| {
                if last_emit.elapsed() >= PROGRESS_EVERY {
                    last_emit = Instant::now();
                    geo.set_downloading(read, total);
                    sink.emit(CoreEvent::GeoDb(geo.status()));
                }
            },
        };
        let mut decoder = GzDecoder::new(reader);
        let file = File::create(&part).map_err(|err| SentinelError::io(&err, Some(&part)))?;
        let mut writer = BufWriter::new(file);
        io::copy(&mut decoder, &mut writer).map_err(|err| network_error(&url, err))?;
        writer
            .flush()
            .map_err(|err| SentinelError::io(&err, Some(&part)))?;
        drop(writer);
        // Validate before replacing any existing database.
        geo.install(&part)?;
        let final_path = geo.db_path();
        std::fs::rename(&part, &final_path)
            .map_err(|err| SentinelError::io(&err, Some(&final_path)))?;
        return geo.install(&final_path);
    }
    Err(SentinelError::Network {
        detail: format!(
            "DB-IP has not published a database at {}",
            not_found.join(" or ")
        ),
    })
}
