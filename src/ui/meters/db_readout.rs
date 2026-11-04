//! The dB readout: a number that keeps its width while it counts.

use egui::{Color32, FontFamily, FontId, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

use crate::db::{FormatOptions, SILENCE_DB, Sign, format_db};
use crate::ui::meters::level_meter::ChannelLevel;
use crate::ui::theme::Theme;
use crate::zones::{DEFAULT_ZONES, MeterZone, zone_for_db};

/// The two digits either side of zero that a meter is expected to reach,
/// which is what audiocn measures its readout width against.
const WIDEST_MAGNITUDE_DB: f32 = 88.8;

/// Which of a level's two measurements to show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Measure {
    /// The block peak.
    #[default]
    Peak,
    /// The block mean-square, falling back to the peak when the caller did
    /// not measure one.
    Rms,
}

/// Which channel of a set of levels to show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChannelPick {
    /// The loudest of them, as a strip's single readout does.
    #[default]
    Max,
    /// One by index; out of range reads silence.
    Index(usize),
}

/// A monospace dB number, right-aligned, that reserves enough width for the
/// widest value it can print so the layout never twitches as it counts.
///
/// It draws the value it is handed: either a plain number, or a set of
/// [`ChannelLevel`]s plus a [`Measure`] and a [`ChannelPick`] to choose one.
///
/// ```
/// use noda_audio::ui::DbReadout;
///
/// let readout = DbReadout::new(-12.34).decimals(1);
/// assert_eq!(readout.text(), "\u{2212}12.3 dB");
/// ```
#[derive(Clone, Copy, Debug)]
#[must_use = "a DbReadout draws nothing until it is added to a Ui"]
pub struct DbReadout<'a> {
    /// The number to show, when no levels were given.
    value: f32,
    /// Levels to pick one from instead.
    levels: Option<&'a [ChannelLevel]>,
    measure: Measure,
    channel: ChannelPick,
    decimals: usize,
    unit: bool,
    sign: Sign,
    floor_db: f32,
    zones: &'a [MeterZone],
    tinted: bool,
    font_size: Option<f32>,
    badge: bool,
}

