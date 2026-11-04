//! A hi-fi brushed aluminium metallic volume knob and rotary dial.
//!
//! Directly inspired by audiocn's `KnobScale` + `KnobCap` volume dial architecture
//! (`audiocn/components/examples/knob-volume.tsx` and `audiocn/plans/010-knob-volume-dial.md`).
//!
//! Features:
//! - Anisotropic brushed aluminium cap rendered via GPU triangle fan conic sheen,
//!   featuring authentic lathe-turned highlights, concentric micro-grooving, dark bezel,
//!   specular upper rim, and recessed indicator dot.
//! - Circular dial graduation scale with 100 ticks (5-major, 10-numbered labels)
//!   rotating outward along the radial arc.
//! - Active/inactive illumination: ticks and numeric labels light up in bright white
//!   from origin (0) to current value, while remaining ticks are dimmed.
//! - Drag modes: Circular (`Grab`) and Vertical (`Relative`) with Shift for 10x fine control.
//! - Mini variant (`MetallicKnobVariant::Mini`) for dense channel strip layouts with
//!   engraved line indicator and light lower lip.

use std::f32::consts::{PI, TAU};

use egui::epaint::{Mesh, TextShape, Vertex};
use egui::{
    Align2, Color32, FontId, Key, PointerButton, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget,
};

use crate::db::clamp;
use crate::taper::Taper;
use crate::ui::controls::fader::DragMode;
use crate::ui::theme::Theme;

const DEGREES_TO_RADIANS: f32 = PI / 180.0;
const DEFAULT_ARC_DEG: f32 = 270.0;

// Radii in viewBox units (0..100)
// Scale:
const SCALE_LABEL_R: f32 = 47.5;
const SCALE_LABEL_SIZE: f32 = 4.5;
const SCALE_MAJOR_INNER: f32 = 34.5;
const SCALE_MAJOR_OUTER: f32 = 43.5;
const SCALE_MINOR_INNER: f32 = 35.5;
const SCALE_MINOR_OUTER: f32 = 40.5;

// Cap (Default / Volume Dial):
const CAP_DEFAULT_BEZEL: f32 = 25.5;
const CAP_DEFAULT_FACE: f32 = 22.5;
const CAP_DEFAULT_RIM: f32 = 23.3;
const CAP_DEFAULT_DOT_DIST: f32 = 17.0;
const CAP_DEFAULT_DOT_RADIUS: f32 = 1.6;

// Cap (Mini / Dense layout):
const CAP_MINI_BEZEL: f32 = 32.0;
const CAP_MINI_FACE: f32 = 28.2;
const CAP_MINI_RIM: f32 = 29.2;
const CAP_MINI_LINE_INNER: f32 = 8.5;
const CAP_MINI_LINE_OUTER: f32 = 23.0;
const CAP_MINI_LINE_WIDTH: f32 = 3.4;
const CAP_MINI_LINE_LIP: f32 = 0.6;

/// Visual variant for the metallic knob.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetallicKnobVariant {
    /// Full Hi-Fi Volume Dial: large brushed aluminium cap surrounded by
    /// an outer 100-tick graduation scale with radial rotated numbers (0..100)
    /// and an indicator dot.
    Dial,
    /// Mini Metal Knob: compact brushed aluminium cap with an engraved line indicator
    /// and light lower lip, designed for dense channel strip and rack layouts.
    Mini,
}

/// A hi-fi brushed aluminium metallic rotary control.
#[must_use = "a MetallicKnob draws nothing until it is added to a Ui"]
pub struct MetallicKnob<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    origin: Option<f32>,
    reset: Option<f32>,
    variant: MetallicKnobVariant,
    diameter: Option<f32>,
    ticks: usize,
    major_every: usize,
    label_every: usize,
    arc_degrees: f32,
    drag_mode: DragMode,
    throw: f32,
    taper: Taper,
    bipolar: bool,
    step: Option<f32>,
}

