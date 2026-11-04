//! Gain reduction meter for dynamics processors (compressors, limiters, de-essers).
//!
//! A downward-metering attenuation bar:
//! - 0 dBFS rests at the top (vertical) or right (horizontal) with 0 attenuation.
//! - Attenuation expands downward/leftward into negative decibels (e.g. 0 to -24 dB).
//! - Active reduction bar in amber/orange with peak hold tick, scale graduations,
//!   and numeric attenuation readout.
//! - Supports single channel or stereo link reduction displays.

use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget,
};

use crate::db::clamp;
use crate::ui::theme::Theme;
use crate::ui::Orientation;

/// Maximum attenuation range in dB (default 24.0 dB).
pub const DEFAULT_GR_MAX_DB: f32 = 24.0;

/// A gain reduction meter for dynamics processors.
#[must_use = "a GainReductionMeter draws nothing until it is added to a Ui"]
pub struct GainReductionMeter<'a> {
    reduction_db: &'a [f32],
    max_db: f32,
    orientation: Orientation,
    show_scale: bool,
    show_readout: bool,
    show_hold: bool,
    peak_hold_db: Option<f32>,
    thickness: Option<f32>,
    length: Option<f32>,
    color: Option<Color32>,
}

impl<'a> GainReductionMeter<'a> {
    /// Create a mono gain reduction meter showing current attenuation in dB (positive or negative number).
    pub fn new(reduction_db: &'a [f32]) -> Self {
        Self {
            reduction_db,
            max_db: DEFAULT_GR_MAX_DB,
            orientation: Orientation::Vertical,
            show_scale: true,
            show_readout: true,
            show_hold: true,
            peak_hold_db: None,
            thickness: None,
            length: None,
            color: None,
        }
    }

    /// Convenience constructor for a single float reduction value.
    pub fn single(reduction_db: &'a f32) -> Self {
        Self::new(std::slice::from_ref(reduction_db))
    }

    /// Set maximum reduction range in dB (default 24.0).
    #[must_use]
    pub fn range(mut self, max_db: f32) -> Self {
        self.max_db = max_db.abs().max(3.0);
        self
    }

    /// Set layout orientation (Vertical or Horizontal).
    #[must_use]
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Enable or disable graduation scale.
    #[must_use]
    pub fn scale(mut self, show: bool) -> Self {
        self.show_scale = show;
        self
    }

    /// Enable or disable numeric dB readout.
    #[must_use]
    pub fn readout(mut self, show: bool) -> Self {
        self.show_readout = show;
        self
    }

    /// Enable or disable peak hold indicator.
    #[must_use]
    pub fn hold(mut self, show: bool) -> Self {
        self.show_hold = show;
        self
    }

    /// Supply explicit peak hold attenuation value.
    #[must_use]
    pub fn peak_hold(mut self, peak_db: f32) -> Self {
        self.peak_hold_db = Some(peak_db.abs());
        self
    }

    /// Custom bar thickness.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }

    /// Custom meter length.
    #[must_use]
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length);
        self
    }

    /// Custom reduction bar color.
    #[must_use]
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