impl<'a> DbReadout<'a> {
    /// A readout of one dBFS value.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            levels: None,
            measure: Measure::Peak,
            channel: ChannelPick::Max,
            decimals: 1,
            unit: true,
            sign: Sign::Auto,
            floor_db: crate::db::DEFAULT_MIN_DB,
            zones: &DEFAULT_ZONES,
            tinted: false,
            font_size: None,
            badge: false,
        }
    }

    /// Whether to draw a subtle badge container behind the text.
    pub fn badge(mut self, badge: bool) -> Self {
        self.badge = badge;
        self
    }

    /// A readout that picks a level out of a channel set, the way a strip
    /// shows one number for its stereo pair.
    #[must_use]
    pub fn levels(levels: &'a [ChannelLevel]) -> Self {
        Self { levels: Some(levels), ..Self::new(SILENCE_DB) }
    }

    /// Peak or RMS. Default peak.
    #[must_use]
    pub fn measure(mut self, measure: Measure) -> Self {
        self.measure = measure;
        self
    }

    /// The loudest channel, or one by index. Default the loudest.
    #[must_use]
    pub fn channel(mut self, channel: ChannelPick) -> Self {
        self.channel = channel;
        self
    }

    /// Digits after the point. Default one.
    #[must_use]
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Whether to append the unit. Default true.
    #[must_use]
    pub fn unit(mut self, unit: bool) -> Self {
        self.unit = unit;
        self
    }

    /// How the sign is written. Default [`Sign::Auto`].
    #[must_use]
    pub fn sign(mut self, sign: Sign) -> Self {
        self.sign = sign;
        self
    }

    /// At or below this the number reads as silence. Default `-60`, and a
    /// strip should pass its own meter floor so the bar and the number agree.
    #[must_use]
    pub fn floor_db(mut self, floor_db: f32) -> Self {
        self.floor_db = floor_db;
        self
    }

    /// The zone table the tint is read from.
    #[must_use]
    pub fn zones(mut self, zones: &'a [MeterZone]) -> Self {
        self.zones = zones;
        self
    }

    /// Colour the number by its metering zone, in the `*-foreground` tones
    /// audiocn hands to `data-zone` selectors. Off by default: the readout
    /// inherits the surrounding text, exactly as the span does.
    #[must_use]
    pub fn tinted(mut self, tinted: bool) -> Self {
        self.tinted = tinted;
        self
    }

    /// Point size, defaulting a step above the label ladder since a readout
    /// is the thing a user reads.
    #[must_use]
    pub fn size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// The value the readout resolves to after picking a measure and a
    /// channel; silence when there is nothing to pick.
    #[must_use]
    pub fn value(&self) -> f32 {
        let Some(levels) = self.levels else {
            return self.value;
        };
        let read = |index: usize| match levels.get(index) {
            Some(level) => match self.measure {
                Measure::Peak => level.peak_db,
                Measure::Rms => level.rms_db.unwrap_or(level.peak_db),
            },
            None => SILENCE_DB,
        };
        match self.channel {
            ChannelPick::Index(index) => read(index),
            ChannelPick::Max => (0..levels.len()).map(read).fold(SILENCE_DB, f32::max),
        }
    }

    /// The options the number is printed with.
    #[must_use]
    fn options(&self) -> FormatOptions {
        FormatOptions {
            decimals: self.decimals,
            unit: self.unit,
            sign: self.sign,
            floor_db: self.floor_db,
        }
    }

    /// The text as it will be drawn.
    #[must_use]
    pub fn text(&self) -> String {
        format_db(self.value(), self.options())
    }

    /// The glyph count of the widest string this readout can ever print,
    /// probed the way audiocn probes it: silence, one quantum above the
    /// floor, and the two-digit magnitudes either side of zero.
    #[must_use]
    pub fn widest_chars(&self) -> usize {
        let just_above_floor =
            self.floor_db + 10f32.powi(-(self.decimals as i32));
        [SILENCE_DB, just_above_floor, -WIDEST_MAGNITUDE_DB, WIDEST_MAGNITUDE_DB]
            .into_iter()
            .map(|db| format_db(db, self.options()).chars().count())
            .max()
            .unwrap_or(0)
    }

    /// The number's colour: the zone's foreground when tinted, the dim
    /// colour at or below the floor, and the plain ink otherwise.
    #[must_use]
    pub fn color(&self, theme: &Theme) -> Color32 {
        let db = self.value();
        if !db.is_finite() || db <= self.floor_db {
            return theme.palette.text_dim;
        }
        if self.tinted {
            return theme.palette.zone_foreground(zone_for_db(db, self.zones));
        }
        theme.palette.foreground
    }

    /// The font this readout draws with.
    fn font(&self, theme: &Theme) -> FontId {
        FontId::new(self.font_size.unwrap_or_else(|| theme.size.value_font()), FontFamily::Monospace)
    }

    /// The width a monospace digit occupies, which is what a `ch` unit is.
    fn ch_width(&self, ui: &Ui, theme: &Theme) -> f32 {
        ui.painter()
            .layout_no_wrap("0".to_owned(), self.font(theme), Color32::WHITE)
            .size()
            .x
    }

    /// Paint into a rect the caller already owns: right-aligned ink, no
    /// frame, no background, as the span has none.
    pub fn paint(&self, ui: &Ui, theme: &Theme, rect: Rect) {
        if self.badge {
            let bg = theme.palette.track;
            let border_stroke = egui::Stroke::new(1.0_f32, theme.palette.border);
            ui.painter().rect(rect, 3.0, bg, border_stroke, egui::StrokeKind::Inside);
        }
        let font = self.font(theme);
        let color = self.color(theme);
        let galley = ui.painter().layout_no_wrap(self.text(), font, color);
        let right = rect.right() - (if self.badge { 3.0 } else { 0.0 });
        let at = Pos2::new(right - galley.size().x, rect.center().y - galley.size().y * 0.5);
        ui.painter().galley(at, galley, color);
    }
}

