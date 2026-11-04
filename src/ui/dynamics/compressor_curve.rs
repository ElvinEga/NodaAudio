//! Interactive compressor transfer function curve and dynamics display.
//!
//! Visualizes input dBFS vs output dBFS:
//! - 1:1 unity diagonal reference line.
//! - Compressor transfer function with threshold, ratio, soft knee, and makeup gain.
//! - Gain reduction shaded zone between unity and compression transfer line.
//! - Interactive draggable threshold handle on the knee.
//! - Real-time live signal tracking dot on the curve.

use egui::{Align2, FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use crate::db::clamp;
use crate::oklab::with_alpha;
use crate::ui::theme::Theme;

/// Default minimum display level in dBFS.
pub const DEFAULT_MIN_DB: f32 = -60.0;

/// Default maximum display level in dBFS.
pub const DEFAULT_MAX_DB: f32 = 0.0;

/// An interactive compressor transfer curve display.
#[must_use = "a CompressorCurve draws nothing until it is added to a Ui"]
pub struct CompressorCurve<'a> {
    threshold_db: &'a mut f32,
    ratio: &'a mut f32,
    knee_db: f32,
    makeup_gain_db: f32,
    live_input_db: Option<f32>,
    live_gr_db: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    interactive: bool,
}

impl<'a> CompressorCurve<'a> {
    /// Create a new compressor curve display bound to threshold and ratio.
    pub fn new(threshold_db: &'a mut f32, ratio: &'a mut f32) -> Self {
        Self {
            threshold_db,
            ratio,
            knee_db: 6.0,
            makeup_gain_db: 0.0,
            live_input_db: None,
            live_gr_db: None,
            width: None,
            height: None,
            interactive: true,
        }
    }

    /// Soft knee width in decibels (default 6.0 dB).
    #[must_use]
    pub fn knee(mut self, knee_db: f32) -> Self {
        self.knee_db = knee_db.abs();
        self
    }

    /// Makeup gain in decibels (default 0.0 dB).
    #[must_use]
    pub fn makeup_gain(mut self, gain_db: f32) -> Self {
        self.makeup_gain_db = gain_db;
        self
    }

    /// Real-time live input level in dBFS to render tracking ball.
    #[must_use]
    pub fn live_input(mut self, input_db: f32) -> Self {
        self.live_input_db = Some(input_db);
        self
    }

    /// Real-time gain reduction in dB.
    #[must_use]
    pub fn live_gr(mut self, gr_db: f32) -> Self {
        self.live_gr_db = Some(gr_db.abs());
        self
    }

    /// Set explicit dimensions.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Enable or disable interactive handle dragging (default true).
    #[must_use]
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Calculate output level in dBFS from an input level in dBFS according to the soft-knee compressor formula.
    #[must_use]
    pub fn evaluate_output_db(&self, in_db: f32) -> f32 {
        let thresh = *self.threshold_db;
        let ratio = (*self.ratio).max(1.0);
        let knee = self.knee_db;
        let makeup = self.makeup_gain_db;

        let out_db = if knee <= 0.01 {
            // Hard knee
            if in_db <= thresh {
                in_db
            } else {
                thresh + (in_db - thresh) / ratio
            }
        } else {
            // Soft knee quadratic spline
            let half_knee = knee * 0.5;
            if in_db <= thresh - half_knee {
                in_db
            } else if in_db >= thresh + half_knee {
                thresh + (in_db - thresh) / ratio
            } else {
                let diff = in_db - thresh + half_knee;
                in_db + ((1.0 / ratio - 1.0) * diff * diff) / (2.0 * knee)
            }
        };

        out_db + makeup
    }
}