impl<'a> MetallicKnob<'a> {
    /// Create a standard 0..100 metallic volume dial.
    pub fn volume(value: &'a mut f32) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            origin: Some(0.0),
            reset: Some(0.0),
            variant: MetallicKnobVariant::Dial,
            diameter: None,
            ticks: 100,
            major_every: 5,
            label_every: 10,
            arc_degrees: DEFAULT_ARC_DEG,
            drag_mode: DragMode::Relative,
            throw: 200.0,
            taper: Taper::Linear,
            bipolar: false,
            step: Some(1.0),
        }
    }

    /// Create a metallic knob over a custom engineering range.
    pub fn new(value: &'a mut f32, min: f32, max: f32) -> Self {
        Self {
            value,
            min,
            max,
            origin: Some(min),
            reset: Some(min),
            variant: MetallicKnobVariant::Dial,
            diameter: None,
            ticks: 100,
            major_every: 5,
            label_every: 10,
            arc_degrees: DEFAULT_ARC_DEG,
            drag_mode: DragMode::Relative,
            throw: 200.0,
            taper: Taper::Linear,
            bipolar: false,
            step: None,
        }
    }

    /// Create a compact mini metallic knob for dense channel strips.
    pub fn mini(value: &'a mut f32, min: f32, max: f32) -> Self {
        Self {
            value,
            min,
            max,
            origin: Some(min),
            reset: Some(min),
            variant: MetallicKnobVariant::Mini,
            diameter: Some(48.0),
            ticks: 0,
            major_every: 0,
            label_every: 0,
            arc_degrees: DEFAULT_ARC_DEG,
            drag_mode: DragMode::Relative,
            throw: 160.0,
            taper: Taper::Linear,
            bipolar: false,
            step: None,
        }
    }

    /// Set visual variant (Dial or Mini).
    #[must_use]
    pub fn variant(mut self, variant: MetallicKnobVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set explicit diameter in points.
    #[must_use]
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = Some(diameter);
        self
    }

    /// Total graduation ticks around the arc (default 100).
    #[must_use]
    pub fn ticks(mut self, ticks: usize) -> Self {
        self.ticks = ticks;
        self
    }

    /// Every nth tick is a major tick (default 5).
    #[must_use]
    pub fn major_every(mut self, major_every: usize) -> Self {
        self.major_every = major_every;
        self
    }

    /// Every nth tick receives a rotated numeric label (default 10, 0 to disable).
    #[must_use]
    pub fn label_every(mut self, label_every: usize) -> Self {
        self.label_every = label_every;
        self
    }

    /// Total arc sweep in degrees (default 270.0).
    #[must_use]
    pub fn arc_degrees(mut self, degrees: f32) -> Self {
        self.arc_degrees = degrees;
        self
    }

    /// The origin from which active lighting begins (default min, or 0.0 for bipolar).
    #[must_use]
    pub fn origin(mut self, origin: f32) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Set reset value for double-click or secondary click.
    #[must_use]
    pub fn reset(mut self, reset: Option<f32>) -> Self {
        self.reset = reset;
        self
    }

    /// Drag interaction mode: `Relative` (vertical axial drag) or `Grab` (circular angle drag).
    #[must_use]
    pub fn drag_mode(mut self, mode: DragMode) -> Self {
        self.drag_mode = mode;
        self
    }

    /// Transfer law (Linear, Audio console, Logarithmic).
    #[must_use]
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
        self
    }

    /// Enable bipolar lighting from center.
    #[must_use]
    pub fn bipolar(mut self, bipolar: bool) -> Self {
        self.bipolar = bipolar;
        if bipolar && self.origin.is_none() {
            self.origin = Some((self.min + self.max) * 0.5);
        }
        self
    }

    /// Step quantization increment.
    #[must_use]
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// Current normalized position 0..1.
    fn position(&self) -> f32 {
        self.taper.to_position(*self.value, self.min, self.max)
    }

    /// Normalized origin position 0..1.
    fn origin_position(&self) -> f32 {
        if self.bipolar {
            0.5
        } else {
            let orig = self.origin.unwrap_or(self.min);
            self.taper.to_position(orig, self.min, self.max)
        }
    }
}

