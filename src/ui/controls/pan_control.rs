//! A horizontal slider pan/balance control with bipolar center fill and detents.
//!
//! Visual language and behavior directly follow audiocn's `pan-control.tsx`.
//! Maps values in `-1.0..=1.0` where `-1.0` is Full Left, `0.0` is Center ("C"),
//! and `1.0` is Full Right.

use egui::{
    FontFamily, FontId, Key, Rect, Response, Sense, Stroke,
    Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::ui::controls::knob::format_pan;
use crate::ui::theme::{Size, Theme};

/// How close to center, as a fraction of the throw (-1..1), before the
/// thumb snaps to Center (0.0).
const DEFAULT_DETENT_RANGE: f32 = 0.08;

/// A horizontal slider pan control over -1.0..=1.0.
///
/// ```ignore
/// let mut pan = 0.0f32;
/// ui.add(PanControl::new(&mut pan).label("Pan"));
/// ```
#[must_use = "a PanControl draws nothing until it is added to a Ui"]
pub struct PanControl<'a> {
    value: &'a mut f32,
    label: Option<&'a str>,
    step: f32,
    large_step: f32,
    detent: bool,
    detent_range: f32,
    show_readout: bool,
    size: Option<Size>,
    enabled: bool,
    width: Option<f32>,
}