impl Widget for CompressorCurve<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let w = self.width.unwrap_or_else(|| ui.available_width().min(320.0).max(220.0));
        let h = self.height.unwrap_or(w * 0.85);
        let (rect, mut response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let min_db = DEFAULT_MIN_DB;
        let max_db = DEFAULT_MAX_DB;
        let span = max_db - min_db;

        // Coordinate transforms: input dB -> screen X, output dB -> screen Y
        let in_db_to_x = |db: f32| -> f32 {
            let norm = ((db - min_db) / span).clamp(0.0, 1.0);
            rect.min.x + norm * rect.width()
        };
        let out_db_to_y = |db: f32| -> f32 {
            let norm = ((db - min_db) / span).clamp(0.0, 1.0);
            rect.max.y - norm * rect.height() // -60 is at bottom, 0 at top
        };
        let x_to_in_db = |x: f32| -> f32 {
            let norm = ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            min_db + norm * span
        };
        let _y_to_out_db = |y: f32| -> f32 {
            let norm = ((rect.max.y - y) / rect.height()).clamp(0.0, 1.0);
            min_db + norm * span
        };

        // Handle interaction (threshold & ratio)
        let thresh_x = in_db_to_x(*self.threshold_db);
        let thresh_y = out_db_to_y(self.evaluate_output_db(*self.threshold_db));
        let handle_pos = Pos2::new(thresh_x, thresh_y);

        let pointer_pos = ui.ctx().pointer_interact_pos();
        let is_hovered = pointer_pos.map_or(false, |p| p.distance(handle_pos) <= 12.0);

        if self.interactive && response.dragged() {
            if let Some(pos) = pointer_pos {
                *self.threshold_db = clamp(x_to_in_db(pos.x), -50.0, -1.0);
                response.mark_changed();
            }
        }

        // Background chassis
        ui.painter().rect_filled(rect, theme.radius.md(), theme.palette.surface);
        ui.painter().rect_stroke(
            rect,
            theme.radius.md(),
            Stroke::new(1.0_f32, theme.palette.border),
            egui::StrokeKind::Inside,
        );

        // Grid lines (-48, -36, -24, -12, 0 dB)
        let grid_steps = [-48.0, -36.0, -24.0, -12.0, 0.0];
        let font_id = FontId::proportional(theme.size.label_font() - 2.0);

        for &db in &grid_steps {
            let x = in_db_to_x(db);
            let y = out_db_to_y(db);

            let grid_stroke =
                Stroke::new(0.6_f32, with_alpha(theme.palette.text_dim, 0.12));
            // Vertical line
            ui.painter().line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                grid_stroke,
            );
            // Horizontal line
            ui.painter().line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                grid_stroke,
            );

            // dB labels
            ui.painter().text(
                Pos2::new(x, rect.max.y - 3.0),
                Align2::CENTER_BOTTOM,
                format!("{db:.0}"),
                font_id.clone(),
                theme.palette.text_dim,
            );
            ui.painter().text(
                Pos2::new(rect.min.x + 4.0, y - 2.0),
                Align2::LEFT_BOTTOM,
                format!("{db:.0}"),
                font_id.clone(),
                theme.palette.text_dim,
            );
        }

        // 1:1 Unity reference diagonal line (bottom-left to top-right)
        ui.painter().line_segment(
            [Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.min.y)],
            Stroke::new(1.0_f32, with_alpha(theme.palette.text_dim, 0.25)),
        );

        // Evaluate Transfer Curve points
        let n_steps = 128;
        let mut curve_pts: Vec<Pos2> = Vec::with_capacity(n_steps + 1);
        let mut gr_poly_pts: Vec<Pos2> = Vec::with_capacity((n_steps + 1) * 2);

        for i in 0..=n_steps {
            let frac = i as f32 / n_steps as f32;
            let in_db = min_db + frac * span;
            let out_db = self.evaluate_output_db(in_db);

            let pt = Pos2::new(in_db_to_x(in_db), out_db_to_y(out_db));
            curve_pts.push(pt);
        }

        // Build Gain Reduction shaded zone (between unity line and compression curve)
        for &pt in &curve_pts {
            let unity_y = out_db_to_y(x_to_in_db(pt.x));
            if pt.y > unity_y {
                gr_poly_pts.push(pt);
            }
        }
        if gr_poly_pts.len() >= 2 {
            let mut full_gr_poly = gr_poly_pts.clone();
            // Return along the unity line
            for pt in gr_poly_pts.iter().rev() {
                full_gr_poly.push(Pos2::new(pt.x, out_db_to_y(x_to_in_db(pt.x))));
            }
            ui.painter().add(egui::Shape::convex_polygon(
                full_gr_poly,
                with_alpha(theme.palette.meter_warn, 0.14),
                Stroke::NONE,
            ));
        }

        // Draw bold transfer curve line
        ui.painter().add(egui::Shape::line(
            curve_pts,
            Stroke::new(2.2_f32, theme.palette.primary),
        ));

        // Threshold interactive handle
        if self.interactive {
            let handle_color = theme.palette.primary;
            if is_hovered || response.dragged() {
                ui.painter().circle_filled(
                    handle_pos,
                    10.0,
                    with_alpha(handle_color, 0.24),
                );
            }
            ui.painter().circle_filled(handle_pos, 5.5, handle_color);
            ui.painter().circle_stroke(
                handle_pos,
                5.5,
                Stroke::new(1.2_f32, theme.palette.surface),
            );
        }

        // Live input tracking dot
        if let Some(in_db) = self.live_input_db {
            let in_clamped = in_db.clamp(min_db, max_db);
            let out_db = self.evaluate_output_db(in_clamped);
            let live_pos = Pos2::new(in_db_to_x(in_clamped), out_db_to_y(out_db));

            ui.painter().circle_filled(live_pos, 4.0, theme.palette.foreground);
            ui.painter().circle_stroke(
                live_pos,
                4.0,
                Stroke::new(1.0_f32, theme.palette.primary),
            );
        }

        // Header / Info overlay: Threshold and Ratio
        let info_str = format!(
            "Thresh: {:.1} dB  ·  Ratio: {:.1}:1  ·  Knee: {:.1} dB",
            *self.threshold_db, *self.ratio, self.knee_db
        );
        let info_font = FontId::proportional(theme.size.label_font() - 1.0);
        ui.painter().text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 8.0),
            Align2::LEFT_TOP,
            info_str,
            info_font,
            theme.palette.foreground,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_knee_evaluation() {
        let mut thresh = -20.0;
        let mut ratio = 4.0;
        let curve = CompressorCurve::new(&mut thresh, &mut ratio).knee(0.0);

        // Below threshold: unity
        assert_eq!(curve.evaluate_output_db(-30.0), -30.0);
        assert_eq!(curve.evaluate_output_db(-20.0), -20.0);
        // Above threshold: compressed by 4:1
        // -10 dB is 10 dB above threshold, output should be -20 + 10 / 4 = -17.5 dB
        let out = curve.evaluate_output_db(-10.0);
        assert!((out - -17.5).abs() < 0.05, "got {out}");
    }

    #[test]
    fn compressor_curve_renders_headlessly() {
        let mut thresh = -18.0;
        let mut ratio = 3.5;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                CompressorCurve::new(&mut thresh, &mut ratio)
                    .knee(8.0)
                    .live_input(-12.0)
                    .size(260.0, 200.0),
            );
        });
        assert!(!output.shapes.is_empty(), "Compressor curve should emit shapes");
    }
}
