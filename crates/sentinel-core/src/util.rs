//! Small helpers shared across services and platforms.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::SentinelError;
use crate::model::{Pid, TimestampMs, TimestampSecs};

/// `canonicalize` on Windows yields `\\?\C:\…`; show and match plain paths. Shared by every
/// place that canonicalizes a user- or test-supplied path (storage scans, file actions, and
/// their tests), so production and tests always agree on what a "real" path looks like.
pub(crate) fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\")
            && !rest.starts_with("UNC\\")
        {
            return PathBuf::from(rest);
        }
    }
    path
}

pub fn now_ms() -> TimestampMs {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_secs() -> TimestampSecs {
    now_ms() / 1000
}

pub fn system_time_secs(time: SystemTime) -> Option<TimestampSecs> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Refuses process actions that must never be taken regardless of platform.
pub fn guard_process_target(pid: Pid) -> Result<(), SentinelError> {
    if pid == 0 {
        return Err(SentinelError::invalid(
            "PID 0 is the kernel scheduler and cannot be targeted",
        ));
    }
    if pid == 1 {
        return Err(SentinelError::invalid(
            "PID 1 is the system init process; stopping it would bring down the whole system",
        ));
    }
    if pid == std::process::id() {
        return Err(SentinelError::invalid(
            "Sentinel cannot target its own process",
        ));
    }
    Ok(())
}

/// Human-readable byte size using binary units with one decimal ("8.7 GB").
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Replaces the user's home directory prefix with `~` for compact labels.
pub fn tilde_path(path: &Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".to_owned();
        }
        return format!("~{}{}", std::path::MAIN_SEPARATOR, rest.to_string_lossy());
    }
    path_string(path)
}

pub fn home_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let var = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let var = std::env::var_os("HOME");
    var.filter(|v| !v.is_empty()).map(std::path::PathBuf::from)
}

/// Turns cumulative byte counters into per-second rates. Counter resets (interface re-created,
/// wraparound) count as zero traffic for that interval rather than a huge spike.
#[derive(Debug, Default, Clone)]
pub struct RateCounter {
    previous: Option<(std::time::Instant, u64, u64)>,
    last: crate::model::NetThroughput,
}

impl RateCounter {
    pub fn update(&mut self, rx_total: u64, tx_total: u64) -> crate::model::NetThroughput {
        self.update_at(std::time::Instant::now(), rx_total, tx_total)
    }

    pub fn update_at(
        &mut self,
        now: std::time::Instant,
        rx_total: u64,
        tx_total: u64,
    ) -> crate::model::NetThroughput {
        let (rx_bps, tx_bps) = match self.previous {
            Some((at, rx, tx)) => {
                let secs = now.saturating_duration_since(at).as_secs_f64();
                if secs <= 0.0 {
                    (self.last.rx_bps, self.last.tx_bps)
                } else {
                    (
                        (rx_total.saturating_sub(rx) as f64 / secs).round() as u64,
                        (tx_total.saturating_sub(tx) as f64 / secs).round() as u64,
                    )
                }
            }
            None => (0, 0),
        };
        self.previous = Some((now, rx_total, tx_total));
        self.last = crate::model::NetThroughput {
            rx_bps,
            tx_bps,
            rx_total,
            tx_total,
        };
        self.last
    }

    pub fn last(&self) -> crate::model::NetThroughput {
        self.last
    }
}

/// Unix-style exponentially damped load average (1, 5 and 15 minute constants), fed with an
/// instantaneous run-queue estimate on platforms without a kernel load average.
#[derive(Debug, Default, Clone)]
pub struct DampedLoad {
    state: Option<(std::time::Instant, f64, f64, f64)>,
}

impl DampedLoad {
    pub fn update(&mut self, now: std::time::Instant, instant: f64) -> (f64, f64, f64) {
        let (one, five, fifteen) = match self.state {
            None => (instant, instant, instant),
            Some((at, one, five, fifteen)) => {
                let dt = now.saturating_duration_since(at).as_secs_f64();
                let damp = |value: f64, period: f64| {
                    let factor = (-dt / period).exp();
                    value * factor + instant * (1.0 - factor)
                };
                (damp(one, 60.0), damp(five, 300.0), damp(fifteen, 900.0))
            }
        };
        self.state = Some((now, one, five, fifteen));
        (one, five, fifteen)
    }
}

/// UTC (year, month 1..=12) for a Unix timestamp (Howard Hinnant's civil-from-days).
pub fn utc_year_month(secs: u64) -> (i64, u32) {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month)
}

#[cfg(test)]
mod date_tests {
    use super::utc_year_month;

    #[test]
    fn converts_timestamps_to_year_month() {
        assert_eq!(utc_year_month(0), (1970, 1));
        assert_eq!(utc_year_month(951_782_400), (2000, 2));
        assert_eq!(utc_year_month(1_789_430_400), (2026, 9));
        assert_eq!(utc_year_month(1_767_225_599), (2025, 12));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_counter_computes_and_tolerates_reset() {
        let start = std::time::Instant::now();
        let mut counter = RateCounter::default();
        assert_eq!(counter.update_at(start, 1000, 500).rx_bps, 0);
        let two = start + std::time::Duration::from_secs(2);
        let rate = counter.update_at(two, 5000, 2500);
        assert_eq!((rate.rx_bps, rate.tx_bps), (2000, 1000));
        let three = two + std::time::Duration::from_secs(1);
        let reset = counter.update_at(three, 10, 10);
        assert_eq!((reset.rx_bps, reset.tx_bps), (0, 0));
        assert_eq!(reset.rx_total, 10);
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(9_341_553_868), "8.7 GB");
    }

    #[test]
    fn guards_special_pids() {
        assert!(guard_process_target(0).is_err());
        assert!(guard_process_target(1).is_err());
        assert!(guard_process_target(std::process::id()).is_err());
        assert!(guard_process_target(u32::MAX - 1).is_ok());
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn damped_load_converges_at_different_speeds() {
        let mut load = DampedLoad::default();
        let start = Instant::now();
        assert_eq!(load.update(start, 0.0), (0.0, 0.0, 0.0));
        let (one, five, fifteen) = load.update(start + Duration::from_secs(60), 4.0);
        assert!((one - 4.0 * (1.0 - (-1f64).exp())).abs() < 1e-9);
        assert!(one > five && five > fifteen && fifteen > 0.0);
    }
}
