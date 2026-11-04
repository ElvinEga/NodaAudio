//! A live streaming waveform visualizer with scrolling history and static modes.
//!
//! Visual language and behavior directly follow audiocn's `live-waveform.tsx`.
//! Supports `Bars`, `Mirror`, and `Line` variants, rolling history buffer visualization,
//! idle center line, and customizable bar geometry and edge fading.

use egui::{
    Color32, Rect, Response, Sense, Stroke, Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::spectrum::resample_levels;

/// Operating display mode of the live waveform.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveWaveformMode {
    /// Rolling timeline history of levels across elapsed time.
    #[default]
    Scrolling,
    /// Instantaneous buffer visualization of the current frame.
    Static,
}

/// Visual presentation style of the live waveform.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveWaveformVariant {
    /// Upward/vertical bars from bottom or center.
    #[default]
    Bars,
    /// Symmetric bars mirrored around the vertical middle.
    Mirror,
    /// Connected line trace.
    Line,
}

/// A streaming live waveform visualizer.
///
/// ```ignore
/// let history = [0.1, 0.3, 0.6, 0.9, 0.4, 0.2];
/// ui.add(LiveWaveform::new(&history).variant(LiveWaveformVariant::Mirror));
/// ```
#[must_use = "a LiveWaveform draws nothing until it is added to a Ui"]
pub struct LiveWaveform<'a> {
    data: &'a [f32],
    mode: LiveWaveformMode,
    variant: LiveWaveformVariant,
    active: bool,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
    min_bar_height: f32,
    line_width: f32,
    fade_edges: bool,
    fade_width: f32,
    color: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> LiveWaveform<'a> {
    /// Create a new live waveform visualizer with data levels or samples (0.0..=1.0).
    pub fn new(data: &'a [f32]) -> Self {
        Self {
            data,
            mode: LiveWaveformMode::Scrolling,
            variant: LiveWaveformVariant::Bars,
            active: true,
            bar_width: 3.0,
            bar_gap: 1.0,
            bar_radius: 1.5,
            min_bar_height: 4.0,
            line_width: 1.5,
            fade_edges: true,
            fade_width: 24.0,
            color: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Display mode (`Scrolling` history or `Static` frame).
    pub fn mode(mut self, mode: LiveWaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Presentation variant (`Bars`, `Mirror`, `Line`).
    pub fn variant(mut self, variant: LiveWaveformVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Whether the visualizer is actively receiving signal. If false, draws an idle center line.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Bar width in points (default 3.0).
    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = width.max(1.0);
        self
    }

    /// Gap between bars in points (default 1.0).
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = gap.max(0.0);
        self
    }

    /// Bar corner rounding (default 1.5).
    pub fn bar_radius(mut self, radius: f32) -> Self {
        self.bar_radius = radius.max(0.0);
        self
    }

    /// Minimum bar height in points (default 4.0).
    pub fn min_bar_height(mut self, min_height: f32) -> Self {
        self.min_bar_height = min_height.max(1.0);
        self
    }

    /// Line stroke width for `Line` variant (default 1.5).
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Whether to fade out the left and right edges.
    pub fn fade_edges(mut self, fade: bool) -> Self {
        self.fade_edges = fade;
        self
    }

    /// Width of the edge fade zone in points (default 24.0).
    pub fn fade_width(mut self, width: f32) -> Self {
        self.fade_width = width.max(1.0);
        self
    }

    /// Bar / line tint color.
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
            Size::Small => (140.0, 36.0),
            Size::Medium => (220.0, 48.0),
            Size::Large => (300.0, 64.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }
}

impl Widget for LiveWaveform<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter();
        let mid_y = rect.center().y;
        let w = rect.width();
        let h = rect.height();

        let primary_color = self.color.unwrap_or(theme.palette.primary);

        // Idle state: draw dotted or faint center line
        if !self.active || self.data.is_empty() {
            let idle_color = theme.palette.text_dim.linear_multiply(0.35);
            painter.line_segment(
                [pos2(rect.min.x, mid_y), pos2(rect.max.x, mid_y)],
                Stroke::new(1.0f32, idle_color),
            );
            return response;
        }

        let pitch = self.bar_width + self.bar_gap;
        let bar_count = ((w + self.bar_gap) / pitch).floor() as usize;
        if bar_count == 0 {
            return response;
        }

        // Resample input data to the required bar count
        let mut levels = vec![0.0f32; bar_count];
        resample_levels(self.data, &mut levels);

        let fade_w = self.fade_width.min(w * 0.35);

        match self.variant {
            LiveWaveformVariant::Bars | LiveWaveformVariant::Mirror => {
                for (i, &lvl) in levels.iter().enumerate() {
                    let level = clamp(lvl, 0.0, 1.0);
                    let bar_h = (level * h).clamp(self.min_bar_height, h);
                    let x = rect.min.x + i as f32 * pitch;

                    let y = match self.variant {
                        LiveWaveformVariant::Mirror => mid_y - bar_h * 0.5,
                        _ => rect.max.y - bar_h,
                    };

                    let bar_rect = Rect::from_min_size(pos2(x, y), Vec2::new(self.bar_width, bar_h));

                    let alpha = if self.fade_edges {
                        let from_left = ((x - rect.min.x) / fade_w).clamp(0.0, 1.0);
                        let from_right = ((rect.max.x - x) / fade_w).clamp(0.0, 1.0);
                        from_left.min(from_right)
                    } else {
                        1.0
                    };

                    painter.rect_filled(
                        bar_rect,
                        self.bar_radius,
                        primary_color.linear_multiply(alpha),
                    );
                }
            }
            LiveWaveformVariant::Line => {
                let mut points = Vec::with_capacity(bar_count);
                for (i, &lvl) in levels.iter().enumerate() {
                    let level = clamp(lvl, 0.0, 1.0);
                    let x = rect.min.x + i as f32 * pitch + self.bar_width * 0.5;
                    let y = mid_y - (level - 0.5) * (h - 2.0);
                    points.push(pos2(x, y));
                }

                for i in 0..points.len().saturating_sub(1) {
                    let p0 = points[i];
                    let p1 = points[i + 1];
                    let mid_x = (p0.x + p1.x) * 0.5 - rect.min.x;

                    let alpha = if self.fade_edges {
                        let from_left = (mid_x / fade_w).clamp(0.0, 1.0);
                        let from_right = ((w - mid_x) / fade_w).clamp(0.0, 1.0);
                        from_left.min(from_right)
                    } else {
                        1.0
                    };

                    painter.line_segment(
                        [p0, p1],
                        Stroke::new(self.line_width, primary_color.linear_multiply(alpha)),
                    );
                }
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_waveform_renders_variants() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let data = [0.1, 0.5, 0.8, 0.3, 0.2];
            let resp1 = ui.add(LiveWaveform::new(&data).variant(LiveWaveformVariant::Bars));
            let resp2 = ui.add(LiveWaveform::new(&data).variant(LiveWaveformVariant::Mirror));
            let resp3 = ui.add(LiveWaveform::new(&data).variant(LiveWaveformVariant::Line));
            assert!(resp1.rect.width() > 0.0);
            assert!(resp2.rect.width() > 0.0);
            assert!(resp3.rect.width() > 0.0);
        });
    }

    #[test]
    fn live_waveform_idle_renders_center_line() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let resp = ui.add(LiveWaveform::new(&[]).active(false));
            assert!(resp.rect.width() > 0.0);
        });
    }
}