impl Widget for DbReadout<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let ch = self.ch_width(ui, &theme);
        let galley_height = ui
            .painter()
            .layout_no_wrap("0".to_owned(), self.font(&theme), Color32::WHITE)
            .size()
            .y;
        let size = Vec2::new(ch * self.widest_chars() as f32, galley_height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui, &theme, rect);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::Zone;

    #[test]
    fn a_readout_prints_the_value_it_is_handed() {
        assert_eq!(DbReadout::new(-12.34).text(), "\u{2212}12.3 dB");
        assert_eq!(DbReadout::new(-6.0).decimals(1).text(), "\u{2212}6.0 dB");
        assert_eq!(DbReadout::new(-12.0).unit(false).text(), "\u{2212}12.0");
    }

    #[test]
    fn silence_and_the_floor_both_read_as_minus_infinity() {
        let at_floor = DbReadout::new(-60.0);
        assert_eq!(at_floor.text(), "\u{2212}\u{221e} dB");
        assert_eq!(DbReadout::new(SILENCE_DB).text(), "\u{2212}\u{221e} dB");
        assert!(DbReadout::new(f32::NAN).text().contains("--"), "an unmeasured level is not silence");
    }

    #[test]
    fn a_tiny_negative_does_not_print_a_signed_zero() {
        assert_eq!(DbReadout::new(-0.04).text(), "0.0 dB", "prints no -0.0");
    }

    #[test]
    fn the_width_is_the_widest_thing_it_can_ever_print() {
        let readout = DbReadout::new(-6.0);
        // -59.9 dB is eight characters: the probe just above the floor is what sets the width.
        assert_eq!(readout.widest_chars(), 8, "the widest string is just above the floor");
        assert_eq!(DbReadout::new(-6.0).unit(false).widest_chars(), 5);
        assert_eq!(DbReadout::new(-6.0).decimals(0).widest_chars(), 6);
    }

    #[test]
    fn levels_are_picked_by_measure_and_channel() {
        let levels = [
            ChannelLevel::new(-30.0).rms(-36.0),
            ChannelLevel::new(-12.0).rms(-20.0),
        ];
        let loudest = DbReadout::levels(&levels);
        assert_eq!(loudest.value(), -12.0);
        assert_eq!(loudest.measure(Measure::Rms).value(), -20.0);
        assert_eq!(loudest.channel(ChannelPick::Index(0)).value(), -30.0);
        assert_eq!(
            loudest.channel(ChannelPick::Index(7)).value(),
            SILENCE_DB,
            "no such channel is silence, not a panic"
        );
    }

    #[test]
    fn the_tint_follows_the_zone_and_silence_falls_back_to_dim() {
        let theme = Theme::dark();
        let tinted = DbReadout::new(-30.0).tinted(true);
        assert_eq!(tinted.color(&theme), theme.palette.meter_ok_foreground);
        assert_eq!(
            DbReadout::new(-2.0).tinted(true).color(&theme),
            theme.palette.meter_clip_foreground
        );
        assert_eq!(DbReadout::new(-80.0).tinted(true).color(&theme), theme.palette.text_dim);
        assert_eq!(
            DbReadout::new(-30.0).color(&theme),
            theme.palette.foreground,
            "untinted text is just ink"
        );
    }

    #[test]
    fn a_readout_holds_its_width_across_the_scale() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let widths: Vec<f32> = [0.0f32, -6.0, -59.9, SILENCE_DB, f32::NAN]
                .into_iter()
                .map(|db| ui.add(DbReadout::new(db).floor_db(-60.0)).rect.width())
                .collect();
            let widest = widths.iter().copied().fold(0.0f32, f32::max);
            for (db, width) in [-60.0f32, -6.0, 0.0].iter().zip(&widths) {
                assert!(
                    (*width - widest).abs() < 0.5,
                    "{db} dB drew {width} wide against a {widest} reserve"
                );
            }
        });
    }

    #[test]
    fn the_zone_table_is_a_zone_lookup_not_a_colour_choice() {
        let zones: [MeterZone; 1] = [MeterZone { from_db: -6.0, zone: Zone::Clip }];
        let readout = DbReadout::new(-30.0).zones(&zones).tinted(true);
        assert_eq!(readout.color(&Theme::dark()), Theme::dark().palette.meter_ok_foreground);
    }
}