/// Persistent interaction state between frames.
#[derive(Clone, Copy, Debug, Default)]
struct MetallicKnobState {
    drag_start: Option<(Pos2, f32)>,
}

impl Widget for MetallicKnob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let default_diam = match self.variant {
            MetallicKnobVariant::Dial => 156.0,
            MetallicKnobVariant::Mini => 48.0,
        };
        let diameter = self.diameter.unwrap_or(default_diam);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click_and_drag());

        let id = response.id;
        let mut state = ui.data_mut(|d| d.get_temp::<MetallicKnobState>(id).unwrap_or_default());

        let before = *self.value;
        let throw = self.throw;

        // Interaction Handling
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                state.drag_start = Some((pos, self.position()));
            }
        }
        if response.dragged() {
            let shift_fine = ui.input(|i| i.modifiers.shift);
            let effective_throw = if shift_fine { throw * 10.0 } else { throw };

            let new_pos = if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                match self.drag_mode {
                    DragMode::Grab => {
                        let center = rect.center();
                        let dir = pointer - center;
                        if dir.length_sq() > 4.0 {
                            // Angle clockwise from 12 o'clock (-Y axis)
                            let angle_rad = dir.x.atan2(-dir.y);
                            let arc_rad = self.arc_degrees * DEGREES_TO_RADIANS;
                            let start_rad = -arc_rad * 0.5;
                            let norm = (angle_rad - start_rad) / arc_rad;
                            clamp(norm, 0.0, 1.0)
                        } else {
                            self.position()
                        }
                    }
                    DragMode::Relative => {
                        if let Some((start_pos, start_val)) = state.drag_start {
                            let delta_y = start_pos.y - pointer.y; // up increases
                            clamp(start_val + delta_y / effective_throw, 0.0, 1.0)
                        } else {
                            self.position()
                        }
                    }
                }
            } else {
                self.position()
            };

            let mut val = self.taper.to_value(new_pos, self.min, self.max);
            if let Some(step) = self.step {
                val = (val / step).round() * step;
            }
            *self.value = clamp(val, self.min, self.max);
        }
        if response.drag_stopped() {
            state.drag_start = None;
        }

        // Mouse wheel adjustment
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta.abs() > 0.0 {
                let shift = ui.input(|i| i.modifiers.shift);
                let factor = if shift { 0.001 } else { 0.01 };
                let delta_pos = (scroll_delta * factor).clamp(-0.2, 0.2);
                let new_pos = clamp(self.position() + delta_pos, 0.0, 1.0);
                *self.value = clamp(self.taper.to_value(new_pos, self.min, self.max), self.min, self.max);
            }
        }

        // Double-click / Alt+click / Secondary click reset
        if response.double_clicked_by(PointerButton::Primary)
            || response.clicked_by(PointerButton::Secondary)
        {
            if let Some(reset_val) = self.reset {
                *self.value = clamp(reset_val, self.min, self.max);
            }
        }

        // Keyboard navigation
        if response.has_focus() {
            let shift = ui.input(|i| i.modifiers.shift);
            let small_step = (self.max - self.min) * if shift { 0.002 } else { 0.02 };
            let large_step = (self.max - self.min) * 0.10;

            if ui.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowRight)) {
                *self.value = clamp(*self.value + small_step, self.min, self.max);
            }
            if ui.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowLeft)) {
                *self.value = clamp(*self.value - small_step, self.min, self.max);
            }
            if ui.input(|i| i.key_pressed(Key::PageUp)) {
                *self.value = clamp(*self.value + large_step, self.min, self.max);
            }
            if ui.input(|i| i.key_pressed(Key::PageDown)) {
                *self.value = clamp(*self.value - large_step, self.min, self.max);
            }
            if ui.input(|i| i.key_pressed(Key::Home)) {
                *self.value = self.min;
            }
            if ui.input(|i| i.key_pressed(Key::End)) {
                *self.value = self.max;
            }
        }

        ui.data_mut(|d| d.insert_temp(id, state));

        // Painting
        if ui.is_rect_visible(rect) {
            let theme = Theme::of(ui);
            let center = rect.center();
            let scale = diameter / 100.0;
            let norm_pos = self.position();
            let origin_pos = self.origin_position();

            // Pointer angle: 0 rad is 12 o'clock, sweeping from -arc/2 to +arc/2
            let arc_rad = self.arc_degrees * DEGREES_TO_RADIANS;
            let pointer_angle = -arc_rad * 0.5 + norm_pos * arc_rad;

            match self.variant {
                MetallicKnobVariant::Dial => {
                    self.paint_scale(ui, center, scale, norm_pos, origin_pos, &theme);
                    self.paint_cap(ui, center, scale, pointer_angle, MetallicKnobVariant::Dial);
                }
                MetallicKnobVariant::Mini => {
                    self.paint_mini_track(ui, center, scale, norm_pos, origin_pos, &theme);
                    self.paint_cap(ui, center, scale, pointer_angle, MetallicKnobVariant::Mini);
                }
            }
        }

        let mut final_resp = response;
        if *self.value != before {
            final_resp.mark_changed();
        }
        final_resp
    }
}

