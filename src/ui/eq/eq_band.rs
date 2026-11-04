//! A parametric EQ band and the analytic response it draws.
//!
//! The magnitude response is evaluated from Robert Bristow-Johnson's Audio
//! EQ Cookbook coefficients rather than by filtering a signal, so a curve
//! can be drawn for a band that has never processed a sample. The display
//! sums the bands in decibels, which is how every paragraphic EQ shows its
//! composite curve.

use std::f64::consts::PI;

use egui::Color32;

use crate::db::clamp;
use crate::ui::eq::filter_type::{FilterSlope, FilterType};

/// The gain a band's handle falls back to: a bell with no boost is still a
/// band, it just sits on the zero line.
pub const NEUTRAL_GAIN_DB: f32 = 0.0;

/// The Q a new band starts at, the Butterworth value a shelf assumes.
pub const NEUTRAL_Q: f32 = 0.707;

/// A single parametric EQ band.
#[derive(Clone, Debug, PartialEq)]
pub struct EqBand {
    /// Band index / number (1-based, e.g. 1..8). Handles and selection use
    /// it as identity, so bands made by the caller must be numbered.
    pub id: usize,
    /// Center / cutoff frequency in Hz (10.0..30000.0).
    pub frequency: f32,
    /// Boost or cut gain in decibels (-30.0..+30.0 dB).
    pub gain_db: f32,
    /// Quality factor / resonance bandwidth (0.1..40.0).
    pub q: f32,
    /// Filter curve shape.
    pub filter_type: FilterType,
    /// Cutoff slope for high-pass and low-pass filters.
    pub slope: FilterSlope,
    /// Active state (true = enabled, false = bypassed).
    pub enabled: bool,
    /// Solo audition mode for this band: a band that is not soloed is
    /// bypassed while any other band is.
    pub solo: bool,
    /// Visual color accent for this band handle and curve.
    pub color: Color32,
}

impl EqBand {
    /// Create a new EQ band.
    pub fn new(id: usize, filter_type: FilterType, frequency: f32, gain_db: f32, q: f32) -> Self {
        let default_color = match id % 6 {
            1 => Color32::from_rgb(239, 68, 68),   // Red
            2 => Color32::from_rgb(245, 158, 11),  // Amber
            3 => Color32::from_rgb(16, 185, 129),  // Green
            4 => Color32::from_rgb(6, 182, 212),   // Cyan
            5 => Color32::from_rgb(59, 130, 246),  // Blue
            _ => Color32::from_rgb(168, 85, 247),  // Purple
        };

        Self {
            id,
            frequency: clamp(frequency, 10.0, 30_000.0),
            gain_db: clamp(gain_db, -30.0, 30.0),
            q: clamp(q, 0.1, 40.0),
            filter_type,
            slope: FilterSlope::Db12,
            enabled: true,
            solo: false,
            color: default_color,
        }
    }

    /// A peaking bell, the band an EQ starts with.
    #[must_use]
    pub fn bell(id: usize, frequency: f32, gain_db: f32, q: f32) -> Self {
        Self::new(id, FilterType::Bell, frequency, gain_db, q)
    }

    /// Set band color.
    #[must_use]
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Set cutoff slope.
    #[must_use]
    pub fn slope(mut self, slope: FilterSlope) -> Self {
        self.slope = slope;
        self
    }

    /// Set the filter shape without touching the other parameters.
    #[must_use]
    pub fn with_type(mut self, filter_type: FilterType) -> Self {
        self.filter_type = filter_type;
        self
    }

