//! A level meter drawn on an LCD screen's glass: block segments in the
//! display's ink, a held peak, readouts, a scale and a CLIP flag.
//!
//! This is the port of audiocn's `lcd-meter.tsx`. Metering is [`LevelMeter`]'s;
//! the size and every colour come from the screen underneath, which is why the
//! row heights, the gaps and the segment length are all whole multiples of the
//! display's dot pitch.

use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

use crate::db::{FormatOptions, SILENCE_DB, Sign, format_db};
use crate::taper::Taper;
use crate::ui::controls::lcd_display::{
    LcdAppearance, LcdDisplay, LcdScreen, LcdSize, LcdVariant, install, installed,
};
use crate::ui::meters::db_readout::DbReadout;
use crate::ui::meters::level_meter::ChannelLevel;
use crate::ui::theme::Theme;
use crate::zones::{DEFAULT_ZONES, MeterZone, Zone, zone_start_db};

/// The dot grid, shared with the screen this meter sits on.
const PITCH: f32 = 2.0;

/// One segment, in dots: `[--lcd-meter-segment:calc(pitch*3)]`.
const SEGMENT: f32 = PITCH * 3.0;

/// The dark between segments: `[--lcd-meter-gap:var(--lcd-display-pitch)]`.
const GAP: f32 = PITCH;

/// A segment and the gap after it: `--lcd-meter-period`.
const PERIOD: f32 = SEGMENT + GAP;

/// The marks a recorder screen prints, kept to the meter's range.
pub const DEFAULT_TICKS: [f32; 8] = [6.0, 3.0, 0.0, -3.0, -6.0, -12.0, -24.0, -48.0];

/// Between the columns of a row: `gap-x-[calc(pitch*4)]`.
const GAP_X: f32 = PITCH * 4.0;

/// Between the rows: `gap-y-[calc(pitch*2)]`.
const GAP_Y: f32 = PITCH * 2.0;

/// The display's own padding above a meter, `pt-[calc(0.75rem + nudge-y)]`.
const PAD_TOP: f32 = 12.0;

/// …and below it, `pb-3`.
const PAD_BOTTOM: f32 = 12.0;

/// The CLIP flag is printed at three quarters of the screen's text size.
const FLAG_SCALE: f32 = 0.75;

/// Magnitudes as a recorder prints them: no unit, no minus, a plus above zero.
#[must_use]
pub fn print_magnitude(db: f32) -> String {
    format_db(
        db,
        FormatOptions {
            decimals: 0,
            unit: false,
            sign: if db > 0.0 { Sign::Auto } else { Sign::Never },
            floor_db: SILENCE_DB,
        },
    )
}

/// A level meter on an LCD screen.
///
/// It draws horizontally, as every recorder screen does, and takes its row
/// height and colours from the display it sits on. Alone, it builds the screen
/// itself from the theme's primary ink.
///
/// ```
/// use noda_audio::ui::{ChannelLevel, LcdMeter};
///
/// let levels = [ChannelLevel::new(-12.0), ChannelLevel::new(-6.0)];
/// let meter = LcdMeter::new(&levels).holds(&[-8.0, -4.0]);
/// assert_eq!(meter.channel_count(), 2);
/// ```
#[derive(Clone, Copy, Debug)]
#[must_use = "an LcdMeter draws nothing until it is added to a Ui"]
pub struct LcdMeter<'a> {
    levels: &'a [ChannelLevel],
    holds: &'a [f32],
    variant: LcdVariant,
    appearance: LcdAppearance,
    size: LcdSize,
    min_db: f32,
    max_db: f32,
    taper: Taper,
    zones: &'a [MeterZone],
    ticks: &'a [f32],
    matrix: bool,
    show_scale: bool,
    show_value: bool,
    show_hold: bool,
    show_rms: bool,
    show_clip: bool,
    clipping: bool,
    track_width: Option<f32>,
}

