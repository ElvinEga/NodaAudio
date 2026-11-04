//! Metering zones: where a meter turns amber and where it turns red.

use crate::db::SILENCE_DB;

/// The name of a metering zone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Zone {
    /// Quiet enough to stay.
    Ok,
    /// Getting loud.
    Warn,
    /// At or near full scale.
    Clip,
}

/// A zone and the level it starts at, running up to the next zone.
#[derive(Clone, Copy, Debug)]
pub struct MeterZone {
    /// Lower bound of the zone, in dBFS.
    pub from_db: f32,
    /// Which zone starts here.
    pub zone: Zone,
}

/// Green below -20 dBFS, amber from -20, red from -9: the thresholds
/// broadcast engineers expect.
pub const DEFAULT_ZONES: [MeterZone; 3] = [
    MeterZone { from_db: SILENCE_DB, zone: Zone::Ok },
    MeterZone { from_db: -20.0, zone: Zone::Warn },
    MeterZone { from_db: -9.0, zone: Zone::Clip },
];

/// A level at or above this counts as a clip event.
pub const CLIP_THRESHOLD_DB: f32 = -1.0;

/// How long a clip light stays on after the last clip, in ms.
pub const CLIP_HOLD_MS: f32 = 1500.0;

/// The zone a level falls in.
///
/// The zone whose threshold the level has passed most recently wins, so a
/// caller may list zones in any order.
#[must_use]
pub fn zone_for_db(db: f32, zones: &[MeterZone]) -> Zone {
    if db.is_nan() {
        return Zone::Ok;
    }
    zones
        .iter()
        .filter(|zone| db >= zone.from_db)
        .max_by(|a, b| a.from_db.total_cmp(&b.from_db))
        .map_or(Zone::Ok, |zone| zone.zone)
}

/// The dB level where a zone begins, for drawing a hard color stop.
///
/// The lowest zone has no usable start, so it reports the bottom of the
/// display range.
#[must_use]
pub fn zone_start_db(zone: Zone, zones: &[MeterZone], fallback: f32) -> f32 {
    zones
        .iter()
        .find(|candidate| candidate.zone == zone)
        .map_or(fallback, |candidate| {
            if candidate.from_db == SILENCE_DB { fallback } else { candidate.from_db }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_thresholds() {
        assert_eq!(zone_for_db(-21.0, &DEFAULT_ZONES), Zone::Ok);
        assert_eq!(zone_for_db(-20.0, &DEFAULT_ZONES), Zone::Warn);
        assert_eq!(zone_for_db(-9.5, &DEFAULT_ZONES), Zone::Warn);
        assert_eq!(zone_for_db(-9.0, &DEFAULT_ZONES), Zone::Clip);
        assert_eq!(zone_for_db(0.0, &DEFAULT_ZONES), Zone::Clip);
    }

    #[test]
    fn silence_and_nan_fall_in_the_quiet_zone() {
        assert_eq!(zone_for_db(SILENCE_DB, &DEFAULT_ZONES), Zone::Ok);
        assert_eq!(zone_for_db(f32::NAN, &DEFAULT_ZONES), Zone::Ok);
    }

    #[test]
    fn unsorted_zones_resolve_by_threshold() {
        let zones = [
            MeterZone { from_db: -9.0, zone: Zone::Clip },
            MeterZone { from_db: -20.0, zone: Zone::Warn },
            MeterZone { from_db: SILENCE_DB, zone: Zone::Ok },
        ];
        assert_eq!(zone_for_db(-30.0, &zones), Zone::Ok);
        assert_eq!(zone_for_db(-5.0, &zones), Zone::Clip);
    }

    #[test]
    fn zone_start_replaces_the_infinite_floor() {
        assert_eq!(zone_start_db(Zone::Ok, &DEFAULT_ZONES, -60.0), -60.0);
        assert_eq!(zone_start_db(Zone::Warn, &DEFAULT_ZONES, -60.0), -20.0);
        assert_eq!(zone_start_db(Zone::Clip, &DEFAULT_ZONES, -60.0), -9.0);
    }
}
