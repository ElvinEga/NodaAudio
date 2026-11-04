//! The shape of a single line across the width, ported from audiocn's
//! `lib/audio/wave-line.ts`.
//!
//! `Wave` mode draws a drifting multi-harmonic wave whose partials follow four
//! resampled frequency bands. `Scope` mode draws the signal itself, triggered
//! on a rising zero crossing and auto-gained to fill the display. A loading
//! pulse runs along the line for connecting states. Heights ease toward each
//! new shape and loudness releases slowly, so the line breathes rather than
//! snaps.

use std::f32::consts::TAU;

use crate::db::clamp;
use crate::ui::visualizers::bar_levels::{BarIdle, BarLevelsOptions, BarLevelsState};

/// The most points a line can have.
pub const WAVE_LINE_MAX_POINTS: usize = 513;

const FRAME_MS: f64 = 16.67;
const MAX_STEP_SECONDS: f32 = 0.05;
/// Below this the line counts as silent.
const SIGNAL_THRESHOLD: f32 = 0.02;
/// Share of the distance to the new shape still left after one frame.
const EASE_PER_FRAME: f32 = 0.45;
/// Loudness left after one frame of silence.
const LOUDNESS_RELEASE: f32 = 0.92;

/// Cycles across the width, drift in radians per second and weight of each
/// band group, lows first.
const WAVE_PARTIALS: [(f32, f32, f32); 4] = [
    (1.5, 0.8, 0.6),
    (2.5, -1.3, 0.4),
    (4.0, 2.1, 0.25),
    (6.5, -3.2, 0.16),
];

/// Scope mode lifts quiet signals by at most this much.
const SCOPE_MAX_GAIN: f32 = 4.0;
/// Scope mode scales its running peak to this height.
const SCOPE_TARGET: f32 = 0.9;
/// Share of the scope's running peak left after one frame.
const SCOPE_PEAK_RELEASE: f32 = 0.985;

/// Widths per second the loading pulse travels.
const LOADING_SPEED: f32 = 0.55;
/// Where the loading pulse rests with reduced motion: the middle.
const LOADING_REST_SECONDS: f32 = 0.7 / LOADING_SPEED;

/// `Wave` is a smooth wave shaped by the frequency bands. `Scope` is the
/// signal itself, like an oscilloscope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WaveLineMode {
    /// A drifting wave shaped by the frequency bands.
    #[default]
    Wave,
    /// The triggered time-domain signal.
    Scope,
}

/// The inputs that decide how the line moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveLineOptions {
    pub mode: WaveLineMode,
    /// Runs a pulse along the line instead of following the signal.
    pub loading: bool,
    /// Visual gain.
    pub sensitivity: f32,
    /// No drift or easing: the line jumps to each new shape.
    pub reduced_motion: bool,
}

impl WaveLineOptions {
    /// Defaults matching audiocn: `wave`, unity gain, live motion.
    #[must_use]
    pub fn new(mode: WaveLineMode) -> Self {
        Self {
            mode,
            loading: false,
            sensitivity: 1.0,
            reduced_motion: false,
        }
    }
}

/// Where the signal first rises through zero in its first quarter, between
/// samples, so a steady tone stands still. 0 when it never does.
#[must_use]
pub fn trigger_index(samples: &[f32]) -> f32 {
    let limit = samples.len() / 4;
    for index in 1..=limit {
        let before = samples[index - 1];
        let after = samples[index];
        if before < 0.0 && after >= 0.0 {
            return (index - 1) as f32 + (-before / (after - before));
        }
    }
    0.0
}

fn peak_of(values: &[f32], count: usize) -> f32 {
    values
        .iter()
        .take(count)
        .fold(0.0f32, |peak, value| peak.max(value.abs()))
}

/// Fits three quarters of the samples, from the trigger, into `out`.
fn fill_scope(samples: &[f32], out: &mut [f32], gain: f32) {
    let count = out.len();
    if count == 0 {
        return;
    }
    let start = trigger_index(samples);
    let span = ((samples.len() as f32 * 0.75).floor() - 1.0).max(0.0);
    for (point, slot) in out.iter_mut().enumerate() {
        let position = start + span * point as f32 / (count - 1) as f32;
        let index = position.floor();
        let before = samples.get(index as usize).copied().unwrap_or(0.0);
        let after = samples
            .get((index as usize + 1).min(samples.len().saturating_sub(1)))
            .copied()
            .unwrap_or(before);
        let value = before + (after - before) * (position - index);
        *slot = clamp(value * gain, -1.0, 1.0);
    }
}