impl<'a> LcdMeter<'a> {
    /// A meter over a set of channels.
    #[must_use]
    pub fn new(levels: &'a [ChannelLevel]) -> Self {
        Self {
            levels,
            holds: &[],
            variant: LcdVariant::Primary,
            appearance: LcdAppearance::Auto,
            size: LcdSize::Default,
            min_db: crate::db::DEFAULT_MIN_DB,
            max_db: crate::db::DEFAULT_MAX_DB,
            taper: Taper::AUDIO,
            zones: &DEFAULT_ZONES,
            ticks: &DEFAULT_TICKS,
            matrix: true,
            show_scale: true,
            show_value: true,
            show_hold: true,
            show_rms: false,
            show_clip: true,
            clipping: false,
            track_width: None,
        }
    }

    /// A stereo pair.
    #[must_use]
    pub fn stereo(levels: &'a [ChannelLevel; 2]) -> Self {
        Self::new(levels)
    }

    /// The held peak of each channel, in dB. A channel with no entry shows
    /// no marker; a caller that smooths its own levels keeps the holds here.
    #[must_use]
    pub fn holds(mut self, holds: &'a [f32]) -> Self {
        self.holds = holds;
        self
    }

    /// The ink, when the meter is drawn outside a display.
    #[must_use]
    pub fn variant(mut self, variant: LcdVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Pale glass or dark, when drawn outside a display.
    #[must_use]
    pub fn appearance(mut self, appearance: LcdAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// The screen's text size, which sets the row height.
    #[must_use]
    pub fn size(mut self, size: LcdSize) -> Self {
        self.size = size;
        self
    }

    /// The meter's range. Default `-60..0`.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// How dB maps to a position along the track. Default the audio taper,
    /// which gives the loud end most of the track, like a recorder.
    #[must_use]
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
        self
    }

    /// The zone table, which is where the clip bracket starts.
    #[must_use]
    pub fn zones(mut self, zones: &'a [MeterZone]) -> Self {
        self.zones = zones;
        self
    }

    /// The scale's marks. Default the recorder's, kept to the meter's range.
    #[must_use]
    pub fn ticks(mut self, ticks: &'a [f32]) -> Self {
        self.ticks = ticks;
        self
    }

    /// Break the ink into dots. Default true, and it follows the display's.
    #[must_use]
    pub fn matrix(mut self, matrix: bool) -> Self {
        self.matrix = matrix;
        self
    }

    /// Print the scale and the clip bracket under the tracks. Default true.
    #[must_use]
    pub fn scale(mut self, show: bool) -> Self {
        self.show_scale = show;
        self
    }

    /// Print a level beside each track. Default true.
    #[must_use]
    pub fn value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Light one segment at each held peak. Default true.
    #[must_use]
    pub fn hold(mut self, show: bool) -> Self {
        self.show_hold = show;
        self
    }

    /// Layer the RMS bar under the peak bar, as a dual-meter recorder does.
    #[must_use]
    pub fn rms(mut self, show: bool) -> Self {
        self.show_rms = show;
        self
    }

    /// Reserve the CLIP flag's cell. Default true.
    #[must_use]
    pub fn clip(mut self, show: bool) -> Self {
        self.show_clip = show;
        self
    }

    /// Whether the clip flag is lit. A caller drives the hold; this only says
    /// what the flag shows this frame.
    #[must_use]
    pub fn clipping(mut self, clipping: bool) -> Self {
        self.clipping = clipping;
        self
    }

    /// A fixed track length; otherwise the meter takes what the row gives it.
    #[must_use]
    pub fn track_width(mut self, width: f32) -> Self {
        self.track_width = Some(width);
        self
    }

    /// Put this meter on a display's glass, under its rows.
    #[must_use]
    pub fn on_display(self, display: &LcdDisplay<'a>) -> LcdMeterOnDisplay<'a> {
        LcdMeterOnDisplay { meter: self, display: *display }
    }

    /// How many channels this meter draws.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.levels.len()
    }

    /// The screen this meter draws with: the display under it, or one built
    /// from the theme when it is drawn alone.
    #[must_use]
    pub fn screen(&self, ui: &Ui, theme: &Theme) -> LcdScreen {
        match installed(ui) {
            Some(context) => context.screen,
            None => LcdScreen::new(self.variant.ink(theme), self.appearance.resolve(theme)),
        }
    }

    /// The size of the display under this meter, or the one asked for.
    #[must_use]
    pub fn screen_size(&self, ui: &Ui) -> LcdSize {
        installed(ui).map_or(self.size, |context| context.size)
    }

    /// The text size the screen is set in, which the row height follows.
    #[must_use]
    pub fn text_size(&self, ui: &Ui) -> f32 {
        self.screen_size(ui).font_size()
    }

    /// The thickness of one bar: five, seven or ten dots of the screen's grid.
    #[must_use]
    pub fn thickness(&self, ui: &Ui) -> f32 {
        self.screen_size(ui).meter_thickness()
    }

    /// Whether the screen under this meter breaks its ink into dots.
    #[must_use]
    pub fn matrix_on(&self, ui: &Ui) -> bool {
        installed(ui).map_or(self.matrix, |context| context.matrix)
    }

    /// `L` and `R` for a pair, numbers for more, nothing for one - the way a
    /// recorder labels its rows.
    #[must_use]
    pub fn channel_label(index: usize, count: usize) -> Option<String> {
        match count {
            0 | 1 => None,
            2 => Some(if index == 0 { "L".into() } else { "R".into() }),
            _ => Some((index + 1).to_string()),
        }
    }

    /// The position of a level along the track, `0` quiet to `1` full.
    #[must_use]
    pub fn position(&self, db: f32) -> f32 {
        if !db.is_finite() || db <= self.min_db {
            0.0
        } else {
            self.taper.to_position(db, self.min_db, self.max_db).clamp(0.0, 1.0)
        }
    }

    /// The width of one readout column: the widest number the readout can
    /// print, times the 1.2 the screen's tracking costs per cell.
    #[must_use]
    pub fn value_width(&self, ui: &Ui, screen: &LcdScreen) -> f32 {
        let widest = DbReadout::new(SILENCE_DB)
            .unit(false)
            .decimals(1)
            .floor_db(self.min_db)
            .widest_chars();
        screen.ch(ui, self.text_size(ui)) * widest as f32 * 1.2
    }

    /// The label column, `round(up, 1.5em, pitch)` wide, and zero for a meter
    /// with one channel to name.
    #[must_use]
    pub fn label_width(&self, ui: &Ui, _screen: &LcdScreen) -> f32 {
        if self.channel_count() < 2 {
            return 0.0;
        }
        round_up(self.text_size(ui) * 1.5, PITCH)
    }

    /// The width the CLIP flag asks for: `CLIP` at three quarters the text
    /// size, plus `px-1` either side.
    #[must_use]
    pub fn flag_width(&self, ui: &Ui, screen: &LcdScreen) -> f32 {
        if !self.show_clip {
            return 0.0;
        }
        screen.measure(ui, "CLIP", self.text_size(ui) * FLAG_SCALE).x + 8.0
    }

    /// The height of one channel row: the track, or a line of readout text.
    #[must_use]
    pub fn row_height(&self, ui: &Ui, screen: &LcdScreen) -> f32 {
        self.thickness(ui).max(screen.line_height(ui, self.text_size(ui)))
    }

    /// The height of the scale row: a bracket, a dot of gap, a line of marks.
    #[must_use]
    pub fn scale_height(&self, ui: &Ui, _screen: &LcdScreen) -> f32 {
        if !self.show_scale {
            return 0.0;
        }
        bracket_height() + PITCH + round_up(self.text_size(ui) * FLAG_SCALE, PITCH)
    }

    /// The size the meter wants.
    #[must_use]
    pub fn desired(&self, ui: &Ui, theme: &Theme) -> Vec2 {
        let screen = self.screen(ui, theme);
        let rows = self.channel_count().max(1) as f32 * self.row_height(ui, &screen)
            + self.channel_count().saturating_sub(1) as f32 * GAP_Y;
        let tail = GAP_Y + self.scale_height(ui, &screen);
        Vec2::new(
            self.track_width.unwrap_or(PERIOD * 24.0) + self.gutters(ui, &screen),
            PAD_TOP + rows + tail + PAD_BOTTOM,
        )
    }

    /// Whatever the label, readout and flag columns cost beyond the track.
    fn gutters(&self, ui: &Ui, screen: &LcdScreen) -> f32 {
        let label = self.label_width(ui, screen);
        let left = if label > 0.0 { label + GAP_X } else { 0.0 };
        let right = if self.show_value || self.show_clip {
            self.value_width(ui, screen)
                .max(self.flag_width(ui, screen))
                + GAP_X
        } else {
            0.0
        };
        left + right
    }

    /// Draw the meter into a rect, which is normally the content area of the
    /// display underneath it.
    pub fn paint(&self, ui: &Ui, rect: Rect, theme: &Theme) {
        let screen = self.screen(ui, theme);
        let matrix = self.matrix_on(ui);
        let text = self.text_size(ui);
        let row = self.row_height(ui, &screen);
        let label_width = self.label_width(ui, &screen);
        let value_width = self
            .value_width(ui, &screen)
            .max(self.flag_width(ui, &screen));

        let body = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.min.y + PAD_TOP),
            Vec2::new(rect.width(), rect.height() - PAD_TOP - PAD_BOTTOM),
        );
        if !body.is_positive() {
            return;
        }
        let track_left =
            body.min.x + if label_width > 0.0 { label_width + GAP_X } else { 0.0 };
        let track_right = if self.show_value || self.show_clip {
            body.max.x - value_width - GAP_X
        } else {
            body.max.x
        };
        // `ON_GRID`: the track is cut down to whole segments, so the last one
        // is never sliced in half.
        let wanted = self
            .track_width
            .unwrap_or(track_right - track_left)
            .min(track_right - track_left);
        let track_left = (track_right - wanted).max(track_left);
        let track_width = round_down(wanted + GAP, PERIOD) - GAP;
        if track_width < PERIOD {
            return;
        }

