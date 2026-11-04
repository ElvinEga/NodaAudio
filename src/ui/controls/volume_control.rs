//! A volume control slider with integrated mute button (0.0..1.0 gain).
//!
//! Directly inspired by audiocn's `volume-control.tsx`. Uses normalized 0..1
//! gain for players and media clips, with optional perceptual taper and mute restore.

use egui::{
    Color32, FontFamily, FontId, Key, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::ui::Orientation;
use crate::ui::theme::{Size, Theme};

/// Perceptual curve exponent for volume travel.
const PERCEPTUAL_EXPONENT: f32 = 2.0;

/// Semantic volume level classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeLevel {
    /// Muted or zero volume.
    Muted,
    /// Below 34%.
    Low,
    /// Between 34% and 67%.
    Medium,
    /// Above 67%.
    High,
}

impl VolumeLevel {
    /// Determine the volume level category.
    #[must_use]
    pub fn for_volume(volume: f32, muted: bool) -> Self {
        if muted || volume <= 0.0 {
            Self::Muted
        } else if volume < 0.34 {
            Self::Low
        } else if volume < 0.67 {
            Self::Medium
        } else {
            Self::High
        }
    }

    /// Symbol / icon character for the level.
    #[must_use]
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Muted => "🔇",
            Self::Low => "🔈",
            Self::Medium => "🔉",
            Self::High => "🔊",
        }
    }
}

/// Curve mapping volume (0..1) to slider position and back.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VolumeCurve {
    /// Linear: position == volume.
    Linear,
    /// Perceptual: position == volume^(1/2).
    #[default]
    Perceptual,
}

impl VolumeCurve {
    /// Volume 0..1 to position 0..1.
    #[must_use]
    pub fn to_position(self, volume: f32) -> f32 {
        let v = clamp(volume, 0.0, 1.0);
        match self {
            Self::Linear => v,
            Self::Perceptual => v.powf(1.0 / PERCEPTUAL_EXPONENT),
        }
    }

    /// Position 0..1 to volume 0..1.
    #[must_use]
    pub fn to_volume(self, position: f32) -> f32 {
        let p = clamp(position, 0.0, 1.0);
        match self {
            Self::Linear => p,
            Self::Perceptual => p.powf(PERCEPTUAL_EXPONENT),
        }
    }
}

/// Persistent memory of the last non-zero volume so unmuting restores it.
#[derive(Clone, Copy, Debug)]
struct VolumeControlState {
    last_unmuted: f32,
}

impl Default for VolumeControlState {
    fn default() -> Self {
        Self { last_unmuted: 1.0 }
    }
}

/// A volume slider with an optional mute button and perceptual curve.
///
/// ```ignore
/// let mut vol = 0.8f32;
/// let mut muted = false;
/// ui.add(VolumeControl::new(&mut vol).mute(&mut muted));
/// ```
#[must_use = "a VolumeControl draws nothing until it is added to a Ui"]
pub struct VolumeControl<'a> {
    volume: &'a mut f32,
    muted: Option<&'a mut bool>,
    curve: VolumeCurve,
    orientation: Orientation,
    size: Option<Size>,
    show_readout: bool,
    enabled: bool,
    width: Option<f32>,
}

impl<'a> VolumeControl<'a> {
    /// Create a volume control driving `volume` in 0.0..=1.0.
    pub fn new(volume: &'a mut f32) -> Self {
        Self {
            volume,
            muted: None,
            curve: VolumeCurve::Perceptual,
            orientation: Orientation::Horizontal,
            size: None,
            show_readout: true,
            enabled: true,
            width: None,
        }
    }