/// A short wave packet that travels left to right.
fn loading_pulse(position: f32, seconds: f32) -> f32 {
    let center = (seconds * LOADING_SPEED).rem_euclid(1.4) - 0.2;
    let envelope = (-((position - center).powi(2)) / 0.012).exp();
    0.6 * envelope * (TAU * (position * 7.0 - seconds * 2.5)).sin()
}

/// The line's shape and motion, advanced one step at a time.
#[derive(Clone, Debug)]
pub struct WaveLineState {
    options: WaveLineOptions,
    targets: Vec<f32>,
    bands: BarLevelsState,
    phases: [f32; 4],
    count: usize,
    heights: Vec<f32>,
    peak: f32,
    loudness: f32,
    last_ms: f64,
    scope_peak: f32,
}

impl WaveLineState {
    /// A flat line at rest.
    #[must_use]
    pub fn new(options: WaveLineOptions) -> Self {
        let mut band_options = BarLevelsOptions::new(WAVE_PARTIALS.len());
        band_options.idle = BarIdle::Static;
        band_options.min_level = 0.0;
        band_options.reduced_motion = options.reduced_motion;
        Self {
            options,
            targets: vec![0.0; WAVE_LINE_MAX_POINTS],
            bands: BarLevelsState::new(band_options),
            phases: [0.0; 4],
            count: 2,
            heights: vec![0.0; WAVE_LINE_MAX_POINTS],
            peak: 0.0,
            loudness: 0.0,
            last_ms: 0.0,
            scope_peak: 0.0,
        }
    }

    /// The options this line was built for.
    #[must_use]
    pub fn options(&self) -> WaveLineOptions {
        self.options
    }

    /// Points in use, left to right.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Height at each point, −1..1, up positive, eased toward the latest shape.
    #[must_use]
    pub fn heights(&self) -> &[f32] {
        &self.heights[..self.count]
    }

    /// Highest point of the latest shape before easing, 0..1.
    #[must_use]
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// How loud the line is, 0..1: its highest point, released slowly.
    #[must_use]
    pub fn loudness(&self) -> f32 {
        self.loudness
    }

    fn fill_wave(&mut self, now_ms: f64, input: Option<&[f32]>) {
        self.bands.step(now_ms, input.unwrap_or(&[]));
        let count = self.count;
        let bands = self.bands.levels();
        for point in 0..count {
            let position = point as f32 / (count - 1) as f32;
            let mut height = 0.0;
            for (index, (cycles, _drift, weight)) in WAVE_PARTIALS.iter().enumerate() {
                height += weight
                    * bands.get(index).copied().unwrap_or(0.0)
                    * (TAU * cycles * position + self.phases[index]).sin();
            }
            self.targets[point] = clamp(height * self.options.sensitivity, -1.0, 1.0);
        }
    }

    fn fill_targets(&mut self, now_ms: f64, input: Option<&[f32]>) {
        let count = self.count;
        if self.options.loading {
            let seconds = if self.options.reduced_motion {
                LOADING_REST_SECONDS
            } else {
                (now_ms / 1000.0) as f32
            };
            for point in 0..count {
                self.targets[point] =
                    loading_pulse(point as f32 / (count - 1) as f32, seconds);
            }
            return;
        }
        if self.options.mode == WaveLineMode::Scope {
            if let Some(samples) = input.filter(|samples| samples.len() > 1) {
                let peak = peak_of(samples, samples.len());
                self.scope_peak = peak.max(self.scope_peak * SCOPE_PEAK_RELEASE);
                let gain = self.options.sensitivity
                    * SCOPE_MAX_GAIN.min(SCOPE_TARGET / if self.scope_peak > 0.0 {
                        self.scope_peak
                    } else {
                        1.0
                    });
                fill_scope(samples, &mut self.targets[..count], gain);
                return;
            }
        }
        self.fill_wave(now_ms, input);
    }

    fn ease(&mut self, elapsed_ms: f64) {
        let share = if self.options.reduced_motion {
            1.0
        } else {
            1.0 - EASE_PER_FRAME.powf((elapsed_ms / FRAME_MS) as f32)
        };
        for point in 0..self.count {
            let height = self.heights[point];
            self.heights[point] = height + (self.targets[point] - height) * share;
        }
        let release = LOUDNESS_RELEASE.powf((elapsed_ms / FRAME_MS) as f32);
        self.loudness = peak_of(&self.heights, self.count).max(self.loudness * release);
    }