        let thickness = self.thickness(ui);
        for (index, level) in self.levels.iter().enumerate() {
            let top = body.min.y + index as f32 * (row + GAP_Y);
            let track = Rect::from_min_size(
                Pos2::new(track_left, top + (row - thickness) * 0.5),
                Vec2::new(track_width, thickness),
            );
            self.paint_track(ui, track, &screen, level, index);
            if let Some(name) = Self::channel_label(index, self.channel_count()) {
                ui.painter().text(
                    Pos2::new(body.min.x + label_width, track.center().y),
                    Align2::RIGHT_CENTER,
                    name,
                    screen.font(text),
                    screen.ink_dim,
                );
            }
            if self.show_value {
                let shown = DbReadout::new(level.peak_db)
                    .unit(false)
                    .floor_db(self.min_db)
                    .text();
                screen.glow_text(
                    ui,
                    Pos2::new(track.right() + GAP_X + value_width, track.center().y),
                    Align2::RIGHT_CENTER,
                    &shown,
                    text,
                    matrix,
                );
            }
        }

        let rows = self.channel_count().max(1) as f32 * (row + GAP_Y) - GAP_Y;
        let top = body.min.y + rows + GAP_Y;
        let band = Rect::from_min_size(
            Pos2::new(track_left, top),
            Vec2::new(track_width, self.scale_height(ui, &screen)),
        );
        if self.show_scale {
            self.paint_bracket(ui, band, &screen);
            self.paint_ticks(
                ui,
                Rect::from_min_size(
                    Pos2::new(band.min.x, band.min.y + bracket_height() + PITCH),
                    Vec2::new(band.width(), band.height() - bracket_height() - PITCH),
                ),
                &screen,
            );
        }
        if self.show_clip {
            self.paint_flag(
                ui,
                Rect::from_min_size(
                    Pos2::new(body.max.x - self.flag_width(ui, &screen), band.min.y),
                    Vec2::new(self.flag_width(ui, &screen), band.height().max(row)),
                ),
                &screen,
            );
        }
    }

    /// One row: the unlit segments, the lit bar, the held peak.
    fn paint_track(&self, ui: &Ui, track: Rect, screen: &LcdScreen, level: &ChannelLevel, index: usize) {
        let count = segment_count(track.width());
        // The whole track is a column of unlit segments the bar and the held
        // peak light.
        for segment in 0..count {
            ui.painter()
                .rect_filled(segment_rect(track, segment), 0.0, screen.meter_unlit());
        }
        if self.show_rms {
            if let Some(rms) = level.rms_db {
                // The browser gives both bars the same ink; a dimmed
                // under-layer is what makes a dual meter readable.
                paint_bar(ui, track, self.position(rms), screen.ink_dim);
            }
        }
        paint_bar(ui, track, self.position(level.peak_db), screen.lit);
        if !self.show_hold {
            return;
        }
        let held = self.holds.get(index).copied().unwrap_or(SILENCE_DB);
        let hold = self.position(held);
        if hold <= 0.0 {
            return;
        }
        // One whole segment at the held peak, fading in over the first tenth
        // of a dot as `opacity: calc(var(--meter-hold) * 1000)` does.
        let at = (round_up(hold * track.width(), PERIOD) / PERIOD) as usize;
        let rect = segment_rect(track, at.saturating_sub(1).min(count - 1));
        ui.painter().rect_filled(rect, 0.0, fade(screen.lit, hold * 1000.0));
    }

    /// The bracket over the clip zone, opening at the first segment of it.
    fn paint_bracket(&self, ui: &Ui, band: Rect, screen: &LcdScreen) {
        let clip_from = zone_start_db(Zone::Clip, self.zones, f32::INFINITY);
        if !clip_from.is_finite() || clip_from >= self.max_db || band.height() < PITCH {
            return;
        }
        let start = round_down(self.position(clip_from) * band.width(), PERIOD);
        let rect = Rect::from_min_max(
            Pos2::new(band.min.x + start, band.min.y),
            Pos2::new(band.max.x, band.min.y + bracket_height()),
        );
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(PITCH, screen.ink_dim),
            egui::StrokeKind::Inside,
        );
    }

    /// The dim numbers under the tracks, each at its taper position.
    fn paint_ticks(&self, ui: &Ui, band: Rect, screen: &LcdScreen) {
        let text = self.text_size(ui) * FLAG_SCALE;
        for &db in self.ticks {
            if db < self.min_db || db > self.max_db || !band.is_positive() {
                continue;
            }
            let at = Pos2::new(band.min.x + self.position(db) * band.width(), band.min.y);
            let _ = screen.text(ui, at, Align2::CENTER_TOP, &print_magnitude(db), text, screen.ink_dim);
        }
    }

    /// The CLIP flag: an unlit ghost that inverts while the clip hold is on.
    fn paint_flag(&self, ui: &Ui, band: Rect, screen: &LcdScreen) {
        let text = self.text_size(ui) * FLAG_SCALE;
        let height = screen.line_height(ui, text) + 4.0;
        let rect = Rect::from_min_size(
            Pos2::new(band.max.x - self.flag_width(ui, screen), band.max.y - height),
            Vec2::new(self.flag_width(ui, screen), height),
        );
        ui.painter().rect_filled(
            rect,
            2.0,
            if self.clipping { screen.lit } else { Color32::TRANSPARENT },
        );
        let _ = screen.text(
            ui,
            Pos2::new(rect.right() - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
            "CLIP",
            text,
            if self.clipping { screen.glass } else { screen.meter_unlit() },
        );
    }
}