impl MetallicKnob<'_> {
    /// Paint the outer graduation ticks and rotated numbers.
    fn paint_scale(
        &self,
        ui: &mut Ui,
        center: Pos2,
        scale: f32,
        current_pos: f32,
        origin_pos: f32,
        _theme: &Theme,
    ) {
        if self.ticks == 0 {
            return;
        }

        let lit_min = origin_pos.min(current_pos) - 1e-6;
        let lit_max = origin_pos.max(current_pos) + 1e-6;
        let total_ticks = self.ticks;
        let major_step = self.major_every.max(1);
        let arc_rad = self.arc_degrees * DEGREES_TO_RADIANS;

        let active_color = Color32::from_rgb(255, 255, 255);
        let dim_major = Color32::from_rgb(128, 133, 142);
        let dim_minor = Color32::from_rgba_unmultiplied(110, 115, 124, 110);
        let dim_label = Color32::from_rgb(128, 133, 142);

        for i in 0..=total_ticks {
            let tick_pos = i as f32 / total_ticks as f32;
            let angle = -arc_rad * 0.5 + tick_pos * arc_rad;
            let is_lit = tick_pos >= lit_min && tick_pos <= lit_max;
            let is_major = i % major_step == 0;

            let (inner_r, outer_r, stroke_w) = if is_major {
                (
                    SCALE_MAJOR_INNER * scale,
                    SCALE_MAJOR_OUTER * scale,
                    (1.15 * scale).max(1.0),
                )
            } else {
                (
                    SCALE_MINOR_INNER * scale,
                    SCALE_MINOR_OUTER * scale,
                    (0.55 * scale).max(0.75),
                )
            };

            let inner_pt = point_at(center, angle, inner_r);
            let outer_pt = point_at(center, angle, outer_r);

            let tick_color = if is_lit {
                active_color
            } else if is_major {
                dim_major
            } else {
                dim_minor
            };

            ui.painter().line_segment([inner_pt, outer_pt], Stroke::new(stroke_w, tick_color));

            // Rotated numeric label
            if self.label_every > 0 && i % self.label_every == 0 {
                let label_val = self.min + tick_pos * (self.max - self.min);
                let text = format!("{label_val:.0}");
                let label_r = SCALE_LABEL_R * scale;
                let label_pos = point_at(center, angle, label_r);

                let label_color = if is_lit { active_color } else { dim_label };
                let font_size = (SCALE_LABEL_SIZE * scale).max(8.5);
                let font_id = FontId::proportional(font_size);

                let galley = ui.painter().layout_no_wrap(text, font_id, label_color);
                let shape = TextShape::new(label_pos, galley, label_color)
                    .with_angle_and_anchor(angle, Align2::CENTER_CENTER);
                ui.painter().add(shape);
            }
        }
    }

    /// Paint the mini track and range around a compact knob.
    fn paint_mini_track(
        &self,
        ui: &mut Ui,
        center: Pos2,
        scale: f32,
        current_pos: f32,
        origin_pos: f32,
        theme: &Theme,
    ) {
        let track_r = 38.0 * scale;
        let track_w = (3.5 * scale).max(2.0);
        let arc_rad = self.arc_degrees * DEGREES_TO_RADIANS;
        let start_angle = -arc_rad * 0.5;

        // Background track arc
        let n_steps = 48;
        let mut bg_pts = Vec::with_capacity(n_steps + 1);
        for s in 0..=n_steps {
            let a = start_angle + (s as f32 / n_steps as f32) * arc_rad;
            bg_pts.push(point_at(center, a, track_r));
        }
        ui.painter().add(egui::Shape::line(
            bg_pts,
            Stroke::new(track_w, theme.palette.border),
        ));

        // Active range arc
        let lit_start = origin_pos.min(current_pos);
        let lit_end = origin_pos.max(current_pos);
        if lit_end - lit_start > 0.005 {
            let range_steps = (n_steps as f32 * (lit_end - lit_start)).ceil() as usize;
            let range_steps = range_steps.max(2);
            let mut range_pts = Vec::with_capacity(range_steps + 1);
            for s in 0..=range_steps {
                let frac = lit_start + (s as f32 / range_steps as f32) * (lit_end - lit_start);
                let a = start_angle + frac * arc_rad;
                range_pts.push(point_at(center, a, track_r));
            }
            ui.painter().add(egui::Shape::line(
                range_pts,
                Stroke::new(track_w, theme.palette.primary),
            ));
        }
    }

    /// Paint the brushed aluminium cap: dark bezel, metallic rim, anisotropic conic face, and indicator.
    fn paint_cap(
        &self,
        ui: &mut Ui,
        center: Pos2,
        scale: f32,
        pointer_angle: f32,
        variant: MetallicKnobVariant,
    ) {
        let (bezel_r, face_r, rim_r) = match variant {
            MetallicKnobVariant::Dial => (
                CAP_DEFAULT_BEZEL * scale,
                CAP_DEFAULT_FACE * scale,
                CAP_DEFAULT_RIM * scale,
            ),
            MetallicKnobVariant::Mini => (
                CAP_MINI_BEZEL * scale,
                CAP_MINI_FACE * scale,
                CAP_MINI_RIM * scale,
            ),
        };

        // 1. Soft drop-shadow halo
        let halo_r = match variant {
            MetallicKnobVariant::Dial => 33.0 * scale,
            MetallicKnobVariant::Mini => 36.0 * scale,
        };
        for (step_r, alpha) in [
            (bezel_r + 1.2 * scale, 45),
            (bezel_r + 3.0 * scale, 25),
            (bezel_r + 5.5 * scale, 12),
            (halo_r, 4),
        ] {
            ui.painter().circle_stroke(
                center,
                step_r,
                Stroke::new(1.8 * scale, Color32::from_rgba_unmultiplied(0, 0, 0, alpha)),
            );
        }

        // 2. Dark bezel disc
        ui.painter().circle_filled(center, bezel_r, Color32::from_rgb(22, 24, 28));
        ui.painter().circle_stroke(
            center,
            bezel_r,
            Stroke::new(0.4 * scale, Color32::from_rgba_unmultiplied(200, 205, 215, 40)),
        );

        // Top specular highlight edge on bezel (light from above)
        let top_highlight_pts = [
            point_at(center, -0.6, bezel_r - 0.4 * scale),
            point_at(center, -0.3, bezel_r - 0.2 * scale),
            point_at(center, 0.0, bezel_r - 0.2 * scale),
            point_at(center, 0.3, bezel_r - 0.2 * scale),
            point_at(center, 0.6, bezel_r - 0.4 * scale),
        ];
        ui.painter().add(egui::Shape::line(
            top_highlight_pts.to_vec(),
            Stroke::new(0.8 * scale, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
        ));

        // 3. Bright metal chamfer / rim
        ui.painter().circle_filled(center, rim_r, Color32::from_rgb(215, 218, 224));
        // Bevel rim top shadow
        ui.painter().circle_stroke(
            center,
            rim_r,
            Stroke::new(0.6 * scale, Color32::from_rgba_unmultiplied(35, 38, 42, 90)),
        );

        // 4. Brushed aluminium face (GPU Anisotropic Conic Sheen Triangle Fan)
        let mut mesh = Mesh::default();
        let center_color = Color32::from_rgb(220, 222, 226);
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::new(0.5, 0.5),
            color: center_color,
        });

        let n_slices = 144;
        for s in 0..=n_slices {
            let phi = (s as f32 / n_slices as f32) * TAU;
            let rel_t = (phi - 15.0_f32.to_radians()).rem_euclid(TAU) / TAU;
            let factor = conic_sheen_factor(rel_t);
            let v_color = metal_sheen_color(factor);
            let p = Pos2::new(center.x + face_r * phi.sin(), center.y - face_r * phi.cos());
            mesh.vertices.push(Vertex {
                pos: p,
                uv: Pos2::new(0.5 + 0.5 * phi.sin(), 0.5 - 0.5 * phi.cos()),
                color: v_color,
            });
        }

        for s in 1..=n_slices {
            mesh.indices.push(0);
            mesh.indices.push(s as u32);
            mesh.indices.push((s + 1) as u32);
        }
        ui.painter().add(mesh);

        // 5. Lathe-turned concentric micro-texture rings
        let pitch = match variant {
            MetallicKnobVariant::Dial => 1.4 * scale,
            MetallicKnobVariant::Mini => 2.0 * scale,
        };
        let mut r = 2.5 * scale;
        let mut ring_idx = 0;
        while r < face_r - 0.5 * scale {
            let ring_col = if ring_idx % 2 == 0 {
                Color32::from_rgba_unmultiplied(255, 255, 255, 14)
            } else {
                Color32::from_rgba_unmultiplied(18, 22, 28, 16)
            };
            ui.painter().circle_stroke(center, r, Stroke::new(0.5 * scale, ring_col));
            r += pitch;
            ring_idx += 1;
        }

        // 6. Indicator Marker
        match variant {
            MetallicKnobVariant::Dial => {
                let dot_dist = CAP_DEFAULT_DOT_DIST * scale;
                let dot_r = CAP_DEFAULT_DOT_RADIUS * scale;
                let dot_center = point_at(center, pointer_angle, dot_dist);

                // Outer bevel shadow
                ui.painter().circle_filled(
                    dot_center,
                    dot_r + 0.35 * scale,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 60),
                );
                // Dark recessed dot core
                ui.painter().circle_filled(
                    dot_center,
                    dot_r,
                    Color32::from_rgb(26, 28, 32),
                );
                // Lower specular highlight crescent inside indentation
                let lip_pt1 = dot_center + Vec2::new(-dot_r * 0.7, dot_r * 0.4);
                let lip_pt2 = dot_center + Vec2::new(0.0, dot_r * 0.75);
                let lip_pt3 = dot_center + Vec2::new(dot_r * 0.7, dot_r * 0.4);
                ui.painter().add(egui::Shape::line(
                    vec![lip_pt1, lip_pt2, lip_pt3],
                    Stroke::new(0.45 * scale, Color32::from_rgba_unmultiplied(255, 255, 255, 75)),
                ));
            }
            MetallicKnobVariant::Mini => {
                let inner_r = CAP_MINI_LINE_INNER * scale;
                let outer_r = CAP_MINI_LINE_OUTER * scale;
                let line_w = (CAP_MINI_LINE_WIDTH * scale).max(2.0);
                let lip_dy = CAP_MINI_LINE_LIP * scale;

                let p1 = point_at(center, pointer_angle, inner_r);
                let p2 = point_at(center, pointer_angle, outer_r);

                // Lower lip highlight catching light from above
                ui.painter().line_segment(
                    [p1 + Vec2::new(0.0, lip_dy), p2 + Vec2::new(0.0, lip_dy)],
                    Stroke::new(line_w, Color32::from_rgba_unmultiplied(255, 255, 255, 175)),
                );
                // Dark engraved groove
                ui.painter().line_segment(
                    [p1, p2],
                    Stroke::new(line_w, Color32::from_rgba_unmultiplied(22, 24, 28, 210)),
                );
            }
        }
    }
}