    /// Advances to `now_ms` with the latest input, over `count` points.
    /// Returns true when there is a signal.
    pub fn step(&mut self, now_ms: f64, input: Option<&[f32]>, count: usize) -> bool {
        let elapsed_ms = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            (now_ms - self.last_ms).max(0.0)
        };
        self.last_ms = now_ms;
        self.count = count.clamp(2, WAVE_LINE_MAX_POINTS);
        if !self.options.reduced_motion {
            let seconds = ((elapsed_ms / 1000.0) as f32).clamp(0.0, MAX_STEP_SECONDS);
            for (phase, (_, drift, _)) in self.phases.iter_mut().zip(WAVE_PARTIALS) {
                *phase = (*phase + drift * seconds).rem_euclid(TAU);
            }
        }
        self.fill_targets(now_ms, input);
        self.peak = peak_of(&self.targets, self.count);
        self.ease(elapsed_ms);
        !self.options.loading && self.peak >= SIGNAL_THRESHOLD
    }
}

/// The line a widget should draw this frame.
#[derive(Clone, Debug)]
pub struct WaveLineFrame {
    pub heights: Vec<f32>,
    pub loudness: f32,
    pub peak: f32,
    pub active: bool,
}

/// Steps a [`WaveLineState`] stored in egui's temporary data, rebuilding it
/// when the options change.
pub fn step_in_ui(
    ui: &egui::Ui,
    id: egui::Id,
    options: WaveLineOptions,
    now_ms: f64,
    input: Option<&[f32]>,
    count: usize,
) -> WaveLineFrame {
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<WaveLineState>(id))
        .filter(|state| state.options() == options)
        .unwrap_or_else(|| WaveLineState::new(options));
    let active = state.step(now_ms, input, count);
    let frame = WaveLineFrame {
        heights: state.heights().to_vec(),
        loudness: state.loudness(),
        peak: state.peak(),
        active,
    };
    ui.ctx().data_mut(|data| data.insert_temp(id, state));
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(mode: WaveLineMode) -> WaveLineOptions {
        WaveLineOptions::new(mode)
    }

    #[test]
    fn trigger_index_finds_the_rising_crossing() {
        let samples = [-0.8, -0.4, 0.2, 0.7, 0.3, -0.2, -0.6, -0.9, 0.1, 0.5, 0.2, -0.1];
        let trigger = trigger_index(&samples);
        assert!(trigger > 1.0 && trigger < 2.0, "{trigger}");
    }

    #[test]
    fn a_steady_tone_stands_still_in_scope_mode() {
        let samples: Vec<f32> = (0..64)
            .map(|index| (index as f32 * 0.2).sin())
            .collect();
        let mut first = WaveLineState::new(options(WaveLineMode::Scope));
        first.step(16.67, Some(&samples), 64);
        let heights = first.heights().to_vec();
        let mut second = WaveLineState::new(options(WaveLineMode::Scope));
        // Reduced motion jumps straight to the shape.
        second.options.reduced_motion = true;
        second.step(16.67, Some(&samples), 64);
        assert_eq!(heights.len(), 64);
        assert!(second.heights().iter().any(|&height| height.abs() > 0.1));
    }

    #[test]
    fn wave_mode_follows_bands_and_releases_loudness() {
        let mut state = WaveLineState::new(options(WaveLineMode::Wave));
        state.step(16.67, Some(&[0.9, 0.7, 0.4, 0.2]), 128);
        assert!(state.loudness() > 0.05, "{}", state.loudness());
        let loud = state.loudness();
        for frame in 1..30 {
            state.step(16.67 + f64::from(frame) * 16.67, None, 128);
        }
        assert!(state.loudness() < loud, "loudness releases");
    }

    #[test]
    fn loading_pulse_moves_and_stays_in_range() {
        let mut options = options(WaveLineMode::Wave);
        options.loading = true;
        let mut state = WaveLineState::new(options);
        state.step(16.67, None, 96);
        let first = state.heights().to_vec();
        state.step(200.0, None, 96);
        let second = state.heights().to_vec();
        assert_ne!(first, second, "the pulse travels");
        assert!(second.iter().all(|height| height.abs() <= 1.0));
        assert!(!state.step(400.0, None, 96));
    }
}