/// How many whole segments fit in a track of this width.
#[must_use]
fn segment_count(width: f32) -> usize {
    ((width + GAP) / PERIOD).floor().max(0.0) as usize
}

/// One segment's rect inside a track.
#[must_use]
fn segment_rect(track: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(track.min.x + index as f32 * PERIOD, track.min.y),
        Vec2::new(SEGMENT, track.height()),
    )
}

/// Reveal whole segments from the quiet end: a clip-path of
/// `round(up, level * 100%, period)` lights a segment as soon as the level
/// reaches into it.
fn paint_bar(ui: &Ui, track: Rect, level: f32, color: Color32) {
    if level <= 0.0 {
        return;
    }
    let count = segment_count(track.width());
    let lit = (round_up(level * track.width(), PERIOD) / PERIOD) as usize;
    for segment in 0..lit.min(count) {
        ui.painter().rect_filled(segment_rect(track, segment), 0.0, color);
    }
}

/// A colour whose alpha is `fraction`, clamped into the range CSS clamps it.
#[must_use]
fn fade(color: Color32, fraction: f32) -> Color32 {
    let alpha = fraction.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8)
}

/// The largest multiple of `period` that is no more than `distance`.
#[must_use]
fn round_down(distance: f32, period: f32) -> f32 {
    (distance / period).floor() * period
}

