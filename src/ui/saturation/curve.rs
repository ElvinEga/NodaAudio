//! Saturation and distortion transfer curve visualizer.
//!
//! Visualizes waveshaper transfer functions $y = f(x)$ for analog tape,
//! tube warmth, soft tanh, hard clipping, wavefolding, and fuzz distortion.
//!
//! Used in saturator and distortion plugin interfaces (e.g., FabFilter Saturn,
//! Decapitator, Trash) to illustrate drive, bias, mix, and non-linear harmonic generation.

use egui::{
    Color32, CornerRadius, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
    epaint::RectShape,
};

use crate::oklab::with_alpha;
use crate::ui::theme::Theme;

/// Type of waveshaping / saturation algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SaturationType {
    /// Soft magnetic tape saturation with cubic soft knee.
    Tape,
    /// Asymmetric vacuum tube saturation generating rich even harmonics.
    Tube,
    /// Hyperbolic tangent soft saturation.
    SoftTanh,
    /// Brickwall hard clipping with sharp corners (odd harmonics).
    HardClip,
    /// Sine wavefolding / foldback distortion.
    Foldback,
    /// Asymmetric high-gain fuzz distortion.
    Fuzz,
}

impl SaturationType {
    /// All available saturation types.
    pub const ALL: [Self; 6] = [
        Self::Tape,
        Self::Tube,
        Self::SoftTanh,
        Self::HardClip,
        Self::Foldback,
        Self::Fuzz,
    ];

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tape => "Tape",
            Self::Tube => "Tube",
            Self::SoftTanh => "Soft Tanh",
            Self::HardClip => "Hard Clip",
            Self::Foldback => "Foldback",
            Self::Fuzz => "Fuzz",
        }
    }

    /// Compact name for a segmented selector.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Tape => "Tape",
            Self::Tube => "Tube",
            Self::SoftTanh => "Tanh",
            Self::HardClip => "Clip",
            Self::Foldback => "Fold",
            Self::Fuzz => "Fuzz",
        }
    }

    /// One-line description of the character, for tooltips.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Tape => "Soft cubic compression with a gentle knee",
            Self::Tube => "Asymmetric valve warmth with even harmonics",
            Self::SoftTanh => "Smooth symmetric tanh saturation",
            Self::HardClip => "Brickwall clipping with sharp odd harmonics",
            Self::Foldback => "Sine wavefolding for metallic overtones",
            Self::Fuzz => "High-gain asymmetric fuzz distortion",
        }
    }

    /// Evaluate the non-linear transfer function $f(x)$ for input $x \in [-1, 1]$
    /// given a drive factor ($\ge 1.0$) and a DC bias offset.
    pub fn evaluate(&self, x: f32, drive: f32, bias: f32) -> f32 {
        let x_biased = (x + bias) * drive;
        let raw = match self {
            Self::Tape => {
                // Cubic soft saturator: f'(0) = 1 and f(±1.5) = ±1.
                let clamped = x_biased.clamp(-1.5, 1.5);
                clamped - (clamped * clamped * clamped) / 6.75
            }
            Self::Tube => {
                // Asymmetric: softer positive compression, sharper negative clip.
                if x_biased >= 0.0 {
                    1.0 - (-x_biased).exp()
                } else {
                    -(1.0 - (x_biased * 0.7).exp()) / 0.7
                }
            }
            Self::SoftTanh => {
                // Normalized by tanh(drive) so the ceiling stays near 1.0.
                let norm = drive.max(0.1).tanh();
                x_biased.tanh() / norm
            }
            Self::HardClip => x_biased.clamp(-1.0, 1.0),
            Self::Foldback => (x_biased * std::f32::consts::FRAC_PI_2).sin(),
            Self::Fuzz => {
                let sign = if x_biased >= 0.0 { 1.0 } else { -1.0 };
                sign * (1.0 - (-x_biased.abs() * 1.5).exp()) * 1.1
            }
        };

        raw.clamp(-1.2, 1.2)
    }
}

/// A transfer curve visualizer for saturation and distortion.
///
/// ```ignore
/// let curve = SaturationCurve::new(SaturationType::Tube, 4.0)
///     .mix(0.75)
///     .bias(0.05)
///     .input_level(Some(0.6));
/// ui.add(curve);
/// ```
#[must_use = "a SaturationCurve draws nothing until it is added to a Ui"]
pub struct SaturationCurve {
    sat_type: SaturationType,
    drive: f32,
    mix: f32,
    bias: f32,
    input_level: Option<f32>,
    desired_size: Vec2,
    show_harmonics_fill: bool,
    show_unity_line: bool,
}

impl SaturationCurve {
    /// Create a new saturation curve with the given type and drive (typically 1.0..10.0).
    pub fn new(sat_type: SaturationType, drive: f32) -> Self {
        Self {
            sat_type,
            drive: drive.max(0.1),
            mix: 1.0,
            bias: 0.0,
            input_level: None,
            desired_size: Vec2::new(180.0, 180.0),
            show_harmonics_fill: true,
            show_unity_line: true,
        }
    }

