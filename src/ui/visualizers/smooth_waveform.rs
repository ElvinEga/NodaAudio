//! A fluid continuous waveform visualizer with wave and oscilloscope modes.
//!
//! Visual language and behavior directly follow audiocn's
//! `smooth-waveform.tsx` and `wave-line.ts`. In `Scope` mode the line is
//! triggered on a rising zero crossing so steady tones stand still. In `Wave`
//! mode four drifting partials follow the frequency bands. Heights ease toward
//! each new shape and the ends fade out.

use std::time::Duration;

use egui::{
    Color32, Pos2, Response, Sense, Ui, Vec2, Widget,
};

use crate::db::clamp;
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::line_paint::faded_line;
use crate::ui::visualizers::wave_line::{step_in_ui, WaveLineMode, WaveLineOptions};

pub use super::wave_line::trigger_index;

/// One point every this many pixels along the line.
const POINT_SPACING_PX: f32 = 3.0;
/// Width of the faded ends, in pixels.
const FADE_PX: f32 = 24.0;

/// Operating mode for the smooth waveform display.
pub type SmoothWaveformMode = WaveLineMode;

/// A smooth continuous waveform visualizer.
///
/// ```ignore
/// let samples = [0.0, 0.5, 0.9, 0.4, -0.2, -0.8, -0.5, 0.0];
/// ui.add(SmoothWaveform::new(&samples).mode(SmoothWaveformMode::Scope));
/// ```
#[must_use = "a SmoothWaveform draws nothing until it is added to a Ui"]
pub struct SmoothWaveform<'a> {
    data: &'a [f32],
    mode: SmoothWaveformMode,
    loading: bool,
    sensitivity: f32,
    line_width: f32,
    fade_edges: bool,
    fade_width: f32,
    time: Option<f32>,
    color: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> SmoothWaveform<'a> {
    /// Create a new smooth waveform widget from audio samples or frequency bands.
    pub fn new(data: &'a [f32]) -> Self {
        Self {
            data,
            mode: SmoothWaveformMode::Wave,
            loading: false,
            sensitivity: 1.0,
            line_width: 2.0,
            fade_edges: true,
            fade_width: FADE_PX,
            time: None,
            color: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Display mode (`Wave` or `Scope`).
    pub fn mode(mut self, mode: SmoothWaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Run a traveling pulse for connecting or loading states.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Visual gain sensitivity multiplier (default 1.0).
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity.max(0.1);
        self
    }

    /// Line stroke width in points (default 2.0).
    pub fn line_width(mut self, line_width: f32) -> Self {
        self.line_width = line_width.max(0.5);
        self
    }

    /// Whether to fade the left and right ends smoothly.
    pub fn fade_edges(mut self, fade: bool) -> Self {
        self.fade_edges = fade;
        self
    }

    /// Width of the end fade zones in points (default 24.0).
    pub fn fade_width(mut self, width: f32) -> Self {
        self.fade_width = width.max(1.0);
        self
    }

    /// Override the animation clock time in seconds.
    pub fn time(mut self, time: f32) -> Self {
        self.time = Some(time);
        self
    }

    /// Line tint color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Explicit widget width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Explicit widget height.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let (default_w, default_h) = match size {
            Size::Small => (160.0, 44.0),
            Size::Medium => (240.0, 64.0),
            Size::Large => (320.0, 84.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }
}

impl Widget for SmoothWaveform<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let now_ms = self
            .time
            .map_or_else(|| ui.input(|i| i.time), f64::from)
            * 1000.0;

        let count = ((rect.width() / POINT_SPACING_PX).round() as usize + 1)
            .clamp(2, super::wave_line::WAVE_LINE_MAX_POINTS);
        let options = WaveLineOptions {
            mode: self.mode,
            loading: self.loading,
            sensitivity: self.sensitivity,
            reduced_motion: false,
        };
        let frame = step_in_ui(
            ui,
            response.id.with("wave_line"),
            options,
            now_ms,
            Some(self.data),
            count,
        );

        if !frame.active || frame.loudness > 0.0005 {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        let painter = ui.painter();
        let width = rect.width();
        let height = rect.height();
        let line_width = clamp(self.line_width, 0.5, height * 0.5);
        let middle = rect.center().y;
        let reach = (height * 0.5 - line_width).max(0.0);
        let points_in_use = frame.heights.len();
        let points: Vec<Pos2> = frame
            .heights
            .iter()
            .enumerate()
            .map(|(index, height)| {
                let x = rect.min.x + index as f32 / (points_in_use - 1) as f32 * width;
                Pos2::new(x, middle - height * reach)
            })
            .collect();

        let color = self.color.unwrap_or(theme.palette.primary);
        let fade = if self.fade_edges {
            self.fade_width.min(width * 0.5)
        } else {
            0.0
        };
        faded_line(painter, &points, line_width, color, fade, 1.0);

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_index_detects_rising_zero_crossing() {
        let samples = [-0.8, -0.4, 0.2, 0.7, 0.3, -0.2, -0.6, -0.9, 0.1, 0.5, 0.2, -0.1];
        let trig = trigger_index(&samples);
        assert!(trig > 0.0 && trig < 2.0);
    }

    #[test]
    fn smooth_waveform_renders_scope_and_wave() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let samples = [0.0, 0.4, 0.8, -0.2, -0.6, 0.0];
            let resp1 = ui.add(SmoothWaveform::new(&samples).mode(SmoothWaveformMode::Wave));
            let resp2 = ui.add(SmoothWaveform::new(&samples).mode(SmoothWaveformMode::Scope));
            assert!(resp1.rect.width() > 0.0);
            assert!(resp2.rect.width() > 0.0);
        });
    }
}