/// The smallest multiple of `period` that is no less than `distance`.
#[must_use]
fn round_up(distance: f32, period: f32) -> f32 {
    (distance / period).ceil() * period
}

/// The clip bracket is two dots tall.
#[must_use]
fn bracket_height() -> f32 {
    PITCH * 2.0
}

/// A meter sitting on a display's glass, under its rows.
///
/// This is what `<LcdDisplay><LcdMeter /></LcdDisplay>` says in the browser:
/// the display draws the bezel and publishes its screen, the meter inherits
/// the ink, the size and the dot grid from it, and takes the band the rows
/// leave behind.
///
/// ```ignore
/// ui.add(LcdMeter::stereo(&levels).on_display(&display));
/// ```
#[must_use = "an LcdMeterOnDisplay draws nothing until it is added to a Ui"]
pub struct LcdMeterOnDisplay<'a> {
    meter: LcdMeter<'a>,
    display: LcdDisplay<'a>,
}

impl<'a> LcdMeterOnDisplay<'a> {
    /// The glass this meter draws on.
    #[must_use]
    pub fn display(&self) -> &LcdDisplay<'a> {
        &self.display
    }
}

impl Widget for LcdMeterOnDisplay<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let screen = self.display.screen(&theme);
        let (size, matrix) = self.display.inherited();
        install(ui, &screen, size, matrix);
        let glass = self.meter.desired(ui, &theme);
        let frame = (self.display.desired(ui, &screen) + Vec2::new(0.0, glass.y)).max(glass);
        let (rect, response) = ui.allocate_exact_size(frame, Sense::hover());
        if ui.is_rect_visible(rect) {
            let content = self.display.paint_frame(ui, rect, &theme);
            let rows = content.height() - glass.y;
            self.display.show_rows(
                ui,
                Rect::from_min_size(content.min, Vec2::new(content.width(), rows.max(0.0))),
                &screen,
            );
            self.meter.paint(
                ui,
                Rect::from_min_size(
                    Pos2::new(content.min.x, content.min.y + rows.max(0.0)),
                    Vec2::new(content.width(), glass.y),
                ),
                &theme,
            );
        }
        response
    }
}

