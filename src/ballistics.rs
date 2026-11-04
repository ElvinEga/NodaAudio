//! Meter movement: how a needle travels between readings.

use crate::db::{SILENCE_DB, db_to_gain, gain_to_db};

/// Time constants for a meter, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ballistics {
    /// Time constant for rising levels. Zero jumps instantly.
    pub attack_ms: f32,
    /// Time constant for falling levels. Zero drops instantly.
    pub release_ms: f32,
    /// How long the peak-hold marker sits before falling. Zero disables it.
    pub peak_hold_ms: f32,
    /// Time constant for the peak-hold marker's fall.
    pub peak_release_ms: f32,
}

impl Ballistics {
    /// No smoothing at all: the meter shows the raw reading.
    pub const INSTANT: Ballistics =
        Ballistics { attack_ms: 0.0, release_ms: 0.0, peak_hold_ms: 0.0, peak_release_ms: 0.0 };

    /// Fast peak meter with a latching hold: the default for digital
    /// meters.
    pub const PEAK: Ballistics = Ballistics {
        attack_ms: 15.0,
        release_ms: 350.0,
        peak_hold_ms: 1200.0,
        peak_release_ms: 600.0,
    };

    /// Slow, needle-like movement, as on a VU bridge.
    pub const VU: Ballistics = Ballistics {
        attack_ms: 300.0,
        release_ms: 300.0,
        peak_hold_ms: 0.0,
        peak_release_ms: 300.0,
    };
}

impl Default for Ballistics {
    fn default() -> Self {
        Self::PEAK
    }
}

/// One smoothed frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeterReading {
    /// The smoothed level, in dBFS.
    pub db: f32,
    /// The peak-hold marker, in dBFS.
    pub hold_db: f32,
}

/// One channel's integrator.
///
/// Smoothing runs on linear amplitude rather than on dB, so a falling
/// level sweeps past the graduations at a steady rate the way a hardware
/// meter does. An integrator that smoothed in dB would crawl through the
/// bottom of the scale.
#[derive(Clone, Copy, Debug)]
pub struct MeterState {
    amplitude: f32,
    hold_amplitude: f32,
    hold_until_ms: f32,
    last_ms: Option<f32>,
}

impl Default for MeterState {
    fn default() -> Self {
        Self { amplitude: 0.0, hold_amplitude: 0.0, hold_until_ms: 0.0, last_ms: None }
    }
}

impl MeterState {
    /// Advance the integrator to `now_ms` with a new peak reading.
    ///
    /// Feed the *raw* block peak in dBFS; the first call adopts it without
    /// smoothing so a meter opens on the current level rather than rising
    /// from silence.
    #[must_use]
    pub fn step(&mut self, input_db: f32, options: Ballistics, now_ms: f32) -> MeterReading {
        let target = db_to_gain(input_db);

        match self.last_ms {
            None => self.amplitude = target,
            Some(last) => {
                let elapsed_ms = (now_ms - last).max(0.0);
                let time_constant =
                    if target > self.amplitude { options.attack_ms } else { options.release_ms };
                self.amplitude += (target - self.amplitude) * smoothing(elapsed_ms, time_constant);
            }
        }

        if options.peak_hold_ms <= 0.0 {
            self.hold_amplitude = self.amplitude;
        } else if self.amplitude >= self.hold_amplitude {
            self.hold_amplitude = self.amplitude;
            self.hold_until_ms = now_ms + options.peak_hold_ms;
        } else if now_ms > self.hold_until_ms
            && let Some(last) = self.last_ms
        {
            // Decay from the moment the hold expired, not from the last
            // frame, so a stalled repaint does not freeze the marker.
            let elapsed_ms = (now_ms - last.max(self.hold_until_ms)).max(0.0);
            self.hold_amplitude -= (self.hold_amplitude - self.amplitude)
                * smoothing(elapsed_ms, options.peak_release_ms);
        }

        self.last_ms = Some(now_ms);
        MeterReading { db: gain_to_db(self.amplitude), hold_db: gain_to_db(self.hold_amplitude) }
    }

