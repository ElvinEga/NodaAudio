//! Bar levels shared by the bar visualizers, ported from audiocn's
//! `lib/audio/bar-levels.ts`.
//!
//! Input is resampled to the bar count, attack is instant and release is
//! smoothed, mirroring folds the bands around the middle bar, idle motion and
//! the loading sweep are added on top, and the bars report when they have
//! nothing left to animate. The state lives in egui's temporary data so a
//! widget can be constructed afresh every frame and still keep its ballistics.

use egui::Id;

use crate::db::clamp;
use crate::ui::visualizers::spectrum::resample_levels;

/// Share of the distance to a quieter target still left after one frame.
pub const RELEASE_PER_FRAME: f32 = 0.86;
/// The frame the release constant is calibrated against.
pub const FRAME_MS: f64 = 16.67;
/// Below this the input counts as silent.
pub const SIGNAL_THRESHOLD: f32 = 0.02;
/// Bars this close to their target have nothing left to animate.
pub const SETTLE_EPSILON: f32 = 0.002;

/// What bars do with no signal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BarIdle {
    /// Bars rest at `min_level` without animation.
    #[default]
    Static,
    /// Gentle breathing pulse across all bars.
    Pulse,
    /// Traveling sinusoidal wave rippling across bars.
    Wave,
}

/// The inputs that decide how levels move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarLevelsOptions {
    pub bar_count: usize,
    pub idle: BarIdle,
    /// Runs a sweep, for connecting or thinking states.
    pub loading: bool,
    /// Resting bar size, 0..1.
    pub min_level: f32,
    /// Symmetric around the middle bar, lows in the centre.
    pub mirrored: bool,
    /// No release smoothing, idle animation or sweep.
    pub reduced_motion: bool,
}

impl BarLevelsOptions {
    /// Options for a bar count, with the audiocn defaults.
    #[must_use]
    pub fn new(bar_count: usize) -> Self {
        Self {
            bar_count,
            idle: BarIdle::Static,
            loading: false,
            min_level: 0.08,
            mirrored: false,
            reduced_motion: false,
        }
    }
}

/// The idle level a bar adds on top of its resting size.
#[must_use]
pub fn idle_level(idle: BarIdle, index: usize, seconds: f32) -> f32 {
    match idle {
        BarIdle::Static => 0.0,
        BarIdle::Pulse => 0.1 * (0.5 + 0.5 * (seconds * std::f32::consts::PI * 1.6).sin()),
        BarIdle::Wave => {
            0.16 * (0.5 + 0.5 * (seconds * 4.0 - index as f32 * 0.55).sin())
        }
    }
}

/// The Gaussian blob the loading sweep moves along the bars.
#[must_use]
pub fn sweep_level(index: usize, count: usize, seconds: f32) -> f32 {
    let span = (count + 6) as f32;
    let position = ((seconds * count as f32 * 0.9).rem_euclid(span)) - 3.0;
    let distance = index as f32 - position;
    0.55 * (-((distance * distance) / 3.0)).exp()
}

/// Which resampled band a bar shows: the bar itself, or its distance from the
/// middle when mirrored.
#[must_use]
pub fn target_index_for(index: usize, count: usize, half: usize, mirrored: bool) -> usize {
    if !mirrored {
        return index;
    }
    let center = (count - 1) as f32 * 0.5;
    ((index as f32 - center).abs().floor() as usize).min(half.saturating_sub(1))
}

/// The smoothed level of every bar, advanced one step at a time.
#[derive(Clone, Debug)]
pub struct BarLevelsState {
    options: BarLevelsOptions,
    half: usize,
    targets: Vec<f32>,
    current: Vec<f32>,
    levels: Vec<f32>,
    last_ms: f64,
    settled: bool,
}

impl BarLevelsState {
    /// A fresh set of bars resting at `min_level`.
    #[must_use]
    pub fn new(mut options: BarLevelsOptions) -> Self {
        options.min_level = clamp(options.min_level, 0.0, 1.0);
        let bar_count = options.bar_count;
        let half = if options.mirrored {
            bar_count.div_ceil(2)
        } else {
            bar_count
        };
        let min_level = options.min_level;
        Self {
            options,
            half,
            targets: vec![0.0; half],
            current: vec![0.0; bar_count],
            levels: vec![min_level; bar_count],
            last_ms: 0.0,
            settled: false,
        }
    }

    /// The options this state was built for.
    #[must_use]
    pub fn options(&self) -> BarLevelsOptions {
        self.options
    }