impl Widget for LcdMeter<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let size = self.desired(ui, &theme);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, &theme);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> [ChannelLevel; 2] {
        [ChannelLevel::new(-12.0).rms(-18.0), ChannelLevel::new(-6.0)]
    }

    #[test]
    fn the_segments_are_a_whole_number_of_dots() {
        assert_eq!(SEGMENT, 6.0);
        assert_eq!(GAP, 2.0);
        assert_eq!(PERIOD, 8.0);
        assert_eq!(segment_count(16.0 * PERIOD - GAP), 16);
        assert_eq!(segment_count(15.0), 2, "a ragged end is cut, not half-lit");
    }

    #[test]
    fn a_recorder_labels_a_pair_and_numbers_the_rest() {
        assert_eq!(LcdMeter::channel_label(0, 1), None);
        assert_eq!(LcdMeter::channel_label(0, 2).as_deref(), Some("L"));
        assert_eq!(LcdMeter::channel_label(1, 2).as_deref(), Some("R"));
        assert_eq!(LcdMeter::channel_label(3, 8).as_deref(), Some("4"));
    }

    #[test]
    fn the_audio_taper_gives_the_loud_end_most_of_the_track() {
        let meter = LcdMeter::new(&[]).range(-60.0, 0.0);
        assert_eq!(meter.position(-60.0), 0.0);
        assert_eq!(meter.position(0.0), 1.0);
        assert!((meter.position(-15.0) - 0.5).abs() < 0.001, "unity sits at the middle");
        assert!(meter.position(-6.0) > 0.65, "and -6 dB sits in the loud quarter");
        assert!(meter.position(-30.0) < 0.3, "while half the scale is quieter than -30");
        assert_eq!(meter.position(SILENCE_DB), 0.0, "silence lights nothing");
    }

    #[test]
    fn the_scale_keeps_only_the_ticks_inside_the_range() {
        assert_eq!(print_magnitude(0.0), "0");
        assert_eq!(print_magnitude(6.0), "+6");
        assert_eq!(print_magnitude(-24.0), "24", "a recorder prints magnitudes");
        let inside = DEFAULT_TICKS.iter().filter(|&&db| db >= -20.0 && db <= 0.0).count();
        assert_eq!(inside, 4, "6, 3, 0 and -3 of the eight marks");
    }

    #[test]
    fn a_meter_paints_both_on_a_display_and_alone() {
        let data = levels();
        let holds = [-9.0, -3.0];
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let theme = Theme::dark();
            let meter = LcdMeter::stereo(&data).holds(&holds).rms(true).clipping(true);
            let size = meter.desired(ui, &theme);
            assert!(size.x > 100.0 && size.y > 40.0, "{size}");
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            meter.paint(ui, rect, &theme);
            assert_eq!(meter.screen(ui, &theme).lit, theme.palette.primary);
        });
    }

    #[test]
    fn a_track_of_the_default_width_holds_whole_segments() {
        let data = levels();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let theme = Theme::dark();
            let meter = LcdMeter::stereo(&data);
            let screen = meter.screen(ui, &theme);
            let track = Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(24.0 * PERIOD - GAP, meter.size.meter_thickness()),
            );
            assert_eq!(segment_count(track.width()), 24);
            meter.paint_track(ui, track, &screen, &data[0], 0);
            assert!(track.height() > 0.0);
        });
    }

    #[test]
    fn the_rounding_ladder_matches_css() {
        assert_eq!(round_down(8.4, PERIOD), 8.0);
        assert_eq!(round_down(7.9, PERIOD), 0.0);
        assert_eq!(round_up(27.0, PITCH), 28.0, "1.5em of a 2px grid");
        assert_eq!(round_up(13.0, PERIOD), 16.0);
    }

    #[test]
    fn the_hold_marker_is_faint_low_and_opaque_high() {
        let ink = Color32::from_rgb(0, 212, 146);
        assert_eq!(fade(ink, 0.0001).a(), 0);
        assert_eq!(fade(ink, 0.5).a(), 127);
        assert_eq!(fade(ink, 1.5).a(), 255, "and never past solid");
    }
}
