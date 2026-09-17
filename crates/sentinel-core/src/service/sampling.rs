use crate::events::{SamplingConfig, StreamKind};

pub const INTERVALS_MS: [u32; 3] = [500, 1000, 2000];

/// Snaps the interval to a supported value and removes duplicate streams.
pub fn clamp_config(config: SamplingConfig) -> SamplingConfig {
    let interval_ms = INTERVALS_MS
        .iter()
        .copied()
        .min_by_key(|candidate| candidate.abs_diff(config.interval_ms))
        .unwrap_or(1000);
    let mut streams: Vec<StreamKind> = Vec::with_capacity(3);
    for stream in config.streams {
        if !streams.contains(&stream) {
            streams.push(stream);
        }
    }
    SamplingConfig {
        interval_ms,
        streams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_interval_and_dedupes() {
        let clamped = clamp_config(SamplingConfig {
            interval_ms: 90,
            streams: vec![
                StreamKind::Network,
                StreamKind::Resources,
                StreamKind::Network,
            ],
        });
        assert_eq!(clamped.interval_ms, 500);
        assert_eq!(
            clamped.streams,
            vec![StreamKind::Network, StreamKind::Resources]
        );
        assert_eq!(
            clamp_config(SamplingConfig {
                interval_ms: 60_000,
                streams: vec![]
            })
            .interval_ms,
            2000
        );
    }
}
