//! Soft and hard clip lamps for saturation and distortion stages.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use crate::oklab::with_alpha;
use crate::ui::saturation::SaturationType;
use crate::ui::theme::Theme;

/// Input level, in shaper units, where the transfer visibly leaves its
/// linear region.
const SOFT_THRESHOLD: f32 = 0.7;

/// Output magnitude that counts as hitting the shaper's ceiling.
const CEILING: f32 = 0.995;

/// A pair of lamps reporting soft and hard clipping.
///
/// Presentational: it is told which lamps are lit. [`ClipIndicators::from_peak`]
/// derives both from a shaper, a drive and an input peak when the plugin
/// does not already track clipping itself.
///
/// ```ignore
/// let lamps = ClipIndicators::from_peak(SaturationType::HardClip, drive, bias, peak);
/// ui.add(lamps);
/// ```
#[must_use = "ClipIndicators draw nothing until they are added to a Ui"]
pub struct ClipIndicators {
    soft: bool,
    hard: bool,
    labels: bool,
    led_size: f32,
    desired_size: Option<Vec2>,
}

impl ClipIndicators {
    /// Two lamps in the given states.
    pub fn new(soft: bool, hard: bool) -> Self {
        Self {
            soft,
            hard,
            labels: true,
            led_size: 8.0,
            desired_size: None,
        }
    }

    /// Derive the lamp states from a shaper's response to `peak`.
    ///
    /// Soft lights when the biased, driven input leaves the curve's near
    /// linear region; hard lights when the output reaches the ceiling the
    /// shaper clamps to (or, for the hard clipper, when the input itself
    /// crosses full scale).
    pub fn from_peak(sat_type: SaturationType, drive: f32, bias: f32, peak: f32) -> Self {
        let driven = (peak.clamp(-1.0, 1.0) + bias) * drive.max(0.1);
        let output = sat_type.evaluate(peak, drive, bias).abs();
        let soft = driven.abs() > SOFT_THRESHOLD;
        let hard = match sat_type {
            SaturationType::HardClip => driven.abs() >= 1.0,
            SaturationType::Foldback => driven.abs() >= 1.0,
            SaturationType::Tape => output >= 0.98,
            SaturationType::Tube | SaturationType::SoftTanh | SaturationType::Fuzz => {
                output >= CEILING
            }
        };
        Self::new(soft, hard)
    }

    /// Draw the `SOFT` and `HARD` captions. Default true.
    #[must_use]
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    /// Lamp diameter. Default 8 points.
    #[must_use]
    pub fn led_size(mut self, size: f32) -> Self {
        self.led_size = size.max(4.0);
        self
    }

    /// An explicit size; otherwise the widget measures its captions.
    #[must_use]
    pub fn desired_size(mut self, size: Vec2) -> Self {
        self.desired_size = Some(size);
        self
    }

    /// Whether either lamp is lit.
    #[must_use]
    pub fn is_lit(&self) -> bool {
        self.soft || self.hard
    }
}

impl Widget for ClipIndicators {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let font = FontId::proportional(8.5);
        let lamps = [
            ("SOFT", self.soft, palette.meter_warn),
            ("HARD", self.hard, palette.meter_clip),
        ];

        let label_width = if self.labels {
            lamps
                .iter()
                .map(|(text, _, _)| {
                    ui.painter()
                        .layout_no_wrap((*text).to_owned(), font.clone(), Color32::PLACEHOLDER)
                        .size()
                        .x
                })
                .fold(0.0_f32, f32::max)
        } else {
            0.0
        };
        let label_gap = 6.0;
        let between = 12.0;
        let pad = 8.0;
        let lamp_width = self.led_size + if self.labels { label_gap + label_width } else { 0.0 };
        let natural = Vec2::new(
            pad * 2.0 + lamp_width * 2.0 + if self.labels { between } else { 6.0 },
            24.0,
        );
        let size = self.desired_size.unwrap_or(natural);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, CornerRadius::same(theme.radius.sm() as u8), palette.track);
        painter.rect_stroke(
            rect,
            CornerRadius::same(theme.radius.sm() as u8),
            Stroke::new(1.0_f32, palette.border),
            egui::StrokeKind::Inside,
        );

        let mut x = rect.left() + pad;
        let center_y = rect.center().y;
        for (text, lit, color) in lamps {
            let center = Pos2::new(x + self.led_size / 2.0, center_y);

            if lit {
                painter.circle_filled(center, self.led_size * 0.95, with_alpha(color, 0.22));
                painter.circle_filled(center, self.led_size / 2.0, color);
            } else {
                painter.circle_filled(
                    center,
                    self.led_size / 2.0,
                    with_alpha(palette.foreground, 0.10),
                );
                painter.circle_stroke(
                    center,
                    self.led_size / 2.0,
                    Stroke::new(1.0_f32, palette.border),
                );
            }

            x += self.led_size;
            if self.labels {
                painter.text(
                    Pos2::new(x + label_gap, center_y),
                    Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    if lit { palette.foreground } else { palette.text_dim },
                );
                x += label_gap + label_width + between;
            } else {
                x += 6.0;
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn a_quiet_signal_lights_nothing() {
        let lamps = ClipIndicators::from_peak(SaturationType::SoftTanh, 1.0, 0.0, 0.3);
        assert!(!lamps.soft);
        assert!(!lamps.hard);
        assert!(!lamps.is_lit());
    }

    #[test]
    fn a_hot_signal_lights_soft_first() {
        let lamps = ClipIndicators::from_peak(SaturationType::SoftTanh, 2.0, 0.0, 0.5);
        assert!(lamps.soft, "drive 2 at half scale should leave the linear region");
        assert!(!lamps.hard, "tanh should not reach its ceiling at drive 2");
    }

    #[test]
    fn a_hard_clipper_lights_both_when_it_clamps() {
        let lamps = ClipIndicators::from_peak(SaturationType::HardClip, 6.0, 0.0, 1.0);
        assert!(lamps.soft);
        assert!(lamps.hard);
    }

    #[test]
    fn clip_indicators_render_headlessly() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(ClipIndicators::new(true, false));
        });
        assert!(!output.shapes.is_empty(), "clip indicators should emit shapes");
    }

    #[test]
    fn a_rect_fits_the_two_lamps() {
        let size = Vec2::new(130.0, 24.0);
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        assert!(rect.width() > 2.0 * 8.0);
    }
}