/// Compute Cartesian coordinate for an angle clockwise from 12 o'clock (-Y axis).
#[inline]
fn point_at(center: Pos2, angle_rad: f32, radius: f32) -> Pos2 {
    Pos2::new(
        center.x + radius * angle_rad.sin(),
        center.y - radius * angle_rad.cos(),
    )
}

/// Conic sheen reflection intensity curve matching audiocn's CSS conic gradient.
fn conic_sheen_factor(rel_t: f32) -> f32 {
    let t = rel_t.fract();
    const STOPS: [(f32, f32); 9] = [
        (0.00, 0.38),
        (0.09, 0.00),
        (0.21, 0.38),
        (0.32, 0.00),
        (0.46, -0.45),
        (0.58, 0.00),
        (0.70, 0.38),
        (0.83, 0.00),
        (1.00, 0.38),
    ];
    for i in 0..STOPS.len() - 1 {
        let (t0, v0) = STOPS[i];
        let (t1, v1) = STOPS[i + 1];
        if t >= t0 && t <= t1 {
            let u = (t - t0) / (t1 - t0);
            let smooth_u = (1.0 - (u * PI).cos()) * 0.5;
            return v0 + (v1 - v0) * smooth_u;
        }
    }
    0.38
}

/// Convert sheen factor into a shaded metallic vertex color.
fn metal_sheen_color(factor: f32) -> Color32 {
    let base_r = 222.0_f32;
    let base_g = 224.0_f32;
    let base_b = 228.0_f32;

    if factor >= 0.0 {
        let t = (factor / 0.38).clamp(0.0, 1.0);
        let r = (base_r + (255.0 - base_r) * t) as u8;
        let g = (base_g + (255.0 - base_g) * t) as u8;
        let b = (base_b + (255.0 - base_b) * t) as u8;
        Color32::from_rgb(r, g, b)
    } else {
        let t = ((-factor) / 0.45).clamp(0.0, 1.0);
        let r = (base_r + (132.0 - base_r) * t) as u8;
        let g = (base_g + (136.0 - base_g) * t) as u8;
        let b = (base_b + (144.0 - base_b) * t) as u8;
        Color32::from_rgb(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metallic_knob_sheen_factor_symmetry() {
        // High specular highlights exist at the stops
        assert!(conic_sheen_factor(0.00) > 0.3);
        assert!(conic_sheen_factor(0.21) > 0.3);
        assert!(conic_sheen_factor(0.70) > 0.3);
        // Deep shadow exists at 0.46
        assert!(conic_sheen_factor(0.46) < -0.3);
    }

    #[test]
    fn metallic_knob_renders_dial_headlessly() {
        let mut volume = 33.0;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(MetallicKnob::volume(&mut volume).diameter(160.0));
        });
        assert!(!output.shapes.is_empty(), "metallic dial should draw shapes");
    }

    #[test]
    fn metallic_knob_renders_mini_headlessly() {
        let mut gain = 6.0;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(MetallicKnob::mini(&mut gain, -24.0, 24.0));
        });
        assert!(!output.shapes.is_empty(), "metallic mini knob should draw shapes");
    }
}
