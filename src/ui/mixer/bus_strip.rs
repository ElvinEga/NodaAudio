//! A group-bus strip: one summing lane with its meter, gain and toggles.
//!
//! A bus is a horizontal row rather than a full channel tower: it is the
//! compact thing a group of tracks feeds, so the name, member count and
//! destination read on one line beside a stereo meter and a gain slider.

use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use crate::taper::Taper;
use crate::ui::Orientation;
use crate::ui::controls::channel_toggle::ChannelToggle;
use crate::ui::controls::parameter_slider::ParameterSlider;
use crate::ui::meters::level_meter::{ChannelLevel, LevelMeter, MeterStyle};
use crate::ui::theme::Theme;

/// Default width of a bus row.
const DEFAULT_WIDTH: f32 = 520.0;

/// Default height of a bus row.
const DEFAULT_HEIGHT: f32 = 88.0;

/// A group or bus strip: the sum of several channels, monitored and trimmed.
///
/// The strip is presentational. It is handed the bus levels and a gain to
/// edit; the host owns the summing and the routing the subtitle describes.
///
/// ```ignore
/// ui.add(
///     BusStrip::new("GTR BUS", &mut bus_gain_db)
///         .members(4)
///         .route("Master")
///         .mute(&mut bus_muted)
///         .levels(&bus_levels),
/// );
/// ```
#[must_use = "a BusStrip draws nothing until it is added to a Ui"]
pub struct BusStrip<'a> {
    name: &'a str,
    members: usize,
    route: Option<&'a str>,
    accent: Option<Color32>,
    gain_db: &'a mut f32,
    muted: Option<&'a mut bool>,
    solo: Option<&'a mut bool>,
    levels: &'a [ChannelLevel],
    min_db: f32,
    max_db: f32,
    width: Option<f32>,
    height: Option<f32>,
}

impl<'a> BusStrip<'a> {
    /// Create a bus strip with a name and its gain in dB.
    pub fn new(name: &'a str, gain_db: &'a mut f32) -> Self {
        Self {
            name,
            members: 0,
            route: None,
            accent: None,
            gain_db,
            muted: None,
            solo: None,
            levels: &[],
            min_db: -60.0,
            max_db: 6.0,
            width: None,
            height: None,
        }
    }

    /// How many channels feed the bus; shown in the subtitle.
    #[must_use]
    pub fn members(mut self, members: usize) -> Self {
        self.members = members;
        self
    }

    /// Where the bus is routed, e.g. `"Master"`.
    #[must_use]
    pub fn route(mut self, route: &'a str) -> Self {
        self.route = Some(route);
        self
    }

    /// Color tag for the bus. Default the first chart color.
    #[must_use]
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Bind the mute state.
    #[must_use]
    pub fn mute(mut self, muted: &'a mut bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Bind the solo state.
    #[must_use]
    pub fn solo(mut self, solo: &'a mut bool) -> Self {
        self.solo = Some(solo);
        self
    }

    /// Post-gain stereo levels for the meter.
    #[must_use]
    pub fn levels(mut self, levels: &'a [ChannelLevel]) -> Self {
        self.levels = levels;
        self
    }

    /// The dB range the gain slider covers. Default -60..6 dB.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Explicit width. Default 520 points.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(340.0));
        self
    }

    /// Explicit height. Default 88 points.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(68.0));
        self
    }
}