    /// Forget the current level and the hold marker.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// True when nothing is moving and the meter is at silence.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amplitude <= f32::EPSILON && self.hold_amplitude <= f32::EPSILON
    }
}

/// The fraction of the remaining distance covered in `elapsed_ms`.
fn smoothing(elapsed_ms: f32, time_constant_ms: f32) -> f32 {
    if time_constant_ms <= 0.0 { 1.0 } else { 1.0 - (-elapsed_ms / time_constant_ms).exp() }
}

/// A level of silence, for a source that has stopped reporting.
pub const IDLE_READING: MeterReading = MeterReading { db: SILENCE_DB, hold_db: SILENCE_DB };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_frame_adopts_the_reading() {
        let mut state = MeterState::default();
        let reading = state.step(-6.0, Ballistics::PEAK, 0.0);
        assert!((reading.db - (-6.0)).abs() < 1e-4);
    }

    #[test]
    fn silence_reads_as_silence() {
        let mut state = MeterState::default();
        let reading = state.step(SILENCE_DB, Ballistics::PEAK, 0.0);
        assert_eq!(reading.db, SILENCE_DB);
    }

    #[test]
    fn instant_ballistics_tracks_the_input() {
        let mut state = MeterState::default();
        let _ = state.step(-20.0, Ballistics::INSTANT, 0.0);
        let reading = state.step(-40.0, Ballistics::INSTANT, 16.0);
        assert!((reading.db - (-40.0)).abs() < 1e-3);
        assert_eq!(reading.hold_db, reading.db);
    }

    #[test]
    fn attack_is_faster_than_release() {
        // The same 16 ms step in both directions, compared in amplitude -
        // the quantity the integrator smooths. A 15 ms attack gets most of
        // the way there; a 350 ms release has barely started.
        let mut rising = MeterState::default();
        let _ = rising.step(-60.0, Ballistics::PEAK, 0.0);
        let risen = db_to_gain(rising.step(0.0, Ballistics::PEAK, 16.0).db);

        let mut falling = MeterState::default();
        let _ = falling.step(0.0, Ballistics::PEAK, 0.0);
        let fallen = db_to_gain(falling.step(-60.0, Ballistics::PEAK, 16.0).db);

        assert!(risen > 0.5, "attack reached amplitude {risen}");
        assert!(fallen > 0.9, "release reached amplitude {fallen}");
        assert!(risen < fallen, "the rise must lead the fall");
    }

    #[test]
    fn the_hold_marker_latches_then_sinks() {
        let mut state = MeterState::default();
        let _ = state.step(-6.0, Ballistics::PEAK, 0.0);
        let _ = state.step(-60.0, Ballistics::PEAK, 100.0);
        // Still inside the hold window: the marker keeps the peak.
        let held = state.step(-60.0, Ballistics::PEAK, 500.0);
        assert!(held.hold_db > -7.0, "hold {held:?} should still read the peak");

        // Past the window, it decays toward the live level.
        let decayed = state.step(-60.0, Ballistics::PEAK, 5000.0);
        assert!(decayed.hold_db < held.hold_db);
        assert!(decayed.hold_db > decayed.db);
    }

    #[test]
    fn a_new_peak_re_arms_the_hold() {
        let mut state = MeterState::default();
        let _ = state.step(-30.0, Ballistics::PEAK, 0.0);
        let _ = state.step(-30.0, Ballistics::PEAK, 5000.0);
        let louder = state.step(-3.0, Ballistics::PEAK, 5100.0);
        assert!(louder.hold_db > -4.0);
        let mut fresh = MeterState::default();
        let _ = fresh.step(-3.0, Ballistics::PEAK, 0.0);
        assert!(!fresh.is_idle());
    }

    #[test]
    fn reset_clears_the_integrator() {
        let mut state = MeterState::default();
        let _ = state.step(-6.0, Ballistics::PEAK, 0.0);
        state.reset();
        assert!(state.is_idle());
        assert_eq!(state.last_ms, None);
    }
}
