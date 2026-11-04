//! dBFS unit maths shared by every component.
//!
//! Levels travel through the library in **decibels full scale**, never in
//! the 0..1 normalized floats widgets usually get. `-Infinity` is digital
//! silence and stays `-Infinity` through the conversions instead of
//! collapsing to the bottom of a range.

/// Digital silence.
pub const SILENCE_DB: f32 = f32::NEG_INFINITY;

/// Default bottom of a meter's range, in dBFS.
pub const DEFAULT_MIN_DB: f32 = -60.0;

/// Default top of a meter's range, in dBFS.
pub const DEFAULT_MAX_DB: f32 = 0.0;

/// Amplitude decibels: 20 log10, not the 10 log10 power convention.
const AMPLITUDE_DB_FACTOR: f32 = 20.0;

/// Text shown in place of a missing reading: two ASCII hyphens, matching a
/// hardware meter with nothing to display.
const NO_READING: &str = "--";

/// Text shown for digital silence: a true minus and an infinity, the way a
/// meter face reads, not the ASCII `-inf` a console would print.
const SILENCE_TEXT: &str = "\u{2212}\u{221e}";

/// The typographic minus U+2212, which is what a negative level is printed
/// with. A hyphen is narrower than a digit and would break the column.
const MINUS: &str = "\u{2212}";

/// Clamp `value` into `min..=max`. NaN returns `min`.
#[must_use]
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value.is_nan() { min } else { value.clamp(min, max) }
}

/// Decibels to linear gain. Silence gives `0.0`.
#[must_use]
pub fn db_to_gain(db: f32) -> f32 {
    if db == SILENCE_DB || db.is_nan() {
        0.0
    } else {
        10f32.powf(db / AMPLITUDE_DB_FACTOR)
    }
}

/// Linear gain to decibels. Zero or negative gives silence.
#[must_use]
pub fn gain_to_db(gain: f32) -> f32 {
    if gain <= 0.0 { SILENCE_DB } else { AMPLITUDE_DB_FACTOR * gain.log10() }
}

/// dBFS to a 0..1 position, linear in dB across `min_db..max_db`.
///
/// Anything at or below `min_db`, silence included, gives `0.0`.
#[must_use]
pub fn db_to_level(db: f32, min_db: f32, max_db: f32) -> f32 {
    if !(db > min_db) { 0.0 } else { clamp((db - min_db) / (max_db - min_db), 0.0, 1.0) }
}

/// A 0..1 position back to dBFS. The inverse of [`db_to_level`].
#[must_use]
pub fn level_to_db(level: f32, min_db: f32, max_db: f32) -> f32 {
    min_db + clamp(level, 0.0, 1.0) * (max_db - min_db)
}

/// Clamp a dB value to a range, keeping silence as silence when
/// `allow_silence` is set.
#[must_use]
pub fn clamp_db(db: f32, min_db: f32, max_db: f32, allow_silence: bool) -> f32 {
    if db == SILENCE_DB {
        if allow_silence { SILENCE_DB } else { min_db }
    } else {
        clamp(db, min_db, max_db)
    }
}

/// Sign policy for [`format_db`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sign {
    /// `+` above zero, `-` below, none at zero.
    Auto,
    /// Only ever show a `-`.
    NegativeOnly,
    /// Never show a sign.
    Never,
}

/// How a level string should read.
#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    /// Digits after the decimal point.
    pub decimals: usize,
    /// Append `" dB"`.
    pub unit: bool,
    /// Sign policy.
    pub sign: Sign,
    /// At or below this value, print silence instead.
    pub floor_db: f32,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { decimals: 1, unit: true, sign: Sign::Auto, floor_db: SILENCE_DB }
    }
}

/// Format decibels for display: `"−12.3 dB"`, `"+3.0 dB"`, `"−∞ dB"`.
///
/// The sign is U+2212 and silence is U+2212 U+221E, as on a meter face; a
/// NaN input reads as `--`, the "no measurement yet" state, so a blank meter
/// never lies about being silent.
#[must_use]
pub fn format_db(db: f32, options: FormatOptions) -> String {
    let suffix = if options.unit { " dB" } else { "" };
    if db.is_nan() {
        return format!("{NO_READING}{suffix}");
    }
    if db == SILENCE_DB || db <= options.floor_db {
        return format!("{SILENCE_TEXT}{suffix}");
    }

    // Round to the shown precision first: -0.04 dB displays as "0.0" and
    // must not carry a sign, which is what a meter reader expects at zero.
    let step = 10f32.powi(options.decimals as i32);
    let rounded = (db * step).round() / step;
    let magnitude = format!("{:.1$}", rounded.abs(), options.decimals);
    if rounded == 0.0 || options.sign == Sign::Never {
        return format!("{magnitude}{suffix}");
    }
    let sign = match options.sign {
        Sign::Auto if rounded < 0.0 => MINUS,
        Sign::Auto => "+",
        Sign::NegativeOnly if rounded < 0.0 => MINUS,
        Sign::NegativeOnly | Sign::Never => "",
    };
    format!("{sign}{magnitude}{suffix}")
}

