//! A labelled parameter slider with numeric text input, custom units, and tapers.
//!
//! Inspired by audiocn's `parameter-slider.tsx`. Used for any audio parameter
//! that is not purely console volume: frequencies, times, percentages, trims, ratios.

use egui::{
    FontFamily, FontId, Key, Rect, Response, Sense, Stroke,
    TextEdit, Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::taper::Taper;
use crate::ui::theme::{Size, Theme};

/// State stored across frames for the parameter slider's inline text editing.
#[derive(Clone, Debug, Default)]
struct ParameterSliderState {
    editing: bool,
    text: String,
}

/// A labelled slider with numeric input and unit suffix.
///
/// ```ignore
/// let mut attack_ms = 15.0f32;
/// ui.add(
///     ParameterSlider::new(&mut attack_ms, 0.1, 500.0)
///         .label("Attack")
///         .unit("ms")
///         .taper(Taper::LOG),
/// );
/// ```
#[must_use = "a ParameterSlider draws nothing until it is added to a Ui"]
pub struct ParameterSlider<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    label: Option<&'a str>,
    unit: Option<&'a str>,
    decimals: usize,
    step: f32,
    large_step: f32,
    reset_value: Option<f32>,
    origin: Option<f32>,
    taper: Taper,
    size: Option<Size>,
    enabled: bool,
    width: Option<f32>,
}

impl<'a> ParameterSlider<'a> {
    /// Create a parameter slider for `value` within `min..=max`.
    pub fn new(value: &'a mut f32, min: f32, max: f32) -> Self {
        let step = (max - min) / 100.0;
        let large_step = step * 5.0;
        Self {
            value,
            min,
            max,
            label: None,
            unit: None,
            decimals: 1,
            step,
            large_step,
            reset_value: None,
            origin: None,
            taper: Taper::Linear,
            size: None,
            enabled: true,
            width: None,
        }
    }

    /// Parameter label (e.g. "Frequency", "Decay", "Drive").
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Unit suffix (e.g. "Hz", "ms", "%", "dB").
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Digits after decimal point for display.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Step amount for arrow keys.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Large step amount for Shift+arrow or PageUp/PageDown.
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Value to restore on double-click.
    pub fn reset_value(mut self, reset: f32) -> Self {
        self.reset_value = Some(reset);
        self
    }

    /// Bipolar range fill origin (default starts at `min`).
    pub fn origin(mut self, origin: f32) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Law mapping position along throw to parameter value.
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
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

    /// Desired dimensions for the widget.
    pub fn desired_size(&self, theme: &Theme, size: Size) -> Vec2 {
        let w = self.width.unwrap_or(match size {
            Size::Small => 120.0,
            Size::Medium => 160.0,
            Size::Large => 200.0,
        });

        let mut h = theme.size.track_thickness().max(6.0) + theme.size.thumb_size();
        if self.label.is_some() || self.unit.is_some() {
            h += theme.size.label_font() + 6.0;
        }
        Vec2::new(w, h)
    }

    /// Format current value as string with unit.
    pub fn formatted_value(&self, val: f32) -> String {
        let num = format!("{:.1$}", val, self.decimals);
        if let Some(unit) = self.unit {
            format!("{num} {unit}")
        } else {
            num
        }
    }

    /// Parse typed string back into value.
    fn parse_input(&self, text: &str) -> Option<f32> {
        let trimmed = text.trim();
        let cleaned = if let Some(unit) = self.unit {
            trimmed.trim_end_matches(unit).trim()
        } else {
            trimmed
        };
        cleaned.parse::<f32>().ok().map(|v| clamp(v, self.min, self.max))
    }

    /// Add this parameter slider to `ui`.
    pub fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (total_rect, mut response) = ui.allocate_exact_size(
            desired,
            if self.enabled { Sense::click_and_drag() } else { Sense::hover() },
        );

        // Keyed on the widget, not the label: two sliders can share a label
        // (one per channel) without sharing their edit state or ids.
        let id = response.id.with("param_slider");
        let mut state = ui
            .ctx()
            .data_mut(|data| data.get_temp::<ParameterSliderState>(id))
            .unwrap_or_default();

        let mut val = *self.value;
        let initial_val = val;

        let has_header = self.label.is_some() || self.unit.is_some();
        let header_h = if has_header { theme.size.label_font() + 6.0 } else { 0.0 };
        let header_rect = Rect::from_min_size(total_rect.min, Vec2::new(total_rect.width(), header_h));
        let slider_rect = Rect::from_min_max(
            pos2(total_rect.left(), total_rect.min.y + header_h),
            total_rect.max,
        );

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

        let track_left = slider_rect.left() + thumb_size * 0.5;
        let track_right = slider_rect.right() - thumb_size * 0.5;
        let track_width = (track_right - track_left).max(1.0);
        let track_y = slider_rect.center().y;

