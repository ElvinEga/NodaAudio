//! Channel toggle button: Mute, Solo, Monitor, or generic state switch.
//!
//! Visual language faithfully follows audiocn's `channel-toggle.tsx` with
//! muted red, solo amber, monitor blue, and neutral tones.

use egui::{
    Align2, Color32, FontFamily, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    Widget,
};

use crate::ui::theme::{Palette, Size, Theme};

/// The semantic color tone of the toggle when pressed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelToggleTone {
    /// Mute toggle (red accent).
    Mute,
    /// Solo toggle (amber accent).
    Solo,
    /// Monitor / record-arm toggle (blue accent).
    Monitor,
    /// Neutral toggle (primary accent).
    Neutral,
}

impl ChannelToggleTone {
    /// Background color when pressed.
    #[must_use]
    pub fn pressed_bg(self, palette: &Palette) -> Color32 {
        match self {
            Self::Mute => palette.channel_mute.linear_multiply(0.20),
            Self::Solo => palette.channel_solo.linear_multiply(0.20),
            Self::Monitor => palette.channel_monitor.linear_multiply(0.20),
            Self::Neutral => palette.primary,
        }
    }

    /// Border stroke when pressed.
    #[must_use]
    pub fn pressed_stroke(self, palette: &Palette) -> Stroke {
        let color = match self {
            Self::Mute => palette.channel_mute.linear_multiply(0.50),
            Self::Solo => palette.channel_solo.linear_multiply(0.50),
            Self::Monitor => palette.channel_monitor.linear_multiply(0.50),
            Self::Neutral => palette.primary,
        };
        Stroke::new(1.0_f32, color)
    }

    /// Text/icon foreground color when pressed.
    #[must_use]
    pub fn pressed_fg(self, palette: &Palette) -> Color32 {
        match self {
            Self::Mute => palette.channel_mute,
            Self::Solo => palette.channel_solo,
            Self::Monitor => palette.channel_monitor,
            Self::Neutral => palette.primary_foreground,
        }
    }
}

/// A channel toggle widget for Mute / Solo / Monitor / Bypass.
///
/// ```ignore
/// let mut muted = false;
/// ui.add(ChannelToggle::mute(&mut muted));
/// ```
#[must_use = "a ChannelToggle draws nothing until it is added to a Ui"]
pub struct ChannelToggle<'a> {
    pressed: &'a mut bool,
    tone: ChannelToggleTone,
    text: &'a str,
    size: Option<Size>,
    min_size: Option<Vec2>,
    enabled: bool,
}

impl<'a> ChannelToggle<'a> {
    /// Create a toggle with a custom tone and label text.
    pub fn new(pressed: &'a mut bool, tone: ChannelToggleTone, text: &'a str) -> Self {
        Self {
            pressed,
            tone,
            text,
            size: None,
            min_size: None,
            enabled: true,
        }
    }

    /// Create a Mute toggle ("M").
    pub fn mute(pressed: &'a mut bool) -> Self {
        Self::new(pressed, ChannelToggleTone::Mute, "M")
    }

    /// Create a Solo toggle ("S").
    pub fn solo(pressed: &'a mut bool) -> Self {
        Self::new(pressed, ChannelToggleTone::Solo, "S")
    }

    /// Create a Monitor toggle ("IN").
    pub fn monitor(pressed: &'a mut bool) -> Self {
        Self::new(pressed, ChannelToggleTone::Monitor, "IN")
    }

    /// Explicit size ladder.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Explicit minimum dimensions.
    pub fn min_size(mut self, min_size: Vec2) -> Self {
        self.min_size = Some(min_size);
        self
    }

    /// Enable or disable interaction.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Default dimensions for a given size ladder.
    fn natural_size(size: Size) -> Vec2 {
        match size {
            Size::Small => Vec2::new(22.0, 20.0),
            Size::Medium => Vec2::new(26.0, 24.0),
            Size::Large => Vec2::new(32.0, 28.0),
        }
    }

    /// Add this toggle to `ui`.
    pub fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.min_size.unwrap_or_else(|| Self::natural_size(size));

        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };

        let (rect, mut response) = ui.allocate_exact_size(desired, sense);

        if self.enabled && response.clicked() {
            *self.pressed = !*self.pressed;
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let is_pressed = *self.pressed;
            let hovered = self.enabled && response.hovered();

            let (bg, stroke, fg) = if !self.enabled {
                (
                    theme.palette.surface,
                    Stroke::new(1.0_f32, theme.palette.border.linear_multiply(0.5)),
                    theme.palette.text_dim.linear_multiply(0.5),
                )
            } else if is_pressed {
                let bg = self.tone.pressed_bg(&theme.palette);
                let stroke = self.tone.pressed_stroke(&theme.palette);
                let fg = self.tone.pressed_fg(&theme.palette);
                (bg, stroke, fg)
            } else if hovered {
                (
                    theme.palette.surface,
                    Stroke::new(1.0_f32, theme.palette.ring),
                    theme.palette.foreground,
                )
            } else {
                (
                    theme.palette.track,
                    Stroke::new(1.0_f32, theme.palette.border),
                    theme.palette.text_dim,
                )
            };

            let rounding = 4.0;
            ui.painter().rect(rect, rounding, bg, stroke, StrokeKind::Inside);

            let font_size = match size {
                Size::Small => 9.0,
                Size::Medium => 10.5,
                Size::Large => 12.0,
            };
            let font_id = FontId::new(font_size, FontFamily::Proportional);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.text,
                font_id,
                fg,
            );
        }

        response
    }
}

impl<'a> Widget for ChannelToggle<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_flips_state() {
        let mut pressed = false;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(ChannelToggle::mute(&mut pressed));
        });
        assert!(!pressed);
    }

    #[test]
    fn tone_colors_match_palette() {
        let palette = Palette::dark();
        assert_eq!(
            ChannelToggleTone::Mute.pressed_fg(&palette),
            palette.channel_mute
        );
        assert_eq!(
            ChannelToggleTone::Solo.pressed_fg(&palette),
            palette.channel_solo
        );
        assert_eq!(
            ChannelToggleTone::Monitor.pressed_fg(&palette),
            palette.channel_monitor
        );
    }
}