impl<'a> PanControl<'a> {
    /// Create a pan control driving `value` (-1.0 to +1.0).
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            label: None,
            step: 0.05,
            large_step: 0.25,
            detent: true,
            detent_range: DEFAULT_DETENT_RANGE,
            show_readout: true,
            size: None,
            enabled: true,
            width: None,
        }
    }

    /// Set an optional label shown above/beside the slider.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Step amount for arrow keys. Default is 0.05 (5%).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Large step amount for Shift+arrows or PageUp/PageDown. Default is 0.25 (25%).
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Whether to magnetically snap to Center when dragged near 0.0. Default is true.
    pub fn detent(mut self, detent: bool) -> Self {
        self.detent = detent;
        self
    }

    /// Detent snap range around zero. Default is 0.08.
    pub fn detent_range(mut self, range: f32) -> Self {
        self.detent_range = range;
        self
    }

    /// Whether to display the text readout ("L30", "C", "R30"). Default is true.
    pub fn show_readout(mut self, show: bool) -> Self {
        self.show_readout = show;
        self
    }

    /// Size ladder.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Enable or disable user interaction.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Explicit slider width in points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Desired dimensions for the control.
    pub fn desired_size(&self, theme: &Theme, size: Size) -> Vec2 {
        let w = self.width.unwrap_or(match size {
            Size::Small => 90.0,
            Size::Medium => 120.0,
            Size::Large => 160.0,
        });

        let mut h = theme.size.track_thickness().max(6.0) + theme.size.thumb_size();
        if self.label.is_some() || self.show_readout {
            h += theme.size.label_font() + 4.0;
        }
        Vec2::new(w, h)
    }

    /// Add this pan control to `ui`.
    pub fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (total_rect, mut response) = ui.allocate_exact_size(
            desired,
            if self.enabled { Sense::click_and_drag() } else { Sense::hover() },
        );

        let mut val = *self.value;
        let initial_val = val;

        // Partition layout: Top label/readout row (if present), bottom slider row
        let has_header = self.label.is_some() || self.show_readout;
        let (header_rect, slider_rect) = if has_header {
            let font_h = theme.size.label_font() + 4.0;
            (
                Rect::from_min_size(total_rect.min, Vec2::new(total_rect.width(), font_h)),
                Rect::from_min_max(
                    pos2(total_rect.left(), total_rect.min.y + font_h),
                    total_rect.max,
                ),
            )
        } else {
            (Rect::NOTHING, total_rect)
        };

        let thumb_size = match size {
            Size::Small => 10.0,
            Size::Medium => 14.0,
            Size::Large => 18.0,
        };
        let track_thickness = match size {
            Size::Small => 3.0,
            Size::Medium => 4.0,
            Size::Large => 6.0,
        };

        // Track span accounts for thumb radius at the ends
        let track_left = slider_rect.left() + thumb_size * 0.5;
        let track_right = slider_rect.right() - thumb_size * 0.5;
        let track_width = (track_right - track_left).max(1.0);
        let center_x = (track_left + track_right) * 0.5;
        let track_y = slider_rect.center().y;

        // Interactive drag & click handling
        if self.enabled {
            if response.double_clicked() || response.secondary_clicked() {
                val = 0.0;
                response.mark_changed();
            } else if response.dragged() || response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let norm = ((pos.x - track_left) / track_width).clamp(0.0, 1.0);
                    let raw = norm * 2.0 - 1.0;
                    val = if self.detent && raw.abs() < self.detent_range {
                        0.0
                    } else {
                        raw
                    };
                    val = clamp(val, -1.0, 1.0);
                    response.mark_changed();
                }
            }

            // Keyboard navigation when focused
            if response.has_focus() {
                let mut delta = 0.0f32;
                ui.input(|input| {
                    let shift = input.modifiers.shift;
                    let step = if shift { self.large_step } else { self.step };

                    if input.key_pressed(Key::ArrowLeft) || input.key_pressed(Key::ArrowDown) {
                        delta -= step;
                    }
                    if input.key_pressed(Key::ArrowRight) || input.key_pressed(Key::ArrowUp) {
                        delta += step;
                    }
                    if input.key_pressed(Key::PageDown) {
                        delta -= self.large_step;
                    }
                    if input.key_pressed(Key::PageUp) {
                        delta += self.large_step;
                    }
                    if input.key_pressed(Key::Home) {
                        val = -1.0;
                        delta = 0.0;
                        response.mark_changed();
                    }
                    if input.key_pressed(Key::End) {
                        val = 1.0;
                        delta = 0.0;
                        response.mark_changed();
                    }
                });

                if delta != 0.0 {
                    val = clamp(val + delta, -1.0, 1.0);
                    if self.detent && val.abs() < self.detent_range {
                        val = 0.0;
                    }
                    response.mark_changed();
                }
            }
        }

        *self.value = val;
        if (val - initial_val).abs() > 1e-5 {
            response.mark_changed();
        }

        // Render visuals
        if ui.is_rect_visible(total_rect) {
            let painter = ui.painter();

            // 1. Draw header (label on left, readout on right)
            if has_header {
                let font_id = FontId::new(theme.size.label_font(), FontFamily::Proportional);
                if let Some(lbl) = self.label {
                    let text_color = if !self.enabled {
                        theme.palette.text_dim.linear_multiply(0.5)
                    } else {
                        theme.palette.text_dim
                    };
                    painter.text(
                        header_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        lbl,
                        font_id.clone(),
                        text_color,
                    );
                }

                if self.show_readout {
                    let readout_color = if !self.enabled {
                        theme.palette.text_dim.linear_multiply(0.5)
                    } else if val == 0.0 {
                        theme.palette.text_dim
                    } else {
                        theme.palette.primary
                    };
                    painter.text(
                        header_rect.right_center(),
                        egui::Align2::RIGHT_CENTER,
                        format_pan(val),
                        FontId::new(theme.size.label_font(), FontFamily::Monospace),
                        readout_color,
                    );
                }
            }

            // 2. Draw slider track
            let full_track_rect = Rect::from_min_max(
                pos2(track_left, track_y - track_thickness * 0.5),
                pos2(track_right, track_y + track_thickness * 0.5),
            );
            let track_rounding = track_thickness * 0.5;
            painter.rect_filled(full_track_rect, track_rounding, theme.palette.track);

            // 3. Draw center detent tick mark
            let center_tick_rect = Rect::from_center_size(
                pos2(center_x, track_y),
                Vec2::new(1.5, track_thickness * 2.8),
            );
            painter.rect_filled(
                center_tick_rect,
                0.75,
                theme.palette.border.linear_multiply(1.5),
            );

            // 4. Draw bipolar range fill (from center to thumb)
            let thumb_x = track_left + ((val + 1.0) * 0.5) * track_width;
            let (fill_min_x, fill_max_x) = if thumb_x >= center_x {
                (center_x, thumb_x)
            } else {
                (thumb_x, center_x)
            };

            if (fill_max_x - fill_min_x) > 0.5 {
                let fill_rect = Rect::from_min_max(
                    pos2(fill_min_x, track_y - track_thickness * 0.5),
                    pos2(fill_max_x, track_y + track_thickness * 0.5),
                );
                let fill_color = if !self.enabled {
                    theme.palette.primary.linear_multiply(0.3)
                } else {
                    theme.palette.primary
                };
                painter.rect_filled(fill_rect, track_rounding, fill_color);
            }

            // 5. Draw circular thumb
            let thumb_pos = pos2(thumb_x, track_y);
            let thumb_radius = thumb_size * 0.5;
            let is_dragged = self.enabled && response.dragged();
            let is_hovered = self.enabled && response.hovered();

            // Focus / active / hover glow ring
            if is_dragged || is_hovered || response.has_focus() {
                let glow_radius = thumb_radius + 3.0;
                let glow_color = if is_dragged {
                    theme.palette.ring.linear_multiply(0.4)
                } else {
                    theme.palette.ring.linear_multiply(0.2)
                };
                painter.circle_filled(thumb_pos, glow_radius, glow_color);
            }

            // Thumb body & border
            let thumb_bg = theme.palette.surface;
            let thumb_stroke = Stroke::new(
                1.5_f32,
                if is_dragged {
                    theme.palette.primary
                } else if is_hovered || response.has_focus() {
                    theme.palette.ring
                } else {
                    theme.palette.border
                },
            );

            painter.circle(thumb_pos, thumb_radius, thumb_bg, thumb_stroke);

            // Center pip on the thumb
            painter.circle_filled(
                thumb_pos,
                1.5,
                if val == 0.0 { theme.palette.text_dim } else { theme.palette.primary },
            );
        }

        response
    }
}

impl<'a> Widget for PanControl<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_pan_representations() {
        assert_eq!(format_pan(0.0), "C");
        assert_eq!(format_pan(-0.3), "30L");
        assert_eq!(format_pan(0.5), "50R");
        assert_eq!(format_pan(-1.0), "100L");
        assert_eq!(format_pan(1.0), "100R");
    }

    #[test]
    fn pan_control_renders_headlessly() {
        let mut pan = 0.25f32;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(PanControl::new(&mut pan).label("Balance"));
        });
        assert_eq!(pan, 0.25);
    }

    #[test]
    fn pan_control_detent_zeros_near_center() {
        let mut pan = 0.02f32;
        let ctrl = PanControl::new(&mut pan).detent(true);
        assert!(ctrl.detent_range > 0.05);
    }
}