impl Widget for BusStrip<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let accent = self.accent.unwrap_or(palette.chart[0]);
        let muted = self.muted.as_ref().is_some_and(|muted| **muted);
        let soloed = self.solo.as_ref().is_some_and(|solo| **solo);

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
            Stroke::new(
                if soloed { 1.3 } else { 1.0_f32 },
                if soloed { accent } else { palette.border },
            ),
            StrokeKind::Inside,
        );

        let accent = if muted {
            accent.linear_multiply(0.35)
        } else if soloed {
            accent
        } else {
            accent.linear_multiply(0.8)
        };
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 8.0, rect.top() + 12.0),
                Vec2::new(3.0, rect.height() - 24.0),
            ),
            1.5,
            accent,
        );

        let pad = 12.0;
        let mut right = rect.right() - pad;

        // Right edge: mute and solo.
        let toggle_size = Vec2::new(26.0, 22.0);
        let mut toggles = Vec::new();
        if let Some(solo) = self.solo.as_deref_mut() {
            let toggle = Rect::from_center_size(
                Pos2::new(right - toggle_size.x / 2.0, rect.center().y),
                toggle_size,
            );
            right -= toggle_size.x + 4.0;
            toggles.push((toggle, ChannelToggle::solo(solo), "Solo this bus"));
        }
        if let Some(muted) = self.muted.as_deref_mut() {
            let toggle = Rect::from_center_size(
                Pos2::new(right - toggle_size.x / 2.0, rect.center().y),
                toggle_size,
            );
            right -= toggle_size.x + 4.0;
            toggles.push((toggle, ChannelToggle::mute(muted), "Mute this bus"));
        }

        // Gain slider.
        let slider_width = 160.0;
        let slider_rect = Rect::from_center_size(
            Pos2::new(right - slider_width / 2.0, rect.center().y),
            Vec2::new(slider_width, 36.0),
        );
        right = slider_rect.left();

        // Stereo meter beside the name.
        let meter_width = 26.0;
        let meter_rect = Rect::from_min_max(
            Pos2::new(right - 14.0 - meter_width, rect.top() + 12.0),
            Pos2::new(right - 14.0, rect.bottom() - 12.0),
        );

        // Name block, clipped so a long name never runs under the meter.
        let name_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 20.0, rect.top()),
            Pos2::new(meter_rect.left() - 12.0, rect.bottom()),
        );
        let text_painter = ui.painter_at(name_rect);
        text_painter.text(
            Pos2::new(name_rect.left(), rect.center().y - 9.0),
            Align2::LEFT_CENTER,
            self.name,
            FontId::proportional(12.5),
            if muted { palette.text_dim } else { palette.foreground },
        );
        let mut subtitle = Vec::new();
        if self.members > 0 {
            subtitle.push(format!("{} ch", self.members));
        }
        if let Some(route) = self.route {
            subtitle.push(format!("to {route}"));
        }
        text_painter.text(
            Pos2::new(name_rect.left(), rect.center().y + 9.0),
            Align2::LEFT_CENTER,
            subtitle.join(" · "),
            FontId::proportional(9.5),
            palette.text_dim,
        );

        // The meter is painted straight into its rect so the row keeps the
        // exact height it was given; a muted bus reads as silence.
        let meter_levels: &[ChannelLevel] = if muted { &[] } else { self.levels };
        LevelMeter::new(meter_levels)
            .style(MeterStyle::Segmented)
            .range(self.min_db, self.max_db)
            .orientation(Orientation::Vertical)
            .thickness(6.0)
            .channel_gap(4.0)
            .scale(false)
            .readout(false)
            .hold(true)
            .clip(false)
            .id(ui.id().with(("bus-meter", self.name)))
            .paint(ui, &theme, meter_rect);

        let slider = ParameterSlider::new(self.gain_db, self.min_db, self.max_db)
            .taper(Taper::AUDIO)
            .label("Gain")
            .unit("dB")
            .decimals(1)
            .width(slider_rect.width());
        if ui.put(slider_rect, slider).changed() {
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
    use crate::ui::meters::level_meter::ChannelLevel;

    #[test]
    fn a_bus_strip_renders_headlessly() {
        let mut gain = -3.0;
        let mut muted = false;
        let mut solo = false;
        let levels = [
            ChannelLevel::new(-12.0),
            ChannelLevel::new(-9.0),
        ];
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                BusStrip::new("GTR BUS", &mut gain)
                    .members(4)
                    .route("Master")
                    .mute(&mut muted)
                    .solo(&mut solo)
                    .levels(&levels),
            );
        });
        assert!(!output.shapes.is_empty(), "bus strip should emit shapes");
        assert_eq!(gain, -3.0);
    }

    #[test]
    fn a_bus_strip_clamps_its_minimum_size() {
        let mut gain = 0.0;
        let strip = BusStrip::new("DRUMS", &mut gain).width(10.0).height(10.0);
        assert_eq!(strip.width, Some(340.0));
        assert_eq!(strip.height, Some(68.0));
    }

    #[test]
    fn a_muted_bus_still_lays_out_its_controls() {
        let mut gain = 1.0;
        let mut muted = true;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(BusStrip::new("BUS", &mut gain).mute(&mut muted));
        });
        assert!(!output.shapes.is_empty());
    }
}