    /// Whether the band contributes anything to the curve.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.enabled && (self.filter_type.has_gain() || self.gain_db.abs() > 0.0)
    }

    /// Calculate the analytical magnitude response in decibels (dB) at
    /// frequency `f` (Hz), using the RBJ digital biquad formulas.
    ///
    /// A band that is switched off — or an inactive band while another one
    /// is soloed — is flat. Cut filters ignore [`Self::gain_db`]; a
    /// [`FilterType::TiltShelf`] is the sum of a low shelf at minus half the
    /// gain and a high shelf at plus half of it, so it pivots around its
    /// frequency instead of lifting one end.
    #[must_use]
    pub fn magnitude_db(&self, f: f32, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let sample_rate = sample_rate.max(8_000.0);
        let f = f.clamp(1.0, sample_rate * 0.495);
        let f0 = self.frequency.clamp(10.0, sample_rate * 0.495);
        let q = self.q.clamp(0.05, 40.0);

        match self.filter_type {
            FilterType::TiltShelf => {
                let low = coefficients(FilterType::LowShelf, f0, -self.gain_db * 0.5, q, sample_rate)
                    .response_db(f, sample_rate);
                let high =
                    coefficients(FilterType::HighShelf, f0, self.gain_db * 0.5, q, sample_rate)
                        .response_db(f, sample_rate);
                low + high
            }
            FilterType::LowCut if self.slope == FilterSlope::Db6 => {
                first_order_db(true, f, f0)
            }
            FilterType::HighCut if self.slope == FilterSlope::Db6 => {
                first_order_db(false, f, f0)
            }
            FilterType::LowCut | FilterType::HighCut => {
                // A biquad is a second-order, 12 dB/octave section; steeper
                // slopes cascade whole sections, as a plugin's oversampled
                // filters do.
                let cascades = self.slope.cascade_count() as f32;
                coefficients(self.filter_type, f0, self.gain_db, q, sample_rate)
                    .response_db(f, sample_rate)
                    * cascades
            }
            kind => coefficients(kind, f0, self.gain_db, q, sample_rate)
                .response_db(f, sample_rate),
        }
    }

    /// The magnitude response of this band on a log-spaced grid, which is
    /// what a curve display plots.
    #[must_use]
    pub fn response(&self, from_hz: f32, to_hz: f32, points: usize, sample_rate: f32) -> Vec<f32> {
        let points = points.max(2);
        let ratio = (to_hz / from_hz).abs().max(1.0);
        (0..points)
            .map(|i| {
                let t = i as f32 / (points - 1) as f32;
                self.magnitude_db(from_hz * ratio.powf(t), sample_rate)
            })
            .collect()
    }
}

/// The normalised coefficients of one RBJ biquad section.
#[derive(Clone, Copy, Debug)]
struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a0: f64,
    a1: f64,
    a2: f64,
}

impl Coefficients {
    /// |H(e^{jw})| in dB, evaluated at an ordinary frequency in Hz.
    fn response_db(self, f: f32, sample_rate: f32) -> f32 {
        let w = 2.0 * PI * f64::from(f) / f64::from(sample_rate);
        let (sin_w, cos_w) = w.sin_cos();
        let (sin_2w, cos_2w) = (2.0 * w).sin_cos();

        let num_re = self.b0 + self.b1 * cos_w + self.b2 * cos_2w;
        let num_im = -(self.b1 * sin_w + self.b2 * sin_2w);
        let den_re = self.a0 + self.a1 * cos_w + self.a2 * cos_2w;
        let den_im = -(self.a1 * sin_w + self.a2 * sin_2w);

        let num = num_re * num_re + num_im * num_im;
        let den = den_re * den_re + den_im * den_im;
        let magnitude_sq = if den > 1e-24 { num / den } else { 1.0 };
        (10.0 * magnitude_sq.max(1e-12).log10()) as f32
    }
}

