//! An electric plasma arc bar visualizer, ported from audiocn's
//! `electric-bar-visualizer.tsx` and `lib/electric.ts`.
//!
//! Every bar is a jagged filament: a midpoint-displaced bolt rerolled every
//! 42 ms and drawn three times — a wide glow, a body sheath and a hot core.
//! Arcs jump between loud neighbours, tips burn white above a quarter level,
//! and sudden rises throw sparks. Bars share the bar-level ballistics: instant
//! attack, smooth release, mirroring, idle motion and a loading sweep.

use std::time::Duration;

use egui::{Color32, Response, Sense, Ui, Vec2, Widget};

use crate::electric::{ElectricColors, mix_white};
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::bar_levels::{step_in_ui, BarLevelsOptions};
use crate::ui::visualizers::bar_visualizer::BarAlign;
use crate::ui::visualizers::electric_scene::{
    DEFAULT_BAR_COUNT, DEFAULT_BAR_GAP, DEFAULT_BAR_WIDTH, DEFAULT_INTENSITY,
    ElectricLayoutOptions, ElectricSceneOptions, ElectricSceneState, layout_electric_bars,
    paint_glow, paint_main,
};
use crate::ui::Orientation;

pub use super::bar_levels::BarIdle;

/// Repeatable pseudo-random hash generator for reproducible lightning crackle.
///
/// Kept for callers that want a cheap deterministic jitter value; the scene
/// itself uses the ported [`Crackle`](crate::electric::Crackle) sequence.
#[must_use]
pub fn pseudo_random(seed: u32, step: u32) -> f32 {
    let s = (seed.wrapping_mul(12989) ^ step.wrapping_mul(78233)) as f32;
    ((s.sin() * 43758.547).fract()).abs()
}

/// An electric plasma bar visualizer.
///
/// ```ignore
/// let levels = [0.2, 0.6, 0.9, 0.4, 0.1];
/// ui.add(ElectricBarVisualizer::new(&levels));
/// ```
#[must_use = "an ElectricBarVisualizer draws nothing until it is added to a Ui"]
pub struct ElectricBarVisualizer<'a> {
    levels: &'a [f32],
    bar_count: Option<usize>,
    align: BarAlign,
    mirrored: bool,
    min_level: f32,
    idle: BarIdle,
    loading: bool,
    intensity: f32,
    arcs: bool,
    sparks: bool,
    orientation: Orientation,
    bar_width: Option<f32>,
    bar_gap: Option<f32>,
    seed: Option<u32>,
    reduced_motion: bool,
    time: Option<f32>,
    color: Option<Color32>,
    glow_color: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> ElectricBarVisualizer<'a> {
    /// Create a new electric bar visualizer with input levels (0..=1).
    pub fn new(levels: &'a [f32]) -> Self {
        Self {
            levels,
            bar_count: None,
            align: BarAlign::Center,
            mirrored: false,
            min_level: 0.08,
            idle: BarIdle::Static,
            loading: false,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            orientation: Orientation::Horizontal,
            bar_width: None,
            bar_gap: None,
            seed: None,
            reduced_motion: false,
            time: None,
            color: None,
            glow_color: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Number of bars to render. Bands are resampled to fit. Default 16.
    pub fn bar_count(mut self, count: usize) -> Self {
        self.bar_count = Some(count);
        self
    }

    /// Vertical alignment (`Center`, `Start`, `End`).
    pub fn align(mut self, align: BarAlign) -> Self {
        self.align = align;
        self
    }

    /// Symmetric around middle bar, lows in the centre.
    pub fn mirrored(mut self, mirrored: bool) -> Self {
        self.mirrored = mirrored;
        self
    }

    /// Resting minimum level (0.0..=1.0).
    pub fn min_level(mut self, min_level: f32) -> Self {
        self.min_level = min_level.clamp(0.0, 1.0);
        self
    }

    /// Idle state mode (`Static`, `Pulse`, `Wave`).
    pub fn idle(mut self, idle: BarIdle) -> Self {
        self.idle = idle;
        self
    }

    /// Run an arc along the bars, for connecting or thinking states.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// How jagged and restless the filaments are, 0..=1. Default 0.6.
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Arcs jump between loud neighbouring bars. Default true.
    pub fn arcs(mut self, arcs: bool) -> Self {
        self.arcs = arcs;
        self
    }

    /// Sparks fly off the tips on sudden rises. Default true.
    pub fn sparks(mut self, sparks: bool) -> Self {
        self.sparks = sparks;
        self
    }

    /// Bars across the width or up the height.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Widest bar, in points. Default 6.
    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = Some(width.max(0.5));
        self
    }

    /// Gap between bars, in points. Default 4.
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = Some(gap.max(0.0));
        self
    }

    /// Seed the crackle. Each widget gets its own by default.
    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = Some(seed);
        self
    }

    /// A still, straight-filament scene, for reduced-motion settings.
    pub fn reduced_motion(mut self, reduced: bool) -> Self {
        self.reduced_motion = reduced;
        self
    }

    /// Animation clock in seconds.
    pub fn time(mut self, time: f32) -> Self {
        self.time = Some(time);
        self
    }

    /// Primary electric plasma color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Soft glow halo color.
    pub fn glow_color(mut self, color: Color32) -> Self {
        self.glow_color = Some(color);
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

    /// Size ladder position.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let (default_w, default_h) = match size {
            Size::Small => (140.0, 40.0),
            Size::Medium => (220.0, 64.0),
            Size::Large => (300.0, 80.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }
}