    /// Set wet/dry mix in range 0.0 (clean 1:1) to 1.0 (fully saturated).
    #[must_use]
    pub fn mix(mut self, mix: f32) -> Self {
        self.mix = mix.clamp(0.0, 1.0);
        self
    }

    /// Set DC bias offset (asymmetry) in range -0.8 to 0.8.
    #[must_use]
    pub fn bias(mut self, bias: f32) -> Self {
        self.bias = bias.clamp(-0.8, 0.8);
        self
    }

    /// Supply a live input signal value in [-1.0, 1.0] to render an active tracking dot.
    #[must_use]
    pub fn input_level(mut self, level: Option<f32>) -> Self {
        self.input_level = level;
        self
    }

    /// Set explicit desired size in pixels.
    #[must_use]
    pub fn desired_size(mut self, size: Vec2) -> Self {
        self.desired_size = size;
        self
    }

    /// Whether to show the filled region between the linear 1:1 line and the saturated curve.
    #[must_use]
    pub fn show_harmonics_fill(mut self, show: bool) -> Self {
        self.show_harmonics_fill = show;
        self
    }

    /// Whether to draw the clean 1:1 reference diagonal.
    #[must_use]
    pub fn show_unity_line(mut self, show: bool) -> Self {
        self.show_unity_line = show;
        self
    }

    /// The wire value of the current drive factor.
    #[must_use]
    pub fn drive(&self) -> f32 {
        self.drive
    }

    /// Evaluate the mixed output for input $x \in [-1, 1]$.
    pub fn evaluate_mixed(&self, x: f32) -> f32 {
        let saturated = self.sat_type.evaluate(x, self.drive, self.bias);
        let linear = x;
        linear * (1.0 - self.mix) + saturated * self.mix
    }
}