impl Widget for GainReductionMeter<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let n_channels = self.reduction_db.len().max(1);

        let default_thick = 14.0 * n_channels as f32;
        let default_len = 160.0;

        let thick = self.thickness.unwrap_or(default_thick);
        let len = self.length.unwrap_or(default_len);

        let scale_w = if self.show_scale { 24.0 } else { 0.0 };
        let readout_h = if self.show_readout { 18.0 } else { 0.0 };

        let want_size = match self.orientation {
            Orientation::Vertical => Vec2::new(thick + scale_w + 4.0, len + readout_h + 4.0),
            Orientation::Horizontal => Vec2::new(len + readout_h + 4.0, thick + scale_w + 4.0),
        };

        let (rect, response) = ui.allocate_exact_size(want_size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let bar_color = self.color.unwrap_or(theme.palette.meter_warn);
        let max_range = self.max_db;

        // Partition rect based on orientation
        match self.orientation {
            Orientation::Vertical => {
                let bar_rect_w = thick;
                let bar_rect_h = len;
                let bar_rect = Rect::from_min_size(
                    Pos2::new(rect.min.x + scale_w + 2.0, rect.min.y),
                    Vec2::new(bar_rect_w, bar_rect_h),
                );

                // Track trough background
                ui.painter().rect_filled(bar_rect, theme.radius.sm(), theme.palette.surface);
                ui.painter().rect_stroke(
                    bar_rect,
                    theme.radius.sm(),
                    Stroke::new(1.0_f32, theme.palette.border),
                    egui::StrokeKind::Inside,
                );

                let ch_w = (bar_rect.width() / n_channels as f32).max(4.0);

                // Paint downward reduction bars per channel
                for (ch_idx, &raw_gr) in self.reduction_db.iter().enumerate() {
                    let atten = raw_gr.abs();
                    let norm = clamp(atten / max_range, 0.0, 1.0);
                    let h = norm * bar_rect.height();

                    let ch_min_x = bar_rect.min.x + ch_idx as f32 * ch_w;
                    let fill_rect = Rect::from_min_size(
                        Pos2::new(ch_min_x + 1.0, bar_rect.min.y + 1.0),
                        Vec2::new(ch_w - 2.0, h),
                    );

                    if h > 1.0 {
                        ui.painter().rect_filled(fill_rect, 1.0, bar_color);
                    }
                }

                // Peak hold marker line
                if self.show_hold {
                    let hold_atten = self.peak_hold_db.unwrap_or_else(|| {
                        self.reduction_db
                            .iter()
                            .fold(0.0_f32, |acc, &g| acc.max(g.abs()))
                    });
                    if hold_atten > 0.2 {
                        let norm = clamp(hold_atten / max_range, 0.0, 1.0);
                        let hold_y = bar_rect.min.y + norm * bar_rect.height();
                        ui.painter().line_segment(
                            [
                                Pos2::new(bar_rect.min.x + 1.0, hold_y),
                                Pos2::new(bar_rect.max.x - 1.0, hold_y),
                            ],
                            Stroke::new(1.5_f32, Color32::from_rgb(255, 255, 255)),
                        );
                    }
                }

                // Scale ticks on the left
                if self.show_scale {
                    let scale_ticks = [0.0, 3.0, 6.0, 10.0, 15.0, 20.0, max_range];
                    let font_id = FontId::proportional(theme.size.label_font() - 2.0);

                    for &tick in &scale_ticks {
                        if tick > max_range {
                            continue;
                        }
                        let norm = tick / max_range;
                        let y = bar_rect.min.y + norm * bar_rect.height();
                        let is_zero = tick == 0.0;

                        let p_tick_start = Pos2::new(bar_rect.min.x - 4.0, y);
                        let p_tick_end = Pos2::new(bar_rect.min.x - 1.0, y);
                        ui.painter().line_segment(
                            [p_tick_start, p_tick_end],
                            Stroke::new(1.0_f32, theme.palette.text_dim),
                        );

                        let label_str = if is_zero { "0".to_owned() } else { format!("{tick:.0}") };
                        ui.painter().text(
                            Pos2::new(rect.min.x + 1.0, y),
                            Align2::LEFT_CENTER,
                            label_str,
                            font_id.clone(),
                            theme.palette.text_dim,
                        );
                    }
                }

                // Readout text below
                if self.show_readout {
                    let max_atten = self
                        .reduction_db
                        .iter()
                        .fold(0.0_f32, |acc, &g| acc.max(g.abs()));
                    let readout_str = if max_atten < 0.1 {
                        "0.0 dB".to_owned()
                    } else {
                        format!("-{max_atten:.1} dB")
                    };
                    let font_id = FontId::proportional(theme.size.value_font());
                    let readout_pos = Pos2::new(bar_rect.center().x, bar_rect.max.y + 10.0);
                    let text_col = if max_atten > 0.1 { bar_color } else { theme.palette.text_dim };
                    ui.painter().text(readout_pos, Align2::CENTER_CENTER, readout_str, font_id, text_col);
                }
            }
            Orientation::Horizontal => {
                let bar_rect_w = len;
                let bar_rect_h = thick;
                let bar_rect = Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.min.y),
                    Vec2::new(bar_rect_w, bar_rect_h),
                );

                ui.painter().rect_filled(bar_rect, theme.radius.sm(), theme.palette.surface);
                ui.painter().rect_stroke(
                    bar_rect,
                    theme.radius.sm(),
                    Stroke::new(1.0_f32, theme.palette.border),
                    egui::StrokeKind::Inside,
                );

                let ch_h = (bar_rect.height() / n_channels as f32).max(4.0);

                // Right-to-left attenuation bar (0 dB is at the right edge)
                for (ch_idx, &raw_gr) in self.reduction_db.iter().enumerate() {
                    let atten = raw_gr.abs();
                    let norm = clamp(atten / max_range, 0.0, 1.0);
                    let w = norm * bar_rect.width();

                    let ch_min_y = bar_rect.min.y + ch_idx as f32 * ch_h;
                    let fill_rect = Rect::from_min_size(
                        Pos2::new(bar_rect.max.x - w - 1.0, ch_min_y + 1.0),
                        Vec2::new(w, ch_h - 2.0),
                    );

                    if w > 1.0 {
                        ui.painter().rect_filled(fill_rect, 1.0, bar_color);
                    }
                }

                // Readout to the right
                if self.show_readout {
                    let max_atten = self
                        .reduction_db
                        .iter()
                        .fold(0.0_f32, |acc, &g| acc.max(g.abs()));
                    let readout_str = if max_atten < 0.1 {
                        "0.0 dB".to_owned()
                    } else {
                        format!("-{max_atten:.1} dB")
                    };
                    let font_id = FontId::proportional(theme.size.value_font());
                    let readout_pos = Pos2::new(bar_rect.max.x + 8.0, bar_rect.center().y);
                    let text_col = if max_atten > 0.1 { bar_color } else { theme.palette.text_dim };
                    ui.painter().text(readout_pos, Align2::LEFT_CENTER, readout_str, font_id, text_col);
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
    fn gr_meter_renders_headlessly() {
        let reduction = [4.5, 6.2];
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(GainReductionMeter::new(&reduction).length(120.0));
            let _ = ui.add(
                GainReductionMeter::single(&3.2)
                    .orientation(Orientation::Horizontal)
                    .range(18.0),
            );
        });
        assert!(!output.shapes.is_empty(), "GR meter should emit shapes");
    }
}
