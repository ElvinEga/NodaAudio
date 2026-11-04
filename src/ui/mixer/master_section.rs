//! The master output section: the desk's final trim, metering and monitors.
//!
//! Wider than a channel strip and meant to sit across the bottom of a
//! console: a large gain readout, a stereo meter with scale, one fader and
//! the master mute, dim and mono switches.

use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use crate::ui::Orientation;
use crate::ui::controls::channel_toggle::{ChannelToggle, ChannelToggleTone};
use crate::ui::controls::fader::Fader;
use crate::ui::meters::db_readout::DbReadout;
use crate::ui::meters::level_meter::{ChannelLevel, LevelMeter, MeterStyle};
use crate::ui::theme::Theme;

/// Default width of the master section.
const DEFAULT_WIDTH: f32 = 640.0;

/// Default height of the master section.
///
/// Tall enough for a meter with clip lamps, a scale and a numeric peak
/// readout to breathe; a shorter panel crowds the scale's lowest label.
const DEFAULT_HEIGHT: f32 = 152.0;

/// The final stereo output of a mixer: gain, metering and monitoring.
///
/// ```ignore
/// ui.add(
///     MasterSection::new(&mut master_gain_db)
///         .mute(&mut master_muted)
///         .dim(&mut dimmed)
///         .levels(&master_levels),
/// );
/// ```
#[must_use = "a MasterSection draws nothing until it is added to a Ui"]
pub struct MasterSection<'a> {
    title: &'a str,
    subtitle: &'a str,
    gain_db: &'a mut f32,
    muted: Option<&'a mut bool>,
    dim: Option<&'a mut bool>,
    mono: Option<&'a mut bool>,
    levels: &'a [ChannelLevel],
    min_db: f32,
    max_db: f32,
    accent: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
}

impl<'a> MasterSection<'a> {
    /// Create a master section driving the output gain in dB.
    pub fn new(gain_db: &'a mut f32) -> Self {
        Self {
            title: "Master",
            subtitle: "Main output",
            gain_db,
            muted: None,
            dim: None,
            mono: None,
            levels: &[],
            min_db: -60.0,
            max_db: 6.0,
            accent: None,
            width: None,
            height: None,
        }
    }

    /// Heading shown at the top left.
    #[must_use]
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Caption under the heading.
    #[must_use]
    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = subtitle;
        self
    }

    /// Bind the master mute.
    #[must_use]
    pub fn mute(mut self, muted: &'a mut bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Bind the dim switch.
    #[must_use]
    pub fn dim(mut self, dim: &'a mut bool) -> Self {
        self.dim = Some(dim);
        self
    }

    /// Bind the mono switch.
    #[must_use]
    pub fn mono(mut self, mono: &'a mut bool) -> Self {
        self.mono = Some(mono);
        self
    }

    /// Post-gain stereo levels for the meter.
    #[must_use]
    pub fn levels(mut self, levels: &'a [ChannelLevel]) -> Self {
        self.levels = levels;
        self
    }

    /// The dB range the fader covers. Default -60..6 dB.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Accent for the edge bar. Default the theme's primary.
    #[must_use]
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Explicit width. Default 640 points.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(480.0));
        self
    }

    /// Explicit height. Default 152 points.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(116.0));
        self
    }
}

