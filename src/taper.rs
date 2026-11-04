//! Value-to-position laws for controls and meters.

use crate::db::clamp;

/// Console fader law: where 0 dB sits along the travel.
const DEFAULT_UNITY_POSITION: f32 = 0.75;

/// How sharply a console law compresses the range below 0 dB.
const AUDIO_CURVE_EXPONENT: f32 = 2.0;

/// Maps a value to a 0..1 position on a control and back.
///
/// The range lives with the call, not the taper: the same law serves a
/// -60..0 meter and a -60..+6 fader.
#[derive(Clone, Copy, Debug)]
pub enum Taper {
    /// Equal distance per unit.
    Linear,
    /// Console law: most travel around 0 dB, a fast fall to the bottom,
    /// linear headroom above it.
    Audio {
        /// Where 0 dB sits on the travel, 0..1.
        unity_position: f32,
    },
    /// Logarithmic law for frequency and time values. Needs a range above
    /// zero.
    Log,
}

impl Taper {
    /// The console law with the stock 0.75 unity position.
    pub const AUDIO: Taper = Taper::Audio { unity_position: DEFAULT_UNITY_POSITION };

    /// Where a value sits along the travel. Values at or below `min` give
    /// the start, which includes silence.
    #[must_use]
    pub fn to_position(&self, value: f32, min: f32, max: f32) -> f32 {
        match self {
            Taper::Linear => linear(value, min, max),
            Taper::Log => {
                if value <= min || min <= 0.0 || max <= min {
                    0.0
                } else {
                    clamp((value / min).ln() / (max / min).ln(), 0.0, 1.0)
                }
            }
            Taper::Audio { unity_position } => {
                let unity = unity_position.clamp(0.0, 1.0);
                // Above zero dB the law is anchored at 0, so a range with
                // headroom keeps its top linear; a range that ends below
                // unity has no headroom segment at all.
                let anchor = 0.0f32.min(max);
                let unity = if max > 0.0 { unity } else { 1.0 };
                if !(value > min) {
                    0.0
                } else if value >= max {
                    1.0
                } else if value >= anchor {
                    unity + ((value - anchor) / (max - anchor)) * (1.0 - unity)
                } else {
                    let ratio = (value - anchor) / (min - anchor);
                    (1.0 - ratio.powf(1.0 / AUDIO_CURVE_EXPONENT)) * unity
                }
            }
        }
    }

    /// The value at a position along the travel. The inverse of
    /// [`Self::to_position`].
    #[must_use]
    pub fn to_value(&self, position: f32, min: f32, max: f32) -> f32 {
        let position = clamp(position, 0.0, 1.0);
        match self {
            Taper::Linear => min + position * (max - min),
            Taper::Log => {
                if min <= 0.0 || max <= min {
                    min + position * (max - min)
                } else {
                    min * (max / min).powf(position)
                }
            }
            Taper::Audio { unity_position } => {
                let unity = unity_position.clamp(0.0, 1.0);
                let anchor = 0.0f32.min(max);
                let unity = if max > 0.0 { unity } else { 1.0 };
                if position >= unity {
                    if unity >= 1.0 {
                        anchor
                    } else {
                        anchor + ((position - unity) / (1.0 - unity)) * (max - anchor)
                    }
                } else {
                    let travel = if unity > 0.0 { position / unity } else { 0.0 };
                    anchor + (min - anchor) * (1.0 - travel).powf(AUDIO_CURVE_EXPONENT)
                }
            }
        }
    }
}

/// Equal distance per unit across the range.
fn linear(value: f32, min: f32, max: f32) -> f32 {
    if !(value > min) { 0.0 } else { clamp((value - min) / (max - min), 0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_proportional() {
        assert_eq!(Taper::Linear.to_position(-30.0, -60.0, 0.0), 0.5);
        assert_eq!(Taper::Linear.to_value(0.5, -60.0, 0.0), -30.0);
        assert_eq!(Taper::Linear.to_position(f32::NEG_INFINITY, -60.0, 0.0), 0.0);
        assert_eq!(Taper::Linear.to_position(-70.0, -60.0, 0.0), 0.0);
        assert_eq!(Taper::Linear.to_position(6.0, -60.0, 6.0), 1.0);
    }

    #[test]
    fn audio_law_anchors_unity_at_the_configured_position() {
        let taper = Taper::AUDIO;
        assert_eq!(taper.to_position(0.0, -60.0, 6.0), 0.75);
        assert!((taper.to_value(0.75, -60.0, 6.0) - 0.0).abs() < 1e-5);
        assert_eq!(taper.to_position(6.0, -60.0, 6.0), 1.0);
        assert_eq!(taper.to_position(-60.0, -60.0, 6.0), 0.0);
        // Travel is spent unevenly on purpose: the decade around 0 dB gets
        // about three times the resolution of the deep-attenuation region.
        assert!((taper.to_position(-30.0, -60.0, 6.0) - 0.21967).abs() < 1e-4);
        assert!((taper.to_value(0.5, -60.0, 6.0) + 6.667).abs() < 1e-2);
    }

    #[test]
    fn audio_law_round_trips() {
        for db in [-59.0, -40.0, -20.0, -6.0, -0.5, 0.0, 3.0, 6.0] {
            let position = Taper::AUDIO.to_position(db, -60.0, 6.0);
            let back = Taper::AUDIO.to_value(position, -60.0, 6.0);
            assert!((back - db).abs() < 1e-3, "{db} dB -> {position} -> {back}");
        }
    }

    #[test]
    fn audio_law_without_headroom_spans_the_whole_travel() {
        // A meter tops out at 0 dB: unity is the top, and the law bows
        // across the full range instead of reserving a headroom segment.
        assert_eq!(Taper::AUDIO.to_position(0.0, -60.0, 0.0), 1.0);
        assert!((Taper::AUDIO.to_position(-15.0, -60.0, 0.0) - 0.5).abs() < 1e-5);
        assert!((Taper::AUDIO.to_value(0.5, -60.0, 0.0) + 15.0).abs() < 1e-4);
    }

    #[test]
    fn log_law_round_trips() {
        let taper = Taper::Log;
        assert!((taper.to_position(100.0, 10.0, 1000.0) - 0.5).abs() < 1e-6);
        assert!((taper.to_value(0.5, 10.0, 1000.0) - 100.0).abs() < 1e-3);
        assert_eq!(taper.to_position(5.0, 10.0, 1000.0), 0.0);
    }
}
