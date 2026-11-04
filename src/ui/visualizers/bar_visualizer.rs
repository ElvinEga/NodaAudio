//! A dynamic multi-bar audio visualizer with idle animations, mirroring, and sweeps.
//!
//! Visual language and behavior directly follow audiocn's `bar-visualizer.tsx`
//! and `bar-levels.ts`. Bars share their ballistics through
//! [`BarLevelsState`](super::bar_levels::BarLevelsState): instant attack, a
//! smooth release, pulse and wave idle states, and a Gaussian loading sweep.

use std::time::Duration;

use egui::{
    Color32, Rect, Response, Sense, Ui, Vec2, Widget, pos2,
};

use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::bar_levels::{step_in_ui, BarLevelsOptions};

pub use super::bar_levels::BarIdle;

/// Gap between bars, in points, matching audiocn's `--bar-gap: 0.1875rem`.
const BAR_GAP: f32 = 3.0;
/// The widest a bar may get, matching audiocn's `--bar-width: 0.375rem`.
const BAR_MAX_WIDTH: f32 = 6.0;

/// Alignment of bars within their vertical container.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BarAlign {
    /// Bars expand outward from vertical center (audiocn default).
    #[default]
    Center,
    /// Bars grow downward from the top edge.
    Start,
    /// Bars grow upward from the bottom edge.
    End,
}

/// A multi-bar audio visualizer.
///
/// ```ignore
/// let levels = [0.1, 0.4, 0.8, 0.5, 0.2];
/// ui.add(BarVisualizer::new(&levels).mirrored(true));
/// ```
#[must_use = "a BarVisualizer draws nothing until it is added to a Ui"]
pub struct BarVisualizer<'a> {
    levels: &'a [f32],
    bar_count: Option<usize>,
    align: BarAlign,
    mirrored: bool,
    min_level: f32,
    idle: BarIdle,
    loading: bool,
    time: Option<f32>,
    color: Option<Color32>,
    bar_width: Option<f32>,
    bar_gap: Option<f32>,
    bar_radius: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> BarVisualizer<'a> {
    /// Create a new bar visualizer with input levels (0.0..=1.0).
    pub fn new(levels: &'a [f32]) -> Self {
        Self {
            levels,
            bar_count: None,
            align: BarAlign::Center,
            mirrored: false,
            min_level: 0.08,
            idle: BarIdle::Static,
            loading: false,
            time: None,
            color: None,
            bar_width: None,
            bar_gap: None,
            bar_radius: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Number of bars to display (resamples input to fit).
    pub fn bar_count(mut self, count: usize) -> Self {
        self.bar_count = Some(count);
        self
    }

    /// Vertical alignment (`Center`, `Start`, `End`).
    pub fn align(mut self, align: BarAlign) -> Self {
        self.align = align;
        self
    }

    /// Symmetric around the middle bar (low frequencies in center).
    pub fn mirrored(mut self, mirrored: bool) -> Self {
        self.mirrored = mirrored;
        self
    }

    /// Minimum resting bar level (0.0..=1.0, default 0.08).
    pub fn min_level(mut self, min_level: f32) -> Self {
        self.min_level = min_level.clamp(0.0, 1.0);
        self
    }

    /// Idle state behavior when audio is quiet (`Static`, `Pulse`, `Wave`).
    pub fn idle(mut self, idle: BarIdle) -> Self {
        self.idle = idle;
        self
    }

    /// Run a Gaussian sweep animation for connecting / loading states.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Override the animation clock time in seconds.
    pub fn time(mut self, time: f32) -> Self {
        self.time = Some(time);
        self
    }

    /// Custom bar tint color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Explicit bar width in points.
    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = Some(width);
        self
    }

    /// Explicit gap between bars in points.
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = Some(gap);
        self
    }

    /// Explicit corner radius for bars.
    pub fn bar_radius(mut self, radius: f32) -> Self {
        self.bar_radius = Some(radius);
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
            Size::Medium => (220.0, 52.0),
            Size::Large => (300.0, 72.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }

}

impl Widget for BarVisualizer<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let time_secs = self.time.unwrap_or_else(|| ui.input(|i| i.time) as f32);
        let count = self.bar_count.unwrap_or_else(|| {
            if !self.levels.is_empty() {
                self.levels.len()
            } else {
                24
            }
        });

        if count == 0 {
            return response;
        }

        let options = BarLevelsOptions {
            bar_count: count,
            idle: self.idle,
            loading: self.loading,
            min_level: self.min_level,
            mirrored: self.mirrored,
            reduced_motion: false,
        };
        let frame = step_in_ui(
            ui,
            response.id.with("bar_levels"),
            options,
            f64::from(time_secs) * 1000.0,
            self.levels,
        );

        if !frame.settled {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        let painter = ui.painter();
        let bar_color = self.color.unwrap_or(theme.palette.primary);

        // audiocn lays the bars out as flex items: a fixed gap, a max width,
        // and the run centred when there is room to spare.
        let gap = self.bar_gap.unwrap_or(BAR_GAP);
        let available = (rect.width() - gap * (count.saturating_sub(1) as f32)) / count as f32;
        let bar_w = self
            .bar_width
            .unwrap_or_else(|| available.min(BAR_MAX_WIDTH).max(0.5));
        let run = count as f32 * bar_w + gap * count.saturating_sub(1) as f32;
        let first_x = rect.center().x - run * 0.5;
        let bar_radius = self.bar_radius.unwrap_or(bar_w * 0.5);

        let total_h = rect.height();

        for (i, &lvl) in frame.levels.iter().enumerate() {
            let bar_h = (lvl * total_h).clamp(2.0, total_h);
            let bar_x = first_x + i as f32 * (bar_w + gap);

            let bar_y = match self.align {
                BarAlign::Center => rect.min.y + (total_h - bar_h) * 0.5,
                BarAlign::Start => rect.min.y,
                BarAlign::End => rect.max.y - bar_h,
            };

            let bar_rect = Rect::from_min_size(pos2(bar_x, bar_y), Vec2::new(bar_w, bar_h));
            painter.rect_filled(bar_rect, bar_radius, bar_color);
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_visualizer_renders_headlessly() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let levels = [0.1, 0.4, 0.8, 0.5, 0.2];
            let resp = ui.add(
                BarVisualizer::new(&levels)
                    .mirrored(true)
                    .align(BarAlign::Center)
                    .idle(BarIdle::Wave),
            );
            assert!(resp.rect.width() > 0.0);
            assert!(resp.rect.height() > 0.0);
        });
    }

    #[test]
    fn sweep_level_peaks_correctly() {
        let sweep = crate::ui::visualizers::bar_levels::sweep_level(12, 24, 0.0);
        assert!(sweep >= 0.0 && sweep <= 1.0);
    }
}