/// RBJ's cookbook coefficients for one section. `a0` is kept unnormalised
/// and the response divides by it, which avoids a division per coefficient.
fn coefficients(
    kind: FilterType,
    f0: f32,
    gain_db: f32,
    q: f32,
    sample_rate: f32,
) -> Coefficients {
    let f0 = f64::from(f0);
    let sample_rate = f64::from(sample_rate);
    let a = 10f64.powf(f64::from(gain_db) / 40.0);
    let w0 = 2.0 * PI * f0 / sample_rate;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / (2.0 * f64::from(q.max(0.05)));

    let (b0, b1, b2, a0, a1, a2) = match kind {
        FilterType::Bell => (
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        ),
        FilterType::LowCut => (
            (1.0 + cos_w0) * 0.5,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::HighCut => (
            (1.0 - cos_w0) * 0.5,
            1.0 - cos_w0,
            (1.0 - cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::LowShelf => {
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
                a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
                (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
            )
        }
        FilterType::HighShelf => {
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
                a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
                (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
            )
        }
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::BandPass => (
            alpha,
            0.0,
            -alpha,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        // The tilt is built from two shelves by the caller; this branch is
        // only reached if a future type is added without one.
        FilterType::TiltShelf => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
    };

    Coefficients { b0, b1, b2, a0, a1, a2 }
}

/// A first-order section: 6 dB/octave instead of a biquad's 12.
fn first_order_db(high_pass: bool, f: f32, cutoff_hz: f32) -> f32 {
    let ratio = f64::from(f / cutoff_hz.max(1.0));
    let ratio_sq = ratio * ratio;
    let magnitude_sq = if high_pass {
        ratio_sq / (1.0 + ratio_sq)
    } else {
        1.0 / (1.0 + ratio_sq)
    };
    (10.0 * magnitude_sq.max(1e-12).log10()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn band(kind: FilterType, f: f32, gain: f32, q: f32) -> EqBand {
        EqBand::new(1, kind, f, gain, q)
    }

    #[test]
    fn bell_filter_peak_gain_matches() {
        let b = band(FilterType::Bell, 1000.0, 6.0, 1.41);
        let mag = b.magnitude_db(1000.0, FS);
        assert!((mag - 6.0).abs() < 0.2, "peak should match +6 dB gain, got {mag}");
        assert!(b.magnitude_db(8000.0, FS).abs() < 0.2, "an octave away is flat");
    }

    #[test]
    fn low_cut_attenuates_sub_bass() {
        let b = band(FilterType::LowCut, 100.0, 0.0, 0.707);
        let at_cut = b.magnitude_db(100.0, FS);
        let at_sub = b.magnitude_db(20.0, FS);
        assert!((at_cut - -3.0).abs() < 0.5, "cutoff should be ~ -3dB, got {at_cut}");
        assert!(at_sub < -20.0, "sub bass should be deeply attenuated, got {at_sub}");
    }

    #[test]
    fn high_cut_is_the_mirror() {
        let b = band(FilterType::HighCut, 8000.0, 0.0, 0.707);
        assert!((b.magnitude_db(8000.0, FS) + 3.0).abs() < 0.5);
        assert!(b.magnitude_db(16_000.0, FS) < -10.0);
        assert!(b.magnitude_db(200.0, FS).abs() < 0.1);
    }

    #[test]
    fn notch_rejects_only_its_centre() {
        let b = band(FilterType::Notch, 1000.0, 0.0, 4.0);
        assert!(b.magnitude_db(1000.0, FS) < -60.0);
        assert!(b.magnitude_db(100.0, FS).abs() < 1.0);
        assert!(b.magnitude_db(10_000.0, FS).abs() < 1.0);
    }

    #[test]
    fn shelves_reach_half_their_gain_at_the_corner() {
        let low = band(FilterType::LowShelf, 200.0, 6.0, NEUTRAL_Q);
        assert!((low.magnitude_db(20.0, FS) - 6.0).abs() < 0.2);
        assert!((low.magnitude_db(200.0, FS) - 3.0).abs() < 0.4);
        assert!(low.magnitude_db(5000.0, FS).abs() < 0.2);

        let high = band(FilterType::HighShelf, 5000.0, -9.0, NEUTRAL_Q);
        assert!((high.magnitude_db(20_000.0, FS) + 9.0).abs() < 0.3);
        assert!(high.magnitude_db(200.0, FS).abs() < 0.2);
    }

    #[test]
    fn a_tilt_shelf_pivots_around_its_frequency() {
        let tilt = band(FilterType::TiltShelf, 1000.0, 12.0, NEUTRAL_Q);
        let low = tilt.magnitude_db(20.0, FS);
        let at = tilt.magnitude_db(1000.0, FS);
        let high = tilt.magnitude_db(20_000.0, FS);
        assert!((low + 6.0).abs() < 1.0, "low end should fall by half the tilt, got {low}");
        assert!(at.abs() < 1.0, "the pivot should stay near unity, got {at}");
        assert!((high - 6.0).abs() < 1.0, "high end should rise by half, got {high}");
    }

    #[test]
    fn a_six_db_slope_is_gentler_than_a_twenty_four() {
        let mut gentle = band(FilterType::LowCut, 200.0, 0.0, 0.707);
        gentle.slope = FilterSlope::Db6;
        let gentle_db = gentle.magnitude_db(100.0, FS);
        gentle.slope = FilterSlope::Db24;
        let steep_db = gentle.magnitude_db(100.0, FS);
        assert!(gentle_db < -3.0 && gentle_db > -8.0, "6 dB/oct one octave below: {gentle_db}");
        assert!(steep_db < gentle_db - 10.0, "24 dB/oct should be much steeper: {steep_db}");
    }

    #[test]
    fn a_higher_q_narrows_a_bell() {
        let wide = band(FilterType::Bell, 1000.0, 12.0, 0.7);
        let narrow = band(FilterType::Bell, 1000.0, 12.0, 6.0);
        assert_eq!(wide.magnitude_db(1000.0, FS), narrow.magnitude_db(1000.0, FS));
        assert!(narrow.magnitude_db(500.0, FS) < wide.magnitude_db(500.0, FS));
    }

    #[test]
    fn a_disabled_band_is_flat() {
        let mut b = EqBand::bell(1, 1000.0, 12.0, 1.0);
        b.enabled = false;
        for f in [20.0, 100.0, 1000.0, 10_000.0] {
            assert_eq!(b.magnitude_db(f, FS), 0.0);
        }
    }

    #[test]
    fn bands_sum_in_decibels() {
        let a = EqBand::bell(1, 1000.0, 6.0, 1.0);
        let b = EqBand::bell(2, 1000.0, 6.0, 1.0);
        assert!((a.magnitude_db(1000.0, FS) + b.magnitude_db(1000.0, FS) - 12.0).abs() < 0.4);
    }

    #[test]
    fn every_kind_stays_finite_at_the_extremes() {
        for kind in FilterType::ALL {
            for slope in [
                FilterSlope::Db6,
                FilterSlope::Db12,
                FilterSlope::Db18,
                FilterSlope::Db24,
                FilterSlope::Db36,
                FilterSlope::Db48,
                FilterSlope::Db96,
            ] {
                let mut b = band(kind, 1000.0, 18.0, NEUTRAL_Q);
                b.slope = slope;
                for f in [1.0, 20.0, 1000.0, 20_000.0, 24_000.0] {
                    let db = b.magnitude_db(f, FS);
                    assert!(db.is_finite(), "{kind:?} {slope:?} at {f} Hz gave {db}");
                    assert!(db <= 18.5, "{kind:?} {slope:?} exceeded its gain at {f} Hz: {db}");
                }
            }
        }
    }

    #[test]
    fn a_resonant_cut_stays_finite() {
        let mut b = band(FilterType::LowCut, 1000.0, 0.0, 8.0);
        b.slope = FilterSlope::Db96;
        let db = b.magnitude_db(1000.0, FS);
        assert!(db.is_finite() && db < 160.0, "eight resonant cascades: {db}");
    }

    #[test]
    fn response_samples_the_log_grid() {
        let b = EqBand::bell(1, 1000.0, 6.0, 1.0);
        let curve = b.response(20.0, 20_000.0, 5, FS);
        assert_eq!(curve.len(), 5);
        let peak = curve.iter().copied().fold(f32::MIN, f32::max);
        assert_eq!(curve[2], peak, "the midpoint of the log grid is the bell's peak");
        assert!(curve[2] > 3.0);
        assert!(curve[0].abs() < 0.5);
    }
}