impl Widget for ElectricBarVisualizer<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let count = self.bar_count.unwrap_or(DEFAULT_BAR_COUNT);
        if count == 0 {
            return response;
        }

        let now_ms = self
            .time
            .map_or_else(|| ui.input(|i| i.time), f64::from)
            * 1000.0;

        let levels = step_in_ui(
            ui,
            response.id.with("electric_bar_levels"),
            BarLevelsOptions {
                bar_count: count,
                idle: self.idle,
                loading: self.loading,
                min_level: self.min_level,
                mirrored: self.mirrored,
                reduced_motion: self.reduced_motion,
            },
            now_ms,
            self.levels,
        );

        let horizontal = self.orientation == Orientation::Horizontal;
        let mut layout = layout_electric_bars(
            rect.width(),
            rect.height(),
            ElectricLayoutOptions {
                align: self.align,
                bar_count: count,
                bar_gap: self.bar_gap.unwrap_or(DEFAULT_BAR_GAP),
                bar_width: self.bar_width.unwrap_or(DEFAULT_BAR_WIDTH),
                horizontal,
            },
        );
        layout.origin = rect.min;

        let seed = self.seed.unwrap_or_else(|| {
            let value = response.id.value();
            (value ^ (value >> 32)) as u32
        });
        let scene_options = ElectricSceneOptions {
            bar_count: count,
            intensity: self.intensity,
            arcs: self.arcs,
            sparks: self.sparks,
            loading: self.loading,
            reduced_motion: self.reduced_motion,
            seed,
        };

        let mut scene = ui
            .ctx()
            .data_mut(|data| data.get_temp::<ElectricSceneState>(response.id.with("electric_scene")))
            .filter(|scene| scene.options() == scene_options)
            .unwrap_or_else(|| ElectricSceneState::new(scene_options));
        scene.step(now_ms, &levels.levels, &layout);

        let body = self.color.unwrap_or(theme.palette.primary);
        let glow = self.glow_color.unwrap_or(body);
        let colors = ElectricColors {
            body,
            core: mix_white(body, 0.72),
            glow,
        };
        let intensity = if self.reduced_motion { 0.0 } else { self.intensity };
        let painter = ui.painter();
        paint_glow(
            painter,
            &layout,
            &scene,
            &levels.levels,
            colors,
            intensity,
            now_ms,
        );
        paint_main(
            painter,
            &layout,
            &scene,
            &levels.levels,
            colors,
            intensity,
            now_ms,
        );

        if !self.reduced_motion || !levels.settled || scene.is_animating() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        ui.ctx()
            .data_mut(|data| data.insert_temp(response.id.with("electric_scene"), scene));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudo_random_produces_normalized_values() {
        let r1 = pseudo_random(1234, 1);
        let r2 = pseudo_random(1234, 2);
        assert!(r1 >= 0.0 && r1 <= 1.0);
        assert!(r2 >= 0.0 && r2 <= 1.0);
        assert_ne!(r1, r2);
    }

    #[test]
    fn electric_bar_visualizer_renders() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let levels = [0.2, 0.5, 0.8, 0.3];
            let resp = ui.add(
                ElectricBarVisualizer::new(&levels)
                    .mirrored(true)
                    .arcs(true)
                    .sparks(true),
            );
            assert!(resp.rect.width() > 0.0);
            assert!(resp.rect.height() > 0.0);
        });
    }

    #[test]
    fn intensity_is_a_share_not_a_multiplier() {
        let widget = ElectricBarVisualizer::new(&[]).intensity(4.0);
        assert_eq!(widget.intensity, 1.0);
        let widget = ElectricBarVisualizer::new(&[]).intensity(-1.0);
        assert_eq!(widget.intensity, 0.0);
    }
}