    /// The level each bar shows, 0..1, never below `min_level`.
    #[must_use]
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// True after a step that left nothing to animate.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.settled
    }

    /// Advances to `now_ms` with the latest input levels (resampled to the bar
    /// count). Returns true when the input is above the signal floor.
    pub fn step(&mut self, now_ms: f64, input: &[f32]) -> bool {
        let count = self.options.bar_count;
        if count == 0 {
            self.settled = true;
            return false;
        }

        let elapsed = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            (now_ms - self.last_ms).max(0.0)
        };
        self.last_ms = now_ms;
        let release = if self.options.reduced_motion {
            0.0
        } else {
            RELEASE_PER_FRAME.powf((elapsed / FRAME_MS) as f32)
        };
        let seconds = (now_ms / 1000.0) as f32;

        if input.is_empty() {
            self.targets.fill(0.0);
        } else {
            resample_levels(input, &mut self.targets);
        }

        let active = self
            .targets
            .iter()
            .copied()
            .fold(0.0f32, f32::max)
            >= SIGNAL_THRESHOLD;
        let quiet = !(self.options.reduced_motion || active);
        let animating = (self.options.loading && !self.options.reduced_motion)
            || (quiet && self.options.idle != BarIdle::Static);
        let mut converged = true;

        for index in 0..count {
            let target_index = target_index_for(
                index,
                count,
                self.half,
                self.options.mirrored,
            );
            let audio = self.targets.get(target_index).copied().unwrap_or(0.0);
            let extra = if self.options.loading && !self.options.reduced_motion {
                sweep_level(index, count, seconds)
            } else if quiet {
                idle_level(self.options.idle, index, seconds)
            } else {
                0.0
            };
            let target = audio.max(extra);
            let previous = self.current[index];
            let next = if target >= previous || self.options.reduced_motion {
                target
            } else {
                previous * release + target * (1.0 - release)
            };
            self.current[index] = next;
            self.levels[index] = clamp(next.max(self.options.min_level), 0.0, 1.0);
            if (next - target).abs() > SETTLE_EPSILON {
                converged = false;
            }
        }

        self.settled = converged && !animating;
        active
    }
}

/// The levels a widget should draw this frame.
#[derive(Clone, Debug)]
pub struct BarLevelsFrame {
    /// Level per bar, already smoothed and clamped.
    pub levels: Vec<f32>,
    /// The input is above the signal floor.
    pub active: bool,
    /// Nothing is left to animate.
    pub settled: bool,
}

/// Steps a [`BarLevelsState`] stored in egui's temporary data, rebuilding it
/// when the options change.
pub fn step_in_ui(
    ui: &egui::Ui,
    id: Id,
    options: BarLevelsOptions,
    now_ms: f64,
    input: &[f32],
) -> BarLevelsFrame {
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<BarLevelsState>(id))
        .filter(|state| state.options() == options)
        .unwrap_or_else(|| BarLevelsState::new(options));
    let active = state.step(now_ms, input);
    let frame = BarLevelsFrame {
        levels: state.levels().to_vec(),
        active,
        settled: state.settled(),
    };
    ui.ctx().data_mut(|data| data.insert_temp(id, state));
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(bar_count: usize) -> BarLevelsOptions {
        BarLevelsOptions::new(bar_count)
    }

    #[test]
    fn release_falls_smoothly_and_attacks_instantly() {
        let mut state = BarLevelsState::new(options(4));
        state.step(16.67, &[0.8, 0.6, 0.4, 0.2]);
        assert_eq!(state.levels(), &[0.8, 0.6, 0.4, 0.2]);
        state.step(33.34, &[0.0, 0.0, 0.0, 0.0]);
        assert!((state.levels()[0] - 0.8 * 0.86).abs() < 1e-5, "{:?}", state.levels());
        assert!(state.levels()[0] > 0.0, "still falling, not snapped");
    }

    #[test]
    fn mirroring_folds_bands_around_the_middle() {
        let mut options = options(5);
        options.mirrored = true;
        let mut state = BarLevelsState::new(options);
        state.step(16.67, &[0.1, 0.2, 0.9]);
        // 5 bars mirror to 3 samples; lows sit in the centre and the
        // outermost bars share the brightest band.
        assert!((state.levels()[0] - 0.9).abs() < 1e-6);
        assert!((state.levels()[2] - 0.1).abs() < 1e-6);
        assert!((state.levels()[4] - 0.9).abs() < 1e-6);
    }

    #[test]
    fn a_sweep_animates_without_input_and_keeps_the_smallest_bar_resting() {
        let mut options = options(24);
        options.loading = true;
        let mut state = BarLevelsState::new(options);
        state.step(500.0, &[]);
        let loudest = state
            .levels()
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        assert!(loudest > 0.08);
        assert!(!state.settled());
        assert!(state.levels().iter().all(|&level| level >= 0.08));
    }

    #[test]
    fn quiet_static_bars_settle() {
        let mut state = BarLevelsState::new(options(8));
        state.step(16.67, &[]);
        assert!(state.settled());
        assert!(state.levels().iter().all(|&level| (level - 0.08).abs() < 1e-6));
    }
}
