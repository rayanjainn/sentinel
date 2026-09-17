//! Local IP geolocation from a DB-IP City Lite mmdb in the app data directory. Lookups never
//! leave the machine; only `Public` addresses are looked up.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use maxminddb::{Mmap, Reader, geoip2};
use parking_lot::{Mutex, RwLock};

use crate::error::{CoreResult, SentinelError};
use crate::model::{GeoDbStatus, GeoLocation};
use crate::util::utc_year_month;

pub const ATTRIBUTION: &str = "IP Geolocation by DB-IP (https://db-ip.com)";
pub const DB_FILE: &str = "dbip-city-lite.mmdb";
const CACHE_LIMIT: usize = 8192;

enum State {
    Missing,
    Ready {
        reader: Box<Reader<Mmap>>,
        build_date: Option<String>,
    },
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Failed(String),
}

pub struct GeoDb {
    dir: PathBuf,
    state: RwLock<State>,
    cache: Mutex<HashMap<IpAddr, Option<GeoLocation>>>,
}

impl GeoDb {
    /// Opens the database if it was downloaded before.
    pub fn open(dir: &Path) -> Self {
        let geo = Self {
            dir: dir.to_path_buf(),
            state: RwLock::new(State::Missing),
            cache: Mutex::new(HashMap::new()),
        };
        let path = geo.db_path();
        if path.exists()
            && let Err(err) = geo.install(&path)
        {
            *geo.state.write() = State::Failed(format!(
                "The downloaded geolocation database could not be read ({err}); download it again."
            ));
        }
        geo
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.dir.join(DB_FILE)
    }

    pub fn status(&self) -> GeoDbStatus {
        match &*self.state.read() {
            State::Missing => GeoDbStatus::Missing,
            State::Ready { build_date, .. } => GeoDbStatus::Ready {
                build_date: build_date.clone(),
                attribution: ATTRIBUTION.to_owned(),
            },
            State::Downloading {
                downloaded_bytes,
                total_bytes,
            } => GeoDbStatus::Downloading {
                downloaded_bytes: *downloaded_bytes,
                total_bytes: *total_bytes,
            },
            State::Failed(message) => GeoDbStatus::Failed {
                message: message.clone(),
            },
        }
    }

    pub fn is_downloading(&self) -> bool {
        matches!(*self.state.read(), State::Downloading { .. })
    }

    pub fn set_downloading(&self, downloaded_bytes: u64, total_bytes: Option<u64>) {
        let mut state = self.state.write();
        // A working database stays usable while a refresh downloads.
        if !matches!(*state, State::Ready { .. }) || downloaded_bytes == 0 {
            *state = State::Downloading {
                downloaded_bytes,
                total_bytes,
            };
        }
    }

    pub fn set_failed(&self, message: String) {
        *self.state.write() = State::Failed(message);
    }

    /// Loads and validates an mmdb file, making it the active database.
    pub fn install(&self, path: &Path) -> CoreResult<()> {
        // SAFETY: the mapped file is owned by Sentinel and only ever replaced by rename, which
        // leaves existing mappings pointing at the previous, unmodified file.
        let reader = unsafe { Reader::open_mmap(path) }.map_err(|err| SentinelError::Io {
            detail: format!("invalid geolocation database: {err}"),
            path: Some(path.to_string_lossy().into_owned()),
        })?;
        let build_date = Some(build_date(reader.metadata().build_epoch));
        *self.state.write() = State::Ready {
            reader: Box::new(reader),
            build_date,
        };
        self.cache.lock().clear();
        Ok(())
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<GeoLocation> {
        if let Some(hit) = self.cache.lock().get(&ip) {
            return hit.clone();
        }
        let located = {
            let state = self.state.read();
            let State::Ready { reader, .. } = &*state else {
                return None;
            };
            locate(reader, ip)
        };
        let mut cache = self.cache.lock();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(ip, located.clone());
        located
    }
}

fn locate(reader: &Reader<Mmap>, ip: IpAddr) -> Option<GeoLocation> {
    let result = reader.lookup(ip).ok()?;
    let city: geoip2::City = result.decode().ok()??;
    let lat = city.location.latitude?;
    let lon = city.location.longitude?;
    Some(GeoLocation {
        lat,
        lon,
        city: city.city.names.english.map(str::to_owned),
        region: city
            .subdivisions
            .first()
            .and_then(|s| s.names.english)
            .map(str::to_owned),
        country: city.country.names.english.map(str::to_owned),
        country_code: city.country.iso_code.map(str::to_owned),
    })
}

fn build_date(epoch: u64) -> String {
    let (year, month) = utc_year_month(epoch);
    format!("{year:04}-{month:02}")
}

/// DB-IP publishes monthly; the current month may not be out yet early in the month.
pub fn download_urls(now_secs: u64) -> Vec<String> {
    let (year, month) = utc_year_month(now_secs);
    let (prev_year, prev_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    [(year, month), (prev_year, prev_month)]
        .iter()
        .map(|(y, m)| {
            format!("https://download.db-ip.com/free/dbip-city-lite-{y:04}-{m:02}.mmdb.gz")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_urls_try_current_then_previous_month() {
        let jan_2027 = 1_798_761_600 + 86_400 * 3;
        assert_eq!(
            download_urls(jan_2027),
            vec![
                "https://download.db-ip.com/free/dbip-city-lite-2027-01.mmdb.gz".to_owned(),
                "https://download.db-ip.com/free/dbip-city-lite-2026-12.mmdb.gz".to_owned(),
            ]
        );
    }

    #[test]
    fn missing_database_reports_missing_and_rejects_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let geo = GeoDb::open(dir.path());
        assert_eq!(geo.status(), GeoDbStatus::Missing);
        assert_eq!(geo.lookup("8.8.8.8".parse().unwrap()), None);
        let bogus = dir.path().join("bogus.mmdb");
        std::fs::write(&bogus, b"not a database").unwrap();
        assert!(geo.install(&bogus).is_err());
        assert_eq!(geo.status(), GeoDbStatus::Missing);
    }
}