impl Widget for MasterSection<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let accent = self.accent.unwrap_or(palette.primary);
        let muted = self.muted.as_ref().is_some_and(|muted| **muted);

        let (rect, mut response) = ui.allocate_exact_size(
            Vec2::new(
                self.width.unwrap_or(DEFAULT_WIDTH),
                self.height.unwrap_or(DEFAULT_HEIGHT),
            ),
            Sense::hover(),
        );
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter_at(rect);
        painter.rect(
            rect,
            theme.radius.md(),
            palette.surface,
            Stroke::new(1.2_f32, palette.border),
            StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 10.0, rect.top() + 14.0),
                Vec2::new(3.0, rect.height() - 28.0),
            ),
            1.5,
            if muted {
                accent.linear_multiply(0.35)
            } else {
                accent
            },
        );

        let pad = 14.0;
        let mut right = rect.right() - pad;

        // Monitor switches, stacked at the right edge.
        let toggle_size = Vec2::new(54.0, 22.0);
        let mut toggles = Vec::new();
        if let Some(mono) = self.mono.as_deref_mut() {
            let toggle_rect = Rect::from_center_size(
                Pos2::new(right - toggle_size.x / 2.0, rect.center().y + 27.0),
                toggle_size,
            );
            toggles.push((
                toggle_rect,
                ChannelToggle::new(mono, ChannelToggleTone::Monitor, "MONO"),
                "Sum the output to mono",
            ));
        }
        if let Some(dim) = self.dim.as_deref_mut() {
            let toggle_rect = Rect::from_center_size(
                Pos2::new(right - toggle_size.x / 2.0, rect.center().y + 0.0),
                toggle_size,
            );
            toggles.push((
                toggle_rect,
                ChannelToggle::new(dim, ChannelToggleTone::Monitor, "DIM"),
                "Attenuate the monitor output",
            ));
        }
        if let Some(muted) = self.muted.as_deref_mut() {
            let toggle_rect = Rect::from_center_size(
                Pos2::new(right - toggle_size.x / 2.0, rect.center().y - 27.0),
                toggle_size,
            );
            toggles.push((
                toggle_rect,
                ChannelToggle::new(muted, ChannelToggleTone::Mute, "MUTE"),
                "Mute the master output",
            ));
        }
        right -= toggle_size.x + 16.0;

        // Output fader.
        let fader_width = 236.0;
        let fader_rect = Rect::from_center_size(
            Pos2::new(right - fader_width / 2.0, rect.center().y),
            Vec2::new(fader_width, 36.0),
        );
        right = fader_rect.left() - 18.0;

        // Stereo meter with its own scale, hold and per-channel clip lamps.
        let meter_width = 86.0;
        let meter_rect = Rect::from_min_max(
            Pos2::new(right - meter_width, rect.top() + 14.0),
            Pos2::new(right, rect.bottom() - 14.0),
        );

        // Heading, caption and the large gain number.
        let text_x = rect.left() + 22.0;
        painter.text(
            Pos2::new(text_x, rect.center().y - 32.0),
            Align2::LEFT_CENTER,
            self.title,
            FontId::proportional(15.0),
            if muted { palette.text_dim } else { palette.foreground },
        );
        painter.text(
            Pos2::new(text_x, rect.center().y - 14.0),
            Align2::LEFT_CENTER,
            self.subtitle,
            FontId::proportional(9.5),
            palette.text_dim,
        );
        DbReadout::new(*self.gain_db)
            .decimals(1)
            .size(22.0)
            .paint(
                ui,
                &theme,
                Rect::from_min_size(
                    Pos2::new(text_x, rect.center().y + 4.0),
                    Vec2::new(104.0, 28.0),
                ),
            );

        let meter_levels: &[ChannelLevel] = if muted { &[] } else { self.levels };
        LevelMeter::new(meter_levels)
            .style(MeterStyle::Segmented)
            .range(self.min_db, self.max_db)
            .orientation(Orientation::Vertical)
            .thickness(9.0)
            .channel_gap(6.0)
            .scale(true)
            .readout(true)
            .hold(true)
            .clip(true)
            .id(ui.id().with(("master-meter", self.title)))
            .paint(ui, &theme, meter_rect);

        let fader = Fader::new(self.gain_db)
            .range(self.min_db, self.max_db)
            .orientation(Orientation::Horizontal)
            .scale(false)
            .value_readout(true)
            .track_thickness(5.0)
            .thumb_size(16.0);
        if ui.put(fader_rect, fader).changed() {
            response.mark_changed();
        }

        for (toggle_rect, toggle, tooltip) in toggles {
            if ui.put(toggle_rect, toggle).on_hover_text(tooltip).changed() {
                response.mark_changed();
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_master_section_renders_headlessly() {
        let mut gain = -2.0;
        let mut muted = false;
        let mut dim = false;
        let mut mono = true;
        let levels = [ChannelLevel::new(-8.0), ChannelLevel::new(-7.0)];
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                MasterSection::new(&mut gain)
                    .mute(&mut muted)
                    .dim(&mut dim)
                    .mono(&mut mono)
                    .levels(&levels),
            );
        });
        assert!(!output.shapes.is_empty(), "master section should emit shapes");
        assert_eq!(gain, -2.0);
    }

    #[test]
    fn a_master_section_keeps_a_working_width() {
        let mut gain = 0.0;
        let section = MasterSection::new(&mut gain).width(200.0).height(40.0);
        assert_eq!(section.width, Some(480.0));
        assert_eq!(section.height, Some(116.0));
    }

    #[test]
    fn the_master_defaults_name_the_main_output() {
        let mut gain = 0.0;
        let section = MasterSection::new(&mut gain);
        assert_eq!(section.title, "Master");
        assert_eq!(section.subtitle, "Main output");
    }
}
