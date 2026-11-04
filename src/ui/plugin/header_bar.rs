//! Header bar for professional audio plugins.
//!
//! Provides a standardized toolbar with:
//! - Input & output gain controls with mini LED meters
//! - A/B state comparison and state copy
//! - Oversampling quality selector (1x, 2x, 4x, 8x)
//! - Global plugin bypass with status illumination
//! - Undo / Redo triggers

use egui::{
    CornerRadius, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2, Widget,
};

use crate::db::SILENCE_DB;
use crate::oklab::with_alpha;
use crate::ui::theme::Theme;

/// Oversampling quality factor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Oversampling {
    /// 1x (No oversampling, zero added latency).
    None,
    /// 2x oversampling.
    X2,
    /// 4x oversampling.
    X4,
    /// 8x oversampling (ultra-high fidelity).
    X8,
}

impl Oversampling {
    /// All available oversampling options.
    pub const ALL: [Self; 4] = [Self::None, Self::X2, Self::X4, Self::X8];

    /// Short label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "1x",
            Self::X2 => "2x",
            Self::X4 => "4x",
            Self::X8 => "8x",
        }
    }
}

/// Active state in an A/B comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbState {
    /// Slot A is active.
    A,
    /// Slot B is active.
    B,
}

/// A standard pro audio plugin header toolbar.
pub struct PluginHeaderBar<'a> {
    title: &'a str,
    bypassed: &'a mut bool,
    ab_state: Option<&'a mut AbState>,
    oversampling: Option<&'a mut Oversampling>,
    in_gain_db: Option<&'a mut f32>,
    out_gain_db: Option<&'a mut f32>,
    in_meter_peak_db: Option<[f32; 2]>,
    out_meter_peak_db: Option<[f32; 2]>,
}

impl<'a> PluginHeaderBar<'a> {
    /// Create a new header bar with title and global bypass state.
    pub fn new(title: &'a str, bypassed: &'a mut bool) -> Self {
        Self {
            title,
            bypassed,
            ab_state: None,
            oversampling: None,
            in_gain_db: None,
            out_gain_db: None,
            in_meter_peak_db: None,
            out_meter_peak_db: None,
        }
    }

    /// Add A/B comparison state toggle.
    #[must_use]
    pub fn ab_state(mut self, ab_state: &'a mut AbState) -> Self {
        self.ab_state = Some(ab_state);
        self
    }

    /// Add oversampling quality selector.
    #[must_use]
    pub fn oversampling(mut self, oversampling: &'a mut Oversampling) -> Self {
        self.oversampling = Some(oversampling);
        self
    }

    /// Add input gain (dB) control and optional stereo peak metering [L, R].
    #[must_use]
    pub fn in_gain(mut self, gain_db: &'a mut f32, meter_peaks: Option<[f32; 2]>) -> Self {
        self.in_gain_db = Some(gain_db);
        self.in_meter_peak_db = meter_peaks;
        self
    }

    /// Add output gain (dB) control and optional stereo peak metering [L, R].
    #[must_use]
    pub fn out_gain(mut self, gain_db: &'a mut f32, meter_peaks: Option<[f32; 2]>) -> Self {
        self.out_gain_db = Some(gain_db);
        self.out_meter_peak_db = meter_peaks;
        self
    }
}

