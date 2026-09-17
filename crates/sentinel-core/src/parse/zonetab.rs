//! tzdb `zone.tab` / `zone1970.tab` coordinates and the CLDR Windows zone mapping.

/// The tz database's zone table (public domain), bundled so every OS resolves the same way.
pub const ZONE_TAB: &str = include_str!("../../data/zone.tab");
/// Windows time zone key → IANA zone, from CLDR `windowsZones.xml` (territory 001).
pub const WINDOWS_ZONES: &str = include_str!("../../data/windows_zones.tsv");

/// Latitude/longitude of a zone's principal location.
pub fn coordinates(table: &str, zone: &str) -> Option<(f64, f64)> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut fields = line.split('\t');
            let _countries = fields.next()?;
            let coords = fields.next()?;
            let name = fields.next()?;
            (name == zone).then(|| parse_iso6709(coords)).flatten()
        })
}

/// `±DDMM±DDDMM` or `±DDMMSS±DDDMMSS`.
pub fn parse_iso6709(text: &str) -> Option<(f64, f64)> {
    let split = text[1..].find(['+', '-'])? + 1;
    let (lat, lon) = text.split_at(split);
    Some((dms(lat, 2)?, dms(lon, 3)?))
}

fn dms(part: &str, degree_digits: usize) -> Option<f64> {
    let sign = match part.as_bytes().first()? {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return None,
    };
    let digits = &part[1..];
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number =
        |range: std::ops::Range<usize>| digits.get(range).and_then(|s| s.parse::<f64>().ok());
    let degrees = number(0..degree_digits)?;
    let minutes = number(degree_digits..degree_digits + 2)?;
    let seconds = match digits.len() - degree_digits {
        2 => 0.0,
        4 => number(degree_digits + 2..degree_digits + 4)?,
        _ => return None,
    };
    Some(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
}

pub fn windows_to_iana(table: &str, windows_zone: &str) -> Option<String> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let (windows, iana) = line.split_once('\t')?;
            (windows == windows_zone).then(|| iana.trim().to_owned())
        })
}

/// "America/Argentina/Buenos_Aires" → "Buenos Aires".
pub fn zone_label(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

/// Zone name from a `/etc/localtime` symlink target or a `TZ` value.
pub fn zone_from_path(target: &str) -> Option<String> {
    let trimmed = target.trim().trim_start_matches(':');
    let zone = match trimmed.find("zoneinfo/") {
        Some(index) => &trimmed[index + "zoneinfo/".len()..],
        None => trimmed,
    };
    let zone = zone.strip_prefix("posix/").unwrap_or(zone);
    (zone.contains('/') && !zone.starts_with('/')).then(|| zone.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundled_zone_table() {
        let (lat, lon) = coordinates(ZONE_TAB, "Asia/Kolkata").unwrap();
        assert!((lat - 22.5333).abs() < 0.01 && (lon - 88.3667).abs() < 0.01);
        let (lat, lon) = coordinates(ZONE_TAB, "America/New_York").unwrap();
        assert!((lat - 40.714).abs() < 0.01 && (lon + 74.006).abs() < 0.01);
        assert!(coordinates(ZONE_TAB, "Mars/Olympus").is_none());
    }

    #[test]
    fn parses_iso6709_with_seconds() {
        let (lat, lon) = parse_iso6709("-373158+1445812").unwrap();
        assert!((lat + 37.5328).abs() < 0.001 && (lon - 144.9700).abs() < 0.001);
        assert!(parse_iso6709("garbage").is_none());
    }

    #[test]
    fn maps_windows_zones_and_paths() {
        assert_eq!(
            windows_to_iana(WINDOWS_ZONES, "Pacific Standard Time").as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(
            windows_to_iana(WINDOWS_ZONES, "India Standard Time").as_deref(),
            Some("Asia/Calcutta")
        );
        assert_eq!(
            zone_from_path("/var/db/timezone/zoneinfo/Asia/Kolkata").as_deref(),
            Some("Asia/Kolkata")
        );
        assert_eq!(
            zone_from_path(":Europe/Paris").as_deref(),
            Some("Europe/Paris")
        );
        assert_eq!(zone_from_path("UTC"), None);
        assert_eq!(zone_label("America/Argentina/Buenos_Aires"), "Buenos Aires");
    }
}