        // Interaction
        if self.enabled {
            if response.double_clicked() || response.secondary_clicked() {
                if let Some(reset) = self.reset_value {
                    val = clamp(reset, self.min, self.max);
                    response.mark_changed();
                }
            } else if response.dragged() || response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let norm = ((pos.x - track_left) / track_width).clamp(0.0, 1.0);
                    val = self.taper.to_value(norm, self.min, self.max);
                    val = clamp(val, self.min, self.max);
                    response.mark_changed();
                }
            }

            // Keyboard navigation
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
                        val = self.min;
                        delta = 0.0;
                        response.mark_changed();
                    }
                    if input.key_pressed(Key::End) {
                        val = self.max;
                        delta = 0.0;
                        response.mark_changed();
                    }
                });

                if delta != 0.0 {
                    val = clamp(val + delta, self.min, self.max);
                    response.mark_changed();
                }
            }
        }

        *self.value = val;
        if (val - initial_val).abs() > 1e-5 {
            response.mark_changed();
        }

        // Render
        if ui.is_rect_visible(total_rect) {
            // 1. Header interaction (Numeric text edit or click to edit)
            let edit_w = 64.0;
            let edit_rect = Rect::from_min_size(
                pos2(header_rect.right() - edit_w, header_rect.min.y),
                Vec2::new(edit_w, header_rect.height()),
            );
            let readout_font = FontId::new(theme.size.label_font(), FontFamily::Monospace);

            if has_header {
                if state.editing {
                    let mut text = state.text.clone();
                    let edit_response = ui.put(
                        edit_rect,
                        TextEdit::singleline(&mut text)
                            .font(readout_font.clone())
                            .horizontal_align(egui::Align::RIGHT)
                            .margin(egui::Margin::symmetric(2, 1)),
                    );

                    if edit_response.lost_focus()
                        || ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape))
                    {
                        if let Some(parsed) = self.parse_input(&text) {
                            *self.value = parsed;
                            response.mark_changed();
                        }
                        state.editing = false;
                    } else {
                        state.text = text;
                    }
                } else {
                    let readout_click = ui.interact(edit_rect, id.with("edit"), Sense::click());
                    if self.enabled && readout_click.clicked() {
                        state.editing = true;
                        state.text = format!("{:.1$}", val, self.decimals);
                    }
                }
            }

            let painter = ui.painter();

            // 2. Header text rendering
            if has_header {
                let font_id = FontId::new(theme.size.label_font(), FontFamily::Proportional);
                if let Some(lbl) = self.label {
                    let label_color = if !self.enabled {
                        theme.palette.text_dim.linear_multiply(0.5)
                    } else {
                        theme.palette.foreground
                    };
                    painter.text(
                        header_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        lbl,
                        font_id,
                        label_color,
                    );
                }

                if !state.editing {
                    let val_text = self.formatted_value(val);
                    let readout_color = if !self.enabled {
                        theme.palette.text_dim.linear_multiply(0.5)
                    } else {
                        theme.palette.primary
                    };
                    painter.text(
                        header_rect.right_center(),
                        egui::Align2::RIGHT_CENTER,
                        val_text,
                        readout_font,
                        readout_color,
                    );
                }
            }

            // 2. Track background
            let full_track_rect = Rect::from_min_max(
                pos2(track_left, track_y - track_thickness * 0.5),
                pos2(track_right, track_y + track_thickness * 0.5),
            );
            let track_rounding = track_thickness * 0.5;
            painter.rect_filled(full_track_rect, track_rounding, theme.palette.track);

            // 3. Range fill
            let pos_norm = self.taper.to_position(val, self.min, self.max);
            let thumb_x = track_left + pos_norm * track_width;

            let origin_val = self.origin.unwrap_or(self.min);
            let origin_norm = self.taper.to_position(origin_val, self.min, self.max);
            let origin_x = track_left + origin_norm * track_width;

            let (fill_min_x, fill_max_x) = if thumb_x >= origin_x {
                (origin_x, thumb_x)
            } else {
                (thumb_x, origin_x)
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

            // 4. Center or Origin detent marker if origin is not min
            if self.origin.is_some() {
                let origin_marker = Rect::from_center_size(
                    pos2(origin_x, track_y),
                    Vec2::new(1.5, track_thickness * 2.5),
                );
                painter.rect_filled(origin_marker, 0.75, theme.palette.border.linear_multiply(1.5));
            }

            // 5. Thumb
            let thumb_pos = pos2(thumb_x, track_y);
            let thumb_radius = thumb_size * 0.5;
            let is_dragged = self.enabled && response.dragged();
            let is_hovered = self.enabled && response.hovered();

            if is_dragged || is_hovered || response.has_focus() {
                let glow_radius = thumb_radius + 3.0;
                let glow_color = if is_dragged {
                    theme.palette.ring.linear_multiply(0.4)
                } else {
                    theme.palette.ring.linear_multiply(0.2)
                };
                painter.circle_filled(thumb_pos, glow_radius, glow_color);
            }

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
        }

        ui.ctx().data_mut(|data| data.insert_temp(id, state));
        response
    }
}

impl<'a> Widget for ParameterSlider<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_parameter_values() {
        let mut val = 440.0;
        let slider = ParameterSlider::new(&mut val, 20.0, 20000.0)
            .unit("Hz")
            .decimals(0);
        assert_eq!(slider.formatted_value(440.0), "440 Hz");
        assert_eq!(slider.parse_input("1000 Hz"), Some(1000.0));
        assert_eq!(slider.parse_input("50"), Some(50.0));
    }

    #[test]
    fn parameter_slider_renders_headlessly() {
        let mut val = 10.0f32;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                ParameterSlider::new(&mut val, 0.0, 100.0)
                    .label("Mix")
                    .unit("%"),
            );
        });
        assert_eq!(val, 10.0);
    }
}