impl<'a> Widget for PluginHeaderBar<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;

        let frame = egui::Frame::NONE
            .fill(palette.surface)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(10, 6));

        let frame_res = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;

                // 1. Global Bypass / Power Switch
                let power_active = !*self.bypassed;
                let btn_text = if power_active { "ON" } else { "BYP" };
                let power_color = if power_active {
                    palette.meter_ok
                } else {
                    palette.destructive
                };
                let power_btn = ui.add(
                    egui::Button::new(
                        RichText::new(btn_text)
                            .font(egui::FontId::proportional(10.0))
                            .strong()
                            .color(power_color),
                    )
                    .fill(with_alpha(power_color, 0.14))
                    .stroke(Stroke::new(1.0_f32, with_alpha(power_color, 0.55)))
                    .min_size(Vec2::new(32.0, 20.0)),
                );

                if power_btn.clicked() {
                    *self.bypassed = !*self.bypassed;
                }

                // 2. Plugin Title
                ui.label(
                    RichText::new(self.title)
                        .font(egui::FontId::proportional(12.0))
                        .strong()
                        .color(palette.foreground),
                );

                ui.separator();

                // 3. A/B Comparison Buttons
                if let Some(ab) = self.ab_state {
                    let is_a = *ab == AbState::A;
                    let a_btn = ui.selectable_label(is_a, "A");
                    let b_btn = ui.selectable_label(!is_a, "B");
                    if a_btn.clicked() {
                        *ab = AbState::A;
                    }
                    if b_btn.clicked() {
                        *ab = AbState::B;
                    }

                    ui.separator();
                }

                // 4. Oversampling selector
                if let Some(os) = self.oversampling {
                    ui.label(
                        RichText::new("HQ:")
                            .font(egui::FontId::proportional(10.0))
                            .color(palette.text_dim),
                    );

                    for opt in Oversampling::ALL {
                        let active = *os == opt;
                        if ui.selectable_label(active, opt.label()).clicked() {
                            *os = opt;
                        }
                    }

                    ui.separator();
                }

                // Push I/O gain & meters to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Output Gain & Meter
                    if let Some(out_g) = self.out_gain_db {
                        if let Some(peaks) = self.out_meter_peak_db {
                            draw_mini_stereo_meter(ui, peaks);
                        }
                        ui.add(
                            egui::DragValue::new(out_g)
                                .speed(0.1)
                                .range(-36.0..=12.0)
                                .suffix(" dB")
                                .custom_formatter(|v, _| format!("Out: {:+.1}", v)),
                        );
                    }

                    // Input Gain & Meter
                    if let Some(in_g) = self.in_gain_db {
                        if let Some(peaks) = self.in_meter_peak_db {
                            draw_mini_stereo_meter(ui, peaks);
                        }
                        ui.add(
                            egui::DragValue::new(in_g)
                                .speed(0.1)
                                .range(-36.0..=12.0)
                                .suffix(" dB")
                                .custom_formatter(|v, _| format!("In: {:+.1}", v)),
                        );
                    }
                });
            });
        });

        frame_res.response
    }
}

/// Draw a compact stereo dual-bar mini peak meter.
fn draw_mini_stereo_meter(ui: &mut Ui, peaks_db: [f32; 2]) {
    let theme = Theme::of(ui);
    let palette = &theme.palette;
    let size = Vec2::new(12.0, 18.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(
        rect,
        CornerRadius::same(2),
        with_alpha(palette.background, 0.9),
    );

    let bar_w = (rect.width() - 3.0) / 2.0;

    for (ch, peak) in peaks_db.iter().enumerate() {
        let norm = ((peak - SILENCE_DB) / (0.0 - SILENCE_DB)).clamp(0.0, 1.0);
        let bar_h = rect.height() * norm;
        let x = rect.left() + 1.0 + ch as f32 * (bar_w + 1.0);
        let bar_rect = Rect::from_min_size(
            Pos2::new(x, rect.bottom() - bar_h),
            Vec2::new(bar_w, bar_h),
        );

        let color = if *peak > 0.0 {
            palette.meter_clip
        } else if *peak > -6.0 {
            palette.meter_warn
        } else {
            palette.meter_ok
        };

        painter.rect_filled(bar_rect, CornerRadius::same(1), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oversampling_all() {
        assert_eq!(Oversampling::ALL.len(), 4);
        assert_eq!(Oversampling::X4.label(), "4x");
    }

    #[test]
    fn test_header_bar_instantiation() {
        let mut bypassed = false;
        let mut ab = AbState::A;
        let mut os = Oversampling::X2;
        let mut in_g = 0.0;
        let mut out_g = -1.5;

        let bar = PluginHeaderBar::new("Test Plugin", &mut bypassed)
            .ab_state(&mut ab)
            .oversampling(&mut os)
            .in_gain(&mut in_g, Some([-12.0, -10.0]))
            .out_gain(&mut out_g, Some([-6.0, -6.5]));

        assert_eq!(bar.title, "Test Plugin");
        assert!(!*bar.bypassed);
    }
}
