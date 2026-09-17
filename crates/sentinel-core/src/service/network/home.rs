//! Map origin: the system time zone's reference location, or a persisted user override.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CoreResult, SentinelError};
use crate::model::{HomeLocation, HomeLocationInput, HomeLocationSource};
use crate::parse::zonetab;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(rename = "override")]
    user_override: Option<HomeLocationInput>,
}

pub struct HomeLocator {
    path: PathBuf,
    time_zone: Box<dyn Fn() -> Option<String> + Send + Sync>,
}

impl HomeLocator {
    pub fn new(path: PathBuf, time_zone: Box<dyn Fn() -> Option<String> + Send + Sync>) -> Self {
        Self { path, time_zone }
    }

    fn stored(&self) -> Stored {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn get(&self) -> CoreResult<HomeLocation> {
        if let Some(input) = self.stored().user_override {
            return Ok(HomeLocation {
                lat: input.lat,
                lon: input.lon,
                label: input.label,
                source: HomeLocationSource::UserSetting,
            });
        }
        let zone = (self.time_zone)().ok_or_else(|| SentinelError::Unavailable {
            feature: "home location".to_owned(),
            reason: "the system time zone could not be determined; set your location manually"
                .to_owned(),
        })?;
        let (lat, lon) = zonetab::coordinates(zonetab::ZONE_TAB, &zone).ok_or_else(|| {
            SentinelError::Unavailable {
                feature: "home location".to_owned(),
                reason: format!(
                    "the time zone {zone} has no reference location; set your location manually"
                ),
            }
        })?;
        Ok(HomeLocation {
            lat,
            lon,
            label: zonetab::zone_label(&zone),
            source: HomeLocationSource::TimeZone,
        })
    }

    /// `None` clears the override and returns to the time zone location.
    pub fn set(&self, input: Option<HomeLocationInput>) -> CoreResult<HomeLocation> {
        let input = input.map(validate).transpose()?;
        let stored = Stored {
            user_override: input,
        };
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| SentinelError::io(&err, Some(parent)))?;
        }
        let json = serde_json::to_vec_pretty(&stored).map_err(SentinelError::internal)?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, json).map_err(|err| SentinelError::io(&err, Some(&temp)))?;
        std::fs::rename(&temp, &self.path)
            .map_err(|err| SentinelError::io(&err, Some(&self.path)))?;
        self.get()
    }
}

fn validate(input: HomeLocationInput) -> CoreResult<HomeLocationInput> {
    if !input.lat.is_finite() || !(-90.0..=90.0).contains(&input.lat) {
        return Err(SentinelError::invalid(
            "latitude must be between -90 and 90",
        ));
    }
    if !input.lon.is_finite() || !(-180.0..=180.0).contains(&input.lon) {
        return Err(SentinelError::invalid(
            "longitude must be between -180 and 180",
        ));
    }
    let label = input.label.trim();
    if label.is_empty() || label.chars().count() > 100 {
        return Err(SentinelError::invalid(
            "location name must be 1 to 100 characters",
        ));
    }
    Ok(HomeLocationInput {
        lat: input.lat,
        lon: input.lon,
        label: label.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_zone_then_override_then_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let locator = HomeLocator::new(
            dir.path().join("home_location.json"),
            Box::new(|| Some("Europe/Paris".to_owned())),
        );
        let home = locator.get().unwrap();
        assert_eq!(home.source, HomeLocationSource::TimeZone);
        assert_eq!(home.label, "Paris");
        assert!((home.lat - 48.8667).abs() < 0.01);

        let custom = locator
            .set(Some(HomeLocationInput {
                lat: 12.97,
                lon: 77.59,
                label: " Bengaluru ".into(),
            }))
            .unwrap();
        assert_eq!(custom.source, HomeLocationSource::UserSetting);
        assert_eq!(custom.label, "Bengaluru");
        assert_eq!(locator.get().unwrap().label, "Bengaluru");

        assert!(
            locator
                .set(Some(HomeLocationInput {
                    lat: 91.0,
                    lon: 0.0,
                    label: "x".into()
                }))
                .is_err()
        );
        assert_eq!(
            locator.set(None).unwrap().source,
            HomeLocationSource::TimeZone
        );
    }

    #[test]
    fn unknown_zone_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let locator = HomeLocator::new(dir.path().join("h.json"), Box::new(|| None));
        assert!(matches!(
            locator.get(),
            Err(SentinelError::Unavailable { .. })
        ));
    }
}