/// Fill the region between two sampled curves with a triangle strip, so a
/// curve that crosses its reference still fills correctly.
fn fill_between(painter: &egui::Painter, upper: &[Pos2], lower: &[Pos2], color: Color32) {
    if upper.len() < 2 || upper.len() != lower.len() || color.a() == 0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for (top, bottom) in upper.iter().zip(lower) {
        mesh.colored_vertex(*top, color);
        mesh.colored_vertex(*bottom, color);
    }
    for i in 0..upper.len() - 1 {
        let top = i as u32 * 2;
        let bottom = top + 1;
        mesh.add_triangle(top, bottom, top + 2);
        mesh.add_triangle(bottom, bottom + 2, top + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl Widget for SaturationCurve {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.desired_size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let painter = ui.painter_at(rect);

        // 1. Background panel
        painter.add(RectShape::new(
            rect,
            CornerRadius::same(theme.radius.md() as u8),
            palette.surface,
            Stroke::new(1.0_f32, palette.border),
            StrokeKind::Inside,
        ));

        // Coordinate transforms: input x in [-1.2, 1.2], output y in [-1.2, 1.2]
        let pad = 12.0_f32;
        let plot_rect = rect.shrink(pad);
        let x_to_screen = |x: f32| -> f32 {
            let norm = (x + 1.2) / 2.4;
            plot_rect.left() + norm * plot_rect.width()
        };
        let y_to_screen = |y: f32| -> f32 {
            let norm = (y + 1.2) / 2.4;
            // Screen Y is inverted (top is +1.2, bottom is -1.2).
            plot_rect.bottom() - norm * plot_rect.height()
        };
        let to_pos = |x: f32, y: f32| -> Pos2 { Pos2::new(x_to_screen(x), y_to_screen(y)) };

        // 2. Grid and axes
        let grid_stroke = Stroke::new(1.0_f32, with_alpha(palette.foreground, 0.08));
        let axis_stroke = Stroke::new(1.0_f32, with_alpha(palette.foreground, 0.16));

        painter.line_segment([to_pos(0.0, -1.2), to_pos(0.0, 1.2)], axis_stroke);
        painter.line_segment([to_pos(-1.2, 0.0), to_pos(1.2, 0.0)], axis_stroke);

        // Full-scale ceilings and the ±1 input boundaries.
        let ceiling_stroke = Stroke::new(1.0_f32, with_alpha(palette.destructive, 0.28));
        painter.line_segment([to_pos(-1.2, 1.0), to_pos(1.2, 1.0)], ceiling_stroke);
        painter.line_segment([to_pos(-1.2, -1.0), to_pos(1.2, -1.0)], ceiling_stroke);
        painter.line_segment([to_pos(1.0, -1.2), to_pos(1.0, 1.2)], grid_stroke);
        painter.line_segment([to_pos(-1.0, -1.2), to_pos(-1.0, 1.2)], grid_stroke);

        // 3. 1:1 Unity reference line (clean diagonal)
        if self.show_unity_line {
            let unity_stroke = Stroke::new(1.0_f32, with_alpha(palette.foreground, 0.22));
            painter.line_segment([to_pos(-1.2, -1.2), to_pos(1.2, 1.2)], unity_stroke);
        }

        // 4. Sample transfer curve points
        const SAMPLES: usize = 96;
        let mut curve_points = Vec::with_capacity(SAMPLES);
        let mut unity_points = Vec::with_capacity(SAMPLES);

        for i in 0..SAMPLES {
            let t = i as f32 / (SAMPLES - 1) as f32;
            let x = -1.2 + t * 2.4;
            let y = self.evaluate_mixed(x).clamp(-1.2, 1.2);
            curve_points.push(to_pos(x, y));
            unity_points.push(to_pos(x, x));
        }

        // 5. Shaded area between unity diagonal and saturation curve (distortion energy)
        if self.show_harmonics_fill {
            let fill_color = with_alpha(palette.meter_warn, 0.14);
            fill_between(&painter, &curve_points, &unity_points, fill_color);
        }

        // 6. Draw transfer curve stroke
        painter.add(egui::epaint::PathShape::line(
            curve_points,
            Stroke::new(2.2_f32, palette.meter_warn),
        ));

        // 7. Live operating point dot
        if let Some(in_val) = self.input_level {
            let clamped_in = in_val.clamp(-1.2, 1.2);
            let out_val = self.evaluate_mixed(clamped_in).clamp(-1.2, 1.2);
            let pos = to_pos(clamped_in, out_val);

            painter.circle_filled(pos, 6.0_f32, with_alpha(palette.primary, 0.35));
            painter.circle_filled(pos, 3.5_f32, palette.foreground);
        }

        // 8. Type badge in top-left
        let badge_text = format!("{}  ({:.1}x)", self.sat_type.name(), self.drive);
        painter.text(
            rect.min + Vec2::new(8.0, 7.0),
            egui::Align2::LEFT_TOP,
            badge_text,
            egui::FontId::proportional(10.5),
            palette.text_dim,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_saturation_types_evaluate_finite() {
        for sat in SaturationType::ALL {
            for drive in [1.0, 2.5, 6.0, 10.0] {
                for bias in [-0.4, 0.0, 0.4] {
                    for x in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                        let y = sat.evaluate(x, drive, bias);
                        assert!(y.is_finite(), "saturated value must be finite");
                        assert!(y.abs() <= 1.21, "saturated value must stay within clamp");
                    }
                }
            }
        }
    }

    #[test]
    fn test_tape_saturation_is_symmetrical_at_zero_bias() {
        let pos = SaturationType::Tape.evaluate(0.6, 2.0, 0.0);
        let neg = SaturationType::Tape.evaluate(-0.6, 2.0, 0.0);
        assert!(
            (pos + neg).abs() < 1e-4,
            "Tape must be odd-symmetric around zero with bias 0"
        );
    }

    #[test]
    fn test_tube_saturation_is_asymmetrical() {
        let pos = SaturationType::Tube.evaluate(0.6, 2.0, 0.0);
        let neg = SaturationType::Tube.evaluate(-0.6, 2.0, 0.0);
        assert_ne!(pos.abs(), neg.abs(), "Tube must produce asymmetric saturation");
    }

    #[test]
    fn test_hard_clip_saturates_at_one() {
        let out = SaturationType::HardClip.evaluate(0.8, 5.0, 0.0);
        assert_eq!(out, 1.0, "Hard clip must clamp to 1.0 at high drive");
    }

    #[test]
    fn test_wet_dry_mix_interpolation() {
        let curve = SaturationCurve::new(SaturationType::SoftTanh, 4.0).mix(0.0);
        // At 0% mix, output should exactly equal input (clean 1:1)
        assert!((curve.evaluate_mixed(0.5) - 0.5).abs() < 1e-5);

        let curve_full = SaturationCurve::new(SaturationType::SoftTanh, 4.0).mix(1.0);
        let full_val = curve_full.evaluate_mixed(0.5);
        assert_ne!(full_val, 0.5);

        let curve_half = SaturationCurve::new(SaturationType::SoftTanh, 4.0).mix(0.5);
        let half_val = curve_half.evaluate_mixed(0.5);
        let expected_half = 0.5 * 0.5 + full_val * 0.5;
        assert!((half_val - expected_half).abs() < 1e-5);
    }

    #[test]
    fn saturation_curve_renders_headlessly() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                SaturationCurve::new(SaturationType::Tube, 3.0)
                    .mix(0.75)
                    .bias(0.1)
                    .input_level(Some(0.4)),
            );
        });
        assert!(!output.shapes.is_empty(), "curve should emit shapes");
    }
}