    /// Bind a mute boolean state.
    pub fn mute(mut self, muted: &'a mut bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Taper curve (Perceptual or Linear).
    pub fn curve(mut self, curve: VolumeCurve) -> Self {
        self.curve = curve;
        self
    }

    /// Orientation (Horizontal or Vertical).
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Size ladder.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Whether to display percentage readout (e.g. "80%").
    pub fn show_readout(mut self, show: bool) -> Self {
        self.show_readout = show;
        self
    }

    /// Enable or disable interaction.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Explicit width in points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let default_w = match size {
            Size::Small => 100.0,
            Size::Medium => 140.0,
            Size::Large => 180.0,
        };
        let w = self.width.unwrap_or(default_w);
        let h = match size {
            Size::Small => 22.0,
            Size::Medium => 26.0,
            Size::Large => 32.0,
        };
        Vec2::new(w, h)
    }

    /// Add this volume control to `ui`.
    pub fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let id = ui.make_persistent_id("volume_control_state");
        let mut state = ui
            .ctx()
            .data_mut(|data| data.get_temp::<VolumeControlState>(id))
            .unwrap_or_default();

        let (total_rect, mut response) = ui.allocate_exact_size(
            desired,
            if self.enabled { Sense::hover() } else { Sense::hover() },
        );

        let is_muted = self.muted.as_ref().map_or(false, |m| **m);
        let current_vol = *self.volume;
        let level = VolumeLevel::for_volume(current_vol, is_muted);

        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(total_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        let has_mute = self.muted.is_some();
        let btn_size = Vec2::new(desired.y, desired.y);

        // 1. Mute Button
        if let Some(muted_ref) = self.muted {
            let mute_response = child_ui.allocate_response(
                btn_size,
                if self.enabled { Sense::click() } else { Sense::hover() },
            );

            if self.enabled && mute_response.clicked() {
                if *muted_ref {
                    // Unmuting: restore last non-zero volume
                    *muted_ref = false;
                    *self.volume = state.last_unmuted.max(0.1);
                } else {
                    // Muting: remember current volume
                    if *self.volume > 0.0 {
                        state.last_unmuted = *self.volume;
                    }
                    *muted_ref = true;
                }
                response.mark_changed();
            }

            if child_ui.is_rect_visible(mute_response.rect) {
                let painter = child_ui.painter();
                let bg = if *muted_ref {
                    theme.palette.channel_mute.linear_multiply(0.2)
                } else if mute_response.hovered() && self.enabled {
                    theme.palette.track
                } else {
                    Color32::TRANSPARENT
                };
                let stroke = if *muted_ref {
                    Stroke::new(1.0_f32, theme.palette.channel_mute.linear_multiply(0.5))
                } else {
                    Stroke::NONE
                };

                painter.rect(mute_response.rect, 4.0, bg, stroke, StrokeKind::Inside);

                let icon_text = level.icon();
                painter.text(
                    mute_response.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    icon_text,
                    FontId::proportional(theme.size.label_font() + 1.0),
                    if *muted_ref {
                        theme.palette.channel_mute
                    } else {
                        theme.palette.foreground
                    },
                );
            }
        }

        // 2. Readout width calculation (if shown)
        let readout_w = if self.show_readout {
            let font = FontId::new(theme.size.label_font(), FontFamily::Monospace);
            child_ui.painter().layout_no_wrap("100%".to_owned(), font, Color32::WHITE).size().x + 6.0
        } else {
            0.0
        };

        // 3. Slider Track
        let mute_space = if has_mute { btn_size.x + 4.0 } else { 0.0 };
        let available_track_w = (total_rect.width() - mute_space - readout_w).max(20.0);
        let slider_rect = Rect::from_min_size(
            pos2(total_rect.left() + mute_space, total_rect.min.y),
            Vec2::new(available_track_w, total_rect.height()),
        );

        let slider_response = child_ui.interact(
            slider_rect,
            id.with("slider"),
            if self.enabled { Sense::click_and_drag() } else { Sense::hover() },
        );

        let thumb_size = match size {
            Size::Small => 10.0,
            Size::Medium => 12.0,
            Size::Large => 16.0,
        };
        let track_thickness = match size {
            Size::Small => 3.0,
            Size::Medium => 4.0,
            Size::Large => 5.0,
        };

        let track_left = slider_rect.left() + thumb_size * 0.5;
        let track_right = slider_rect.right() - thumb_size * 0.5;
        let track_width = (track_right - track_left).max(1.0);
        let track_y = slider_rect.center().y;

        let mut val = *self.volume;

        if self.enabled {
            if slider_response.dragged() || slider_response.clicked() {
                if let Some(pos) = slider_response.interact_pointer_pos() {
                    let pos_norm = ((pos.x - track_left) / track_width).clamp(0.0, 1.0);
                    val = self.curve.to_volume(pos_norm);
                    if val > 0.0 {
                        state.last_unmuted = val;
                    }
                    *self.volume = val;
                    response.mark_changed();
                }
            }

            if slider_response.has_focus() {
                let mut delta = 0.0f32;
                child_ui.input(|input| {
                    if input.key_pressed(Key::ArrowLeft) || input.key_pressed(Key::ArrowDown) {
                        delta -= 0.05;
                    }
                    if input.key_pressed(Key::ArrowRight) || input.key_pressed(Key::ArrowUp) {
                        delta += 0.05;
                    }
                    if input.key_pressed(Key::Home) {
                        val = 0.0;
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
                    val = clamp(val + delta, 0.0, 1.0);
                    if val > 0.0 {
                        state.last_unmuted = val;
                    }
                    *self.volume = val;
                    response.mark_changed();
                }
            }
        }

        // Render slider visuals
        if child_ui.is_rect_visible(slider_rect) {
            let painter = child_ui.painter();

            // Track background
            let track_rect = Rect::from_min_max(
                pos2(track_left, track_y - track_thickness * 0.5),
                pos2(track_right, track_y + track_thickness * 0.5),
            );
            painter.rect_filled(track_rect, track_thickness * 0.5, theme.palette.track);

            // Filled range
            let effective_vol = if is_muted { 0.0 } else { val };
            let pos_norm = self.curve.to_position(effective_vol);
            let thumb_x = track_left + pos_norm * track_width;

            if thumb_x > track_left {
                let fill_rect = Rect::from_min_max(
                    pos2(track_left, track_y - track_thickness * 0.5),
                    pos2(thumb_x, track_y + track_thickness * 0.5),
                );
                let fill_color = if !self.enabled || is_muted {
                    theme.palette.text_dim.linear_multiply(0.4)
                } else {
                    theme.palette.primary
                };
                painter.rect_filled(fill_rect, track_thickness * 0.5, fill_color);
            }

            // Thumb
            let thumb_pos = pos2(thumb_x, track_y);
            let thumb_radius = thumb_size * 0.5;
            let is_dragged = self.enabled && slider_response.dragged();
            let is_hovered = self.enabled && slider_response.hovered();

            if is_dragged || is_hovered || slider_response.has_focus() {
                let glow_radius = thumb_radius + 3.0;
                let glow_color = if is_dragged {
                    theme.palette.ring.linear_multiply(0.4)
                } else {
                    theme.palette.ring.linear_multiply(0.2)
                };
                painter.circle_filled(thumb_pos, glow_radius, glow_color);
            }

            let thumb_stroke = Stroke::new(
                1.0_f32,
                if is_dragged {
                    theme.palette.primary
                } else if is_hovered || slider_response.has_focus() {
                    theme.palette.ring
                } else {
                    theme.palette.border
                },
            );

            painter.circle(thumb_pos, thumb_radius, theme.palette.surface, thumb_stroke);
        }

        // 4. Percentage Readout
        if self.show_readout {
            let readout_rect = Rect::from_min_size(
                pos2(total_rect.right() - readout_w, total_rect.min.y),
                Vec2::new(readout_w, total_rect.height()),
            );
            if child_ui.is_rect_visible(readout_rect) {
                let font = FontId::new(theme.size.label_font(), FontFamily::Monospace);
                let text = if is_muted {
                    "MUTE".to_string()
                } else {
                    format!("{}%", (val * 100.0).round() as i32)
                };
                let color = if is_muted {
                    theme.palette.channel_mute
                } else if !self.enabled {
                    theme.palette.text_dim.linear_multiply(0.5)
                } else {
                    theme.palette.text_dim
                };
                child_ui.painter().text(
                    readout_rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    text,
                    font,
                    color,
                );
            }
        }

        ui.ctx().data_mut(|data| data.insert_temp(id, state));
        response
    }
}

impl<'a> Widget for VolumeControl<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_levels() {
        assert_eq!(VolumeLevel::for_volume(0.0, false), VolumeLevel::Muted);
        assert_eq!(VolumeLevel::for_volume(0.8, true), VolumeLevel::Muted);
        assert_eq!(VolumeLevel::for_volume(0.2, false), VolumeLevel::Low);
        assert_eq!(VolumeLevel::for_volume(0.5, false), VolumeLevel::Medium);
        assert_eq!(VolumeLevel::for_volume(0.9, false), VolumeLevel::High);
    }

    #[test]
    fn volume_curve_round_trip() {
        let curve = VolumeCurve::Perceptual;
        let pos = curve.to_position(0.25);
        assert!((curve.to_volume(pos) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn volume_control_renders() {
        let mut vol = 0.5f32;
        let mut muted = false;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(VolumeControl::new(&mut vol).mute(&mut muted));
        });
        assert_eq!(vol, 0.5);
    }
}