/// Format decibels with the default options.
#[must_use]
pub fn format_db_default(db: f32) -> String {
    format_db(db, FormatOptions::default())
}

/// Sample peak of a block, in dBFS.
#[must_use]
pub fn peak_db(samples: &[f32]) -> f32 {
    let mut peak = 0.0f32;
    for &sample in samples {
        let magnitude = sample.abs();
        if magnitude > peak {
            peak = magnitude;
        }
    }
    gain_to_db(peak)
}

/// RMS level of a block, in dBFS.
#[must_use]
pub fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return SILENCE_DB;
    }
    let sum_of_squares: f32 = samples.iter().map(|&s| s * s).sum();
    gain_to_db((sum_of_squares / samples.len() as f32).sqrt())
}

/// The loudest of a set of readings, treating silence as the floor.
#[must_use]
pub fn loudest(levels: &[f32]) -> f32 {
    levels.iter().fold(SILENCE_DB, |loudest, &db| loudest.max(db))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_gain_round_trip() {
        for db in [-90.0, -24.0, -6.0, 0.0, 6.0] {
            assert!((gain_to_db(db_to_gain(db)) - db).abs() < 1e-4, "{db} dB");
        }
    }

    #[test]
    fn silence_survives_the_conversions() {
        assert_eq!(db_to_gain(SILENCE_DB), 0.0);
        assert_eq!(gain_to_db(0.0), SILENCE_DB);
        assert_eq!(db_to_level(SILENCE_DB, -60.0, 0.0), 0.0);
    }

    #[test]
    fn level_is_linear_in_db_over_the_range() {
        assert_eq!(db_to_level(-30.0, -60.0, 0.0), 0.5);
        assert_eq!(level_to_db(0.5, -60.0, 0.0), -30.0);
        assert_eq!(db_to_level(-80.0, -60.0, 0.0), 0.0);
        assert_eq!(db_to_level(6.0, -60.0, 0.0), 1.0);
    }

    #[test]
    fn formats_signs_units_and_silence() {
        let plain = FormatOptions { unit: false, ..FormatOptions::default() };
        assert_eq!(format_db(-12.34, FormatOptions::default()), "−12.3 dB");
        assert_eq!(format_db(3.0, FormatOptions::default()), "+3.0 dB");
        assert_eq!(format_db(0.0, FormatOptions::default()), "0.0 dB");
        assert_eq!(format_db(-0.04, plain), "0.0");
        assert_eq!(format_db(SILENCE_DB, FormatOptions::default()), "−∞ dB");
        assert_eq!(format_db(f32::NAN, FormatOptions::default()), "-- dB");
        assert_eq!(
            format_db(-70.0, FormatOptions { floor_db: -60.0, ..FormatOptions::default() }),
            "−∞ dB"
        );
        assert_eq!(
            format_db(-12.0, FormatOptions { sign: Sign::Never, ..FormatOptions::default() }),
            "12.0 dB"
        );
    }

    #[test]
    fn the_minus_is_the_typographic_one_a_meter_face_uses() {
        // A hyphen is narrower than a digit, so a readout column would jitter
        // if the sign were ASCII.
        let text = format_db(-6.0, FormatOptions::default());
        assert!(text.starts_with('\u{2212}'), "{text} should open with U+2212");
        assert!(format_db(SILENCE_DB, FormatOptions::default()).contains('\u{221e}'));
    }

    #[test]
    fn rounding_happens_before_the_sign_is_chosen() {
        let negative_only = FormatOptions { sign: Sign::NegativeOnly, ..FormatOptions::default() };
        assert_eq!(format_db(-0.04, negative_only), "0.0 dB", "a rounded zero is unsigned");
        assert_eq!(format_db(-6.0, negative_only), "−6.0 dB");
        assert_eq!(format_db(6.0, negative_only), "6.0 dB", "negative only ever shows a minus");
    }

    #[test]
    fn block_measurements() {
        let block = [0.5, -1.0, 0.25, 0.0];
        assert_eq!(peak_db(&block), 0.0);
        assert_eq!(peak_db(&[]), SILENCE_DB);
        assert!((rms_db(&[1.0, -1.0]) - 0.0).abs() < 1e-5);
        assert_eq!(rms_db(&[]), SILENCE_DB);
        assert_eq!(loudest(&[-20.0, -6.0, -30.0]), -6.0);
        assert_eq!(loudest(&[]), SILENCE_DB);
    }

    #[test]
    fn clamp_db_keeps_silence_optional() {
        assert_eq!(clamp_db(SILENCE_DB, -60.0, 0.0, false), -60.0);
        assert_eq!(clamp_db(SILENCE_DB, -60.0, 0.0, true), SILENCE_DB);
        assert_eq!(clamp_db(f32::NAN, -60.0, 0.0, true), -60.0);
    }
}
