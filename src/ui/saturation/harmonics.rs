//! Harmonic spectrum of a memoryless waveshaper.
//!
//! A static transfer curve has no memory, so feeding it a sine wave and
//! measuring its output spectrum shows exactly what the saturation adds.
//! This widget runs that measurement analytically each frame: it samples
//! [`SaturationType::evaluate`] over one cycle and takes a DFT at the first
//! `count` harmonics. Nothing is filtered, nothing is stored, and the
//! display follows Drive, Bias and Mix as the user turns them.

use egui::{
    Align2, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use crate::oklab::with_alpha;
use crate::ui::saturation::SaturationType;
use crate::ui::theme::Theme;

/// Samples per analysed cycle. A power of two that resolves the first
/// thirty harmonics with negligible leakage.
const DFT_SAMPLES: usize = 1024;

/// The quietest harmonic the display reports, in dB below the
/// fundamental.
pub const MIN_HARMONIC_DB: f32 = -120.0;

/// The most harmonics the analyser can resolve from [`DFT_SAMPLES`].
pub const MAX_HARMONICS: usize = 32;

/// Magnitudes of the first `count` harmonics, in dB relative to the
/// fundamental, for a full-scale sine through the given shaper.
///
/// `mix` is the wet amount in 0..=1; at 0 the shaper is bypassed and every
/// harmonic above the fundamental sits at [`MIN_HARMONIC_DB`].
#[must_use]
pub fn harmonic_levels(
    sat_type: SaturationType,
    drive: f32,
    bias: f32,
    mix: f32,
    count: usize,
) -> Vec<f32> {
    let count = count.clamp(1, MAX_HARMONICS);
    let mix = mix.clamp(0.0, 1.0);
    let drive = drive.max(0.1);
    let bias = bias.clamp(-0.8, 0.8);

    let mut shaped = Vec::with_capacity(DFT_SAMPLES);
    let mut magnitudes = Vec::with_capacity(count);

    let step = std::f32::consts::TAU / DFT_SAMPLES as f32;
    for i in 0..DFT_SAMPLES {
        let x = (step * i as f32).sin();
        let wet = sat_type.evaluate(x, drive, bias);
        shaped.push(x * (1.0 - mix) + wet * mix);
    }

    for harmonic in 1..=count {
        let omega = std::f32::consts::TAU * harmonic as f32 / DFT_SAMPLES as f32;
        let (step_cos, step_sin) = (omega.cos(), omega.sin());
        let (mut cos, mut sin) = (1.0_f32, 0.0_f32);
        let (mut re, mut im) = (0.0_f32, 0.0_f32);
        for sample in &shaped {
            re += sample * cos;
            im -= sample * sin;
            let next_cos = cos * step_cos - sin * step_sin;
            sin = sin * step_cos + cos * step_sin;
            cos = next_cos;
        }
        magnitudes.push(2.0 * (re * re + im * im).sqrt() / DFT_SAMPLES as f32);
    }

    let fundamental = magnitudes[0].max(1e-12);
    magnitudes
        .iter()
        .map(|magnitude| {
            let db = 20.0 * (magnitude / fundamental).log10();
            db.max(MIN_HARMONIC_DB)
        })
        .collect()
}

/// Total harmonic distortion as a percentage, from
/// [`harmonic_levels`]-style relative dB levels.
#[must_use]
pub fn thd_percent(levels: &[f32]) -> f32 {
    let power: f32 = levels.iter().skip(1).map(|db| 10.0_f32.powf(db / 10.0)).sum();
    (power.sqrt() * 100.0).clamp(0.0, 999.0)
}

/// A harmonic bar display for a saturation or distortion transfer.
///
/// ```ignore
/// ui.add(
///     HarmonicsSpectrum::new(SaturationType::Tube, 4.0)
///         .bias(0.1)
///         .mix(0.8),
/// );
/// ```
#[must_use = "a HarmonicsSpectrum draws nothing until it is added to a Ui"]
pub struct HarmonicsSpectrum {
    sat_type: SaturationType,
    drive: f32,
    bias: f32,
    mix: f32,
    count: usize,
    floor_db: f32,
    desired_size: Vec2,
    show_labels: bool,
    show_values: bool,
}

impl HarmonicsSpectrum {
    /// Create a spectrum for the given shaper and drive.
    pub fn new(sat_type: SaturationType, drive: f32) -> Self {
        Self {
            sat_type,
            drive,
            bias: 0.0,
            mix: 1.0,
            count: 8,
            floor_db: -72.0,
            desired_size: Vec2::new(280.0, 160.0),
            show_labels: true,
            show_values: true,
        }
    }

    /// DC bias that breaks symmetry and brings even harmonics up.
    #[must_use]
    pub fn bias(mut self, bias: f32) -> Self {
        self.bias = bias.clamp(-0.8, 0.8);
        self
    }

    /// Wet/dry mix in 0..=1.
    #[must_use]
    pub fn mix(mut self, mix: f32) -> Self {
        self.mix = mix.clamp(0.0, 1.0);
        self
    }

    /// Number of harmonics to display, fundamental included. Default 8.
    #[must_use]
    pub fn count(mut self, count: usize) -> Self {
        self.count = count.clamp(1, MAX_HARMONICS);
        self
    }

    /// The bottom of the dB axis. Default -72 dB.
    #[must_use]
    pub fn floor_db(mut self, floor_db: f32) -> Self {
        self.floor_db = floor_db.clamp(-120.0, -12.0);
        self
    }

    /// Explicit size. Default 280 × 160 points.
    #[must_use]
    pub fn desired_size(mut self, size: Vec2) -> Self {
        self.desired_size = Vec2::new(size.x.max(120.0), size.y.max(80.0));
        self
    }

    /// Draw `H1`…`Hn` under the bars. Default true.
    #[must_use]
    pub fn labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Draw the dB value above each bar. Default true.
    #[must_use]
    pub fn values(mut self, show: bool) -> Self {
        self.show_values = show;
        self
    }

    /// The levels this widget will draw, useful for a readout or a test.
    #[must_use]
    pub fn levels(&self) -> Vec<f32> {
        harmonic_levels(self.sat_type, self.drive, self.bias, self.mix, self.count)
    }
}

impl Widget for HarmonicsSpectrum {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let (rect, response) = ui.allocate_exact_size(self.desired_size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let painter = ui.painter_at(rect);

        // Panel
        painter.rect_filled(rect, CornerRadius::same(theme.radius.md() as u8), palette.surface);
        painter.rect_stroke(
            rect,
            CornerRadius::same(theme.radius.md() as u8),
            Stroke::new(1.0_f32, palette.border),
            StrokeKind::Inside,
        );

        let levels = harmonic_levels(self.sat_type, self.drive, self.bias, self.mix, self.count);
        let thd = thd_percent(&levels);

        // Plot area leaves a gutter for dB tick labels, a band for the value
        // readouts above full-height bars, and a row for H labels.
        let pad = 8.0;
        let gutter = 24.0;
        let label_row = if self.show_labels { 13.0 } else { 2.0 };
        let value_row = if self.show_values { 11.0 } else { 2.0 };
        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + pad + gutter, rect.top() + pad + value_row),
            Pos2::new(rect.right() - pad, rect.bottom() - pad - label_row),
        );
        let floor_db = self.floor_db;
        let y_for = |db: f32| -> f32 {
            let norm = ((db - floor_db) / -floor_db).clamp(0.0, 1.0);
            plot.bottom() - norm * plot.height()
        };

        // dB grid
        let grid_font = FontId::proportional(8.5);
        let grid_stroke = Stroke::new(1.0_f32, with_alpha(palette.foreground, 0.07));
        let mut tick = 0.0_f32;
        while tick >= floor_db {
            let y = y_for(tick);
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                grid_stroke,
            );
            painter.text(
                Pos2::new(plot.left() - 4.0, y),
                Align2::RIGHT_CENTER,
                format!("{tick:.0}"),
                grid_font.clone(),
                palette.text_dim,
            );
            tick -= 20.0;
        }

        // THD badge
        painter.text(
            Pos2::new(plot.right(), rect.top() + 7.0),
            Align2::RIGHT_TOP,
            format!("THD {thd:.1}%"),
            FontId::monospace(9.5),
            palette.text_dim,
        );

        // Bars
        let slot = plot.width() / self.count as f32;
        let bar_width = (slot * 0.56).max(3.0);
        let label_font = FontId::monospace(8.5);
        let value_font = FontId::monospace(8.0);
        let baseline = plot.bottom();

        for (index, level) in levels.iter().enumerate() {
            let center_x = plot.left() + slot * (index as f32 + 0.5);
            let top = y_for(level.clamp(floor_db, 0.0));
            let item = Rect::from_min_max(
                Pos2::new(center_x - slot / 2.0, plot.top()),
                Pos2::new(center_x + slot / 2.0, baseline),
            );

            let color = if index == 0 {
                palette.primary
            } else if index % 2 == 1 {
                palette.chart[1]
            } else {
                palette.chart[3]
            };

            if index == 0 {
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(item.left(), plot.top()),
                        Pos2::new(item.right(), baseline),
                    ),
                    CornerRadius::same(2),
                    with_alpha(palette.primary, 0.06),
                );
            }

            let bar = Rect::from_min_max(
                Pos2::new(center_x - bar_width / 2.0, top),
                Pos2::new(center_x + bar_width / 2.0, baseline),
            );
            painter.rect_filled(bar, CornerRadius::same(2), color);

            if self.show_labels {
                painter.text(
                    Pos2::new(center_x, baseline + 2.0),
                    Align2::CENTER_TOP,
                    format!("H{}", index + 1),
                    label_font.clone(),
                    palette.text_dim,
                );
            }

            if self.show_values && *level > floor_db + 6.0 && slot >= 22.0 {
                painter.text(
                    Pos2::new(center_x, top - 2.0),
                    Align2::CENTER_BOTTOM,
                    format!("{level:.0}"),
                    value_font.clone(),
                    if index == 0 { palette.foreground } else { palette.text_dim },
                );
            }

            let role = match index {
                0 => "Fundamental",
                _ if index % 2 == 1 => "Odd harmonic",
                _ => "Even harmonic",
            };
            ui.interact(item, ui.id().with(("harmonic", index)), Sense::hover())
                .on_hover_text(format!("{role} · H{} · {level:.1} dB", index + 1));
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dry_signal_has_no_harmonics() {
        let levels = harmonic_levels(SaturationType::SoftTanh, 6.0, 0.0, 0.0, 8);
        assert!((levels[0]).abs() < 1e-3, "fundamental should be 0 dB");
        for level in &levels[1..] {
            assert!(*level < -100.0, "dry signal should have no harmonics: {level}");
        }
        assert!(thd_percent(&levels) < 0.01);
    }

    #[test]
    fn a_symmetric_clipper_makes_odd_harmonics() {
        let levels = harmonic_levels(SaturationType::HardClip, 8.0, 0.0, 1.0, 8);
        assert!(levels[2] > -30.0, "third harmonic should be strong");
        assert!(levels[1] < -60.0, "second harmonic should be absent without bias");
    }

    #[test]
    fn bias_breaks_symmetry_and_lifts_even_harmonics() {
        let clean = harmonic_levels(SaturationType::Tube, 3.0, 0.0, 1.0, 4);
        let biased = harmonic_levels(SaturationType::Tube, 3.0, 0.3, 1.0, 4);
        assert!(
            biased[1] > clean[1] + 6.0,
            "bias should raise the second harmonic: {:?} vs {:?}",
            biased,
            clean
        );
    }

    #[test]
    fn distortion_rises_with_drive() {
        let soft = thd_percent(&harmonic_levels(SaturationType::SoftTanh, 1.5, 0.0, 1.0, 8));
        let hot = thd_percent(&harmonic_levels(SaturationType::SoftTanh, 8.0, 0.0, 1.0, 8));
        assert!(hot > soft, "hot drive should distort more: {hot} vs {soft}");
    }

    #[test]
    fn levels_stay_finite_and_normalised() {
        for sat_type in SaturationType::ALL {
            let levels = harmonic_levels(sat_type, 10.0, 0.4, 0.3, 12);
            assert_eq!(levels.len(), 12);
            assert!(levels[0].abs() < 1e-3, "fundamental defines 0 dB");
            assert!(levels.iter().all(|db| db.is_finite() && *db <= 0.01));
            assert!(levels.iter().all(|db| *db >= MIN_HARMONIC_DB));
        }
    }

    #[test]
    fn harmonics_spectrum_renders_headlessly() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                HarmonicsSpectrum::new(SaturationType::Tube, 4.0)
                    .bias(0.15)
                    .mix(0.8),
            );
        });
        assert!(!output.shapes.is_empty(), "spectrum should emit shapes");
    }
}
