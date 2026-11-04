//! Level meters: the bar that tells you how loud something is.
//!
//! A [`LevelMeter`] draws the levels it is handed. It opens no device and
//! owns no signal, so the same widget serves a plugin receiving its
//! `process` peak, a standalone app measuring its own buffer, and a test
//! feeding it a sine sweep.
//!
//! Levels arrive in dBFS. Optionally the meter smooths them with
//! [`Ballistics`] at frame rate, which is what a hardware meter does to its
//! needle; feed it block peaks and the bar moves without strobing.

use egui::{
    Align2, Color32, FontId, Rect, Response, Rgba, Sense, Stroke, StrokeKind, Ui, Vec2, pos2,
};

use crate::ballistics::{Ballistics, MeterReading, MeterState};
use crate::db::{
    DEFAULT_MAX_DB, DEFAULT_MIN_DB, FormatOptions, SILENCE_DB, clamp, db_to_level, format_db,
    gain_to_db, level_to_db,
};
use crate::ui::Orientation;
use crate::ui::meters::clip_indicator::{ClipIndicator, ClipLatch, ClipPolicy};
use crate::ui::meters::db_scale::{DbScale, MarkEdge};
use crate::ui::theme::Theme;
use crate::ui::{Edge, peel};
use crate::zones::{DEFAULT_ZONES, MeterZone, Zone, zone_for_db};

/// One channel's level, as measured by the caller.
///
/// Both fields are dBFS; [`Self::silence`] and anything at or below the
/// meter's floor draw as an empty bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelLevel {
    /// Block peak.
    pub peak_db: f32,
    /// Block RMS, when the caller measures one.
    pub rms_db: Option<f32>,
}

impl ChannelLevel {
    /// A peak with no RMS.
    #[must_use]
    pub const fn new(peak_db: f32) -> Self {
        Self { peak_db, rms_db: None }
    }

    /// Attach an RMS reading, drawn as a line across the bar.
    #[must_use]
    pub const fn rms(mut self, rms_db: f32) -> Self {
        self.rms_db = Some(rms_db);
        self
    }

    /// From a normalized 0..1 peak, which is the scale truce publishes
    /// meters on.
    #[must_use]
    pub fn from_linear(peak: f32) -> Self {
        Self::new(gain_to_db(clamp(peak, 0.0, 1.0)))
    }

    /// Digital silence.
    #[must_use]
    pub const fn silence() -> Self {
        Self::new(SILENCE_DB)
    }
}

/// How a bar is built up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeterStyle {
    /// One fill, colored by the zone the level sits in.
    Solid,
    /// A ladder of segments, lit up to the level.
    Segmented,
    /// One fill whose color ramps continuously through the zones.
    Gradient,
}

/// A level meter for one or more channels.
///
/// ```ignore
/// LevelMeter::new(&levels)
///     .style(MeterStyle::Segmented)
///     .range(-48.0, 0.0)
///     .scale(true)
///     .readout(true)
///     .ui(ui);
/// ```
#[must_use = "a LevelMeter draws nothing until it is added to a Ui"]
pub struct LevelMeter<'a> {
    channels: &'a [ChannelLevel],
    orientation: Orientation,
    style: MeterStyle,
    min_db: f32,
    max_db: f32,
    zones: &'a [MeterZone],
    ballistics: Option<Ballistics>,
    thickness: Option<f32>,
    channel_gap: f32,
    segment_length: f32,
    segment_gap: f32,
    radius: f32,
    show_scale: bool,
    show_readout: bool,
    show_hold: bool,
    show_rms: bool,
    show_clip: bool,
    clip_policy: ClipPolicy,
    id: Option<egui::Id>,
}

impl<'a> LevelMeter<'a> {
    /// A meter over these channels, left to right or bottom to top.
    pub fn new(channels: &'a [ChannelLevel]) -> Self {
        Self {
            channels,
            orientation: Orientation::Vertical,
            style: MeterStyle::Solid,
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_MAX_DB,
            zones: &DEFAULT_ZONES,
            ballistics: Some(Ballistics::default()),
            thickness: None,
            channel_gap: 2.0,
            segment_length: 4.0,
            segment_gap: 1.5,
            radius: 1.0,
            show_scale: false,
            show_readout: false,
            show_hold: true,
            show_rms: true,
            show_clip: false,
            clip_policy: ClipPolicy::default(),
            id: None,
        }
    }

    /// A one-channel meter.
    pub fn mono(level: &'a ChannelLevel) -> Self {
        Self::new(std::slice::from_ref(level))
    }

    /// A two-channel meter, the console's L/R pair.
    pub fn stereo(levels: &'a [ChannelLevel; 2]) -> Self {
        Self::new(levels)
    }

    /// Which way the bars run.
    #[must_use]
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Fill style.
    #[must_use]
    pub fn style(mut self, style: MeterStyle) -> Self {
        self.style = style;
        self
    }

    /// The dBFS range the bar spans.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// The zone boundaries to color by.
    #[must_use]
    pub fn zones(mut self, zones: &'a [MeterZone]) -> Self {
        self.zones = zones;
        self
    }

    /// Smooth the incoming levels at frame rate. This is the default.
    #[must_use]
    pub fn ballistics(mut self, ballistics: Ballistics) -> Self {
        self.ballistics = Some(ballistics);
        self
    }

    /// Draw the levels exactly as handed in, with no smoothing, hold
    /// marker, or clip timing.
    #[must_use]
    pub fn raw(mut self) -> Self {
        self.ballistics = None;
        self
    }

    /// Bar thickness in points. Defaults to the theme's meter thickness.
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }

    /// Space between adjacent channels.
    #[must_use]
    pub fn channel_gap(mut self, gap: f32) -> Self {
        self.channel_gap = gap;
        self
    }

    /// Lit-segment length and the dark gap between them.
    #[must_use]
    pub fn segments(mut self, length: f32, gap: f32) -> Self {
        self.segment_length = length;
        self.segment_gap = gap;
        self
    }

    /// Corner radius of the track and fill.
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Draw a dB scale along the bars.
    #[must_use]
    pub fn scale(mut self, show: bool) -> Self {
        self.show_scale = show;
        self
    }

    /// Draw the loudest channel's level as text.
    #[must_use]
    pub fn readout(mut self, show: bool) -> Self {
        self.show_readout = show;
        self
    }

    /// Draw the peak-hold marker.
    #[must_use]
    pub fn hold(mut self, show: bool) -> Self {
        self.show_hold = show;
        self
    }

    /// Draw an RMS line per channel, where the caller supplied one.
    #[must_use]
    pub fn rms_marker(mut self, show: bool) -> Self {
        self.show_rms = show;
        self
    }

    /// Draw a clip lamp over each channel, fed by the meter's own
    /// [`ClipLatch`].
    #[must_use]
    pub fn clip(mut self, show: bool) -> Self {
        self.show_clip = show;
        self
    }

    /// Clip threshold and hold time for the lamps.
    #[must_use]
    pub fn clip_policy(mut self, policy: ClipPolicy) -> Self {
        self.clip_policy = policy;
        self
    }

    /// Pin the identity the smoothing state is stored under. Two meters in
    /// one [`Ui`] need distinct ids; one meter per `Ui` does not.
    #[must_use]
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Space the meter would like, in points.
    #[must_use]
    pub fn desired_size(&self, theme: &Theme) -> Vec2 {
        let lane = self.thickness.unwrap_or(theme.size.meter_thickness());
        let across = self.lane_extent(self.channels.len(), lane);
        let gap = theme.size.gap();

        let scale = if self.show_scale {
            self.scale_for(theme).desired_thickness(theme) + gap
        } else {
            0.0
        };
        let lamp = if self.show_clip { theme.size.clip_dot_size() + gap } else { 0.0 };
        let readout = if self.show_readout { theme.size.value_font() + gap } else { 0.0 };

        match self.orientation {
            Orientation::Vertical => Vec2::new(across + scale, 120.0 + lamp + readout),
            Orientation::Horizontal => Vec2::new(240.0 + lamp + readout, across + scale),
        }
    }

    /// Paint the meter into `rect`, sharing the layout between bars, scale,
    /// lamps and readout.
    pub fn paint(&self, ui: &Ui, theme: &Theme, rect: Rect) {
        let id = self.state_id(ui);
        let now_ms = now_ms(ui);
        let frames = self.advance(ui, id, now_ms);
        let gap = theme.size.gap();
        let lane_width =
            self.thickness.unwrap_or(theme.size.meter_thickness()).min(self.orientation.thickness(rect));

        let mut rest = rect;
        let (loud_end, quiet_end, scale_side) = match self.orientation {
            Orientation::Vertical => (Edge::Top, Edge::Bottom, Edge::Right),
            Orientation::Horizontal => (Edge::Right, Edge::Left, Edge::Bottom),
        };

        let lamps = if self.show_clip {
            let (strip, rest_rect) = peel(rest, loud_end, theme.size.clip_dot_size() + gap);
            rest = rest_rect;
            Some(strip)
        } else {
            None
        };
        let readout = if self.show_readout {
            let (strip, rest_rect) = peel(rest, quiet_end, theme.size.value_font() + gap);
            rest = rest_rect;
            Some(strip)
        } else {
            None
        };
        let scale_rect = if self.show_scale {
            let (strip, rest_rect) = peel(rest, scale_side, self.scale_for(theme).desired_thickness(theme) + gap);
            rest = rest_rect;
            Some(strip)
        } else {
            None
        };

        let bars = rest;
        if bars.area() <= 0.0 {
            return;
        }
        let painter = ui.painter_at(rect);

        for (index, frame) in frames.iter().enumerate() {
            let lane = self.lane_rect(bars, index, lane_width);
            if lane.area() <= 0.0 {
                continue;
            }
            self.paint_lane(&painter, theme, lane, frame);

            if let Some(strip) = lamps {
                let dot = theme.size.clip_dot_size();
                let center = self.across_center(bars, index, lane_width);
                let at = match self.orientation {
                    Orientation::Vertical => Rect::from_center_size(pos2(center, strip.center().y), Vec2::new(dot + 12.0, dot)),
                    Orientation::Horizontal => Rect::from_center_size(pos2(strip.center().x, center), Vec2::new(dot, dot + 12.0)),
                };
                ClipIndicator::new()
                    .active(frame.clip_active)
                    .dot_size(dot)
                    .paint_at(ui, theme, at);
            }
        }

        if let Some(strip) = scale_rect {
            self.scale_for(theme).paint(ui, strip);
        }
        if let Some(strip) = readout {
            self.paint_readout(&painter, theme, strip, &frames);
        }
    }

    /// The dB scale that would sit beside these bars.
    fn scale_for(&self, theme: &Theme) -> DbScale<'a> {
        let major = 5.0 + theme.size.label_font() * 0.3;
        let scale = match self.orientation {
            Orientation::Vertical => DbScale::vertical().mark_edge(MarkEdge::Start),
            Orientation::Horizontal => DbScale::horizontal().mark_edge(MarkEdge::End),
        };
        scale.range(self.min_db, self.max_db).labels(true).mark_lengths(major, major - 2.0)
    }

    /// Total across-axis space taken by `count` lanes.
    fn lane_extent(&self, count: usize, lane: f32) -> f32 {
        count as f32 * lane + gap_between(count, self.channel_gap)
    }

    /// The rect of one channel's bar inside `bars`.
    fn lane_rect(&self, bars: Rect, index: usize, lane: f32) -> Rect {
        let start = self.across_start(bars, index, lane);
        match self.orientation {
            Orientation::Vertical => Rect::from_min_max(
                pos2(start, bars.top()),
                pos2(start + lane, bars.bottom()),
            ),
            Orientation::Horizontal => Rect::from_min_max(
                pos2(bars.left(), start),
                pos2(bars.right(), start + lane),
            ),
        }
    }

    fn across_start(&self, bars: Rect, index: usize, lane: f32) -> f32 {
        let offset = index as f32 * (lane + self.channel_gap);
        match self.orientation {
            Orientation::Vertical => bars.left() + offset,
            Orientation::Horizontal => bars.top() + offset,
        }
    }

    fn across_center(&self, bars: Rect, index: usize, lane: f32) -> f32 {
        self.across_start(bars, index, lane) + lane / 2.0
    }

    /// One channel: track, fill, and the markers layered on top.
    fn paint_lane(&self, painter: &egui::Painter, theme: &Theme, lane: Rect, frame: &Frame) {
        let palette = &theme.palette;
        painter.rect_filled(lane, self.radius, palette.track);
        painter.rect_stroke(
            lane.shrink(0.5),
            self.radius,
            Stroke::new(1.0_f32, palette.border),
            StrokeKind::Inside,
        );

        let length = self.orientation.length(lane);
        if length <= 0.0 {
            return;
        }
        let level = db_to_level(frame.reading.db, self.min_db, self.max_db);

        match self.style {
            MeterStyle::Solid => self.paint_solid(painter, theme, lane, level),
            MeterStyle::Gradient => self.paint_gradient(painter, theme, lane, level),
            MeterStyle::Segmented => self.paint_segmented(painter, theme, lane, level, length),
        }

        if self.show_hold && self.ballistics.is_some() {
            let hold = db_to_level(frame.reading.hold_db, self.min_db, self.max_db);
            if hold > level {
                let color = palette.zone_color(zone_for_db(frame.reading.hold_db, self.zones));
                painter.rect_filled(self.marker(lane, hold, 2.0), 0.0, color);
            }
        }

        if self.show_rms
            && let Some(rms_db) = frame.rms_db
        {
            let rms = db_to_level(rms_db, self.min_db, self.max_db);
            let color = fade(palette.foreground, 0.75);
            painter.rect_filled(self.marker(lane, rms, 1.0), 0.0, color);
        }
    }

    /// Crisp zone slabs: each zone paints its own band up to the level.
    fn paint_solid(&self, painter: &egui::Painter, theme: &Theme, lane: Rect, level: f32) {
        for slab in self.slabs() {
            let top = slab.to.min(level);
            if top > slab.from {
                let color = theme.palette.zone_color(slab.zone);
                painter.rect_filled(self.band_rect(lane, slab.from, top), self.radius, color);
            }
        }
    }

    /// One fill whose color interpolates between the zone colors.
    fn paint_gradient(&self, painter: &egui::Painter, theme: &Theme, lane: Rect, level: f32) {
        let stops = self.stops(theme);
        let steps = (self.orientation.length(lane) / 2.0).ceil().clamp(8.0, 64.0) as usize;
        for step in 0..steps {
            let from = step as f32 / steps as f32;
            if from >= level {
                break;
            }
            let to = ((step + 1) as f32 / steps as f32).min(level);
            let color = ramp(&stops, (from + to) / 2.0);
            painter.rect_filled(self.band_rect(lane, from, to), 0.0, color);
        }
    }

    /// A ladder of segments, with the unlit ones ghosted so the range
    /// stays readable.
    fn paint_segmented(&self, painter: &egui::Painter, theme: &Theme, lane: Rect, level: f32, length: f32) {
        let pitch = (self.segment_length + self.segment_gap).max(1.0);
        let count = (length / pitch).floor() as usize;
        if count == 0 {
            return;
        }
        for index in 0..count {
            let from = index as f32 * pitch / length;
            let to = (from * length + self.segment_length) / length;
            let center = (from + to) / 2.0;
            let lit = center <= level;
            let db = level_to_db(center, self.min_db, self.max_db);
            let color = theme.palette.zone_color(zone_for_db(db, self.zones));
            let fill = if lit { color } else { fade(color, theme.meter_ghost_opacity) };
            painter.rect_filled(self.band_rect(lane, from, to), 0.0, fill);
        }
    }

    /// The rect spanning positions `from..to` along the bar.
    fn band_rect(&self, lane: Rect, from: f32, to: f32) -> Rect {
        let low = self.orientation.along(lane, from);
        let high = self.orientation.along(lane, to);
        match self.orientation {
            Orientation::Vertical => Rect::from_min_max(pos2(lane.left(), high.min(low)), pos2(lane.right(), high.max(low))),
            Orientation::Horizontal => Rect::from_min_max(pos2(high.min(low), lane.top()), pos2(high.max(low), lane.bottom())),
        }
    }

    /// A band of fixed pixel thickness centered on a position, for
    /// markers.
    fn marker(&self, lane: Rect, center: f32, thickness: f32) -> Rect {
        let half = thickness / 2.0 / self.orientation.length(lane).max(1.0);
        self.band_rect(lane, center - half, center + half)
    }

    /// Zone bands in position space, ascending.
    fn slabs(&self) -> Vec<Slab> {
        let sorted = self.sorted_zones();
        sorted
            .iter()
            .enumerate()
            .map(|(index, zone)| {
                let from = db_to_level(zone.from_db.max(self.min_db), self.min_db, self.max_db);
                let to = match sorted.get(index + 1) {
                    Some(next) => db_to_level(next.from_db.max(self.min_db), self.min_db, self.max_db),
                    None => 1.0,
                };
                Slab { from, to, zone: zone.zone }
            })
            .collect()
    }

    /// Color stops for the gradient fill: one per zone, at the middle of
    /// that zone's band. Below the first and above the last the color
    /// holds, so the quiet end stays green and the loud end stays red.
    fn stops(&self, theme: &Theme) -> Vec<(f32, Color32)> {
        self.slabs()
            .into_iter()
            .map(|slab| ((slab.from + slab.to) / 2.0, theme.palette.zone_color(slab.zone)))
            .collect()
    }

    fn sorted_zones(&self) -> Vec<MeterZone> {
        let mut sorted = self.zones.to_vec();
        sorted.sort_by(|a, b| a.from_db.total_cmp(&b.from_db));
        sorted
    }

    fn paint_readout(&self, painter: &egui::Painter, theme: &Theme, strip: Rect, frames: &[Frame]) {
        let mut db = SILENCE_DB;
        for frame in frames {
            if frame.reading.db > db {
                db = frame.reading.db;
            }
        }
        let text = format_db(
            db,
            FormatOptions { decimals: 1, unit: false, ..FormatOptions::default() },
        );
        let color = theme.palette.zone_color(zone_for_db(db, self.zones));
        painter.text(
            strip.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(theme.size.value_font()),
            color,
        );
    }

    /// Smoothed state, stored per meter id and advanced once per frame.
    fn advance(&self, ui: &Ui, id: egui::Id, now_ms: f32) -> Vec<Frame> {
        let raw = self.channels.to_vec();
        let policy = self.clip_policy;
        let ballistics = self.ballistics;
        ui.ctx().data_mut(|data| {
            let bank = data.get_temp_mut_or_default::<MeterBank>(id);
            bank.lanes.resize(raw.len(), Lane::default());
            bank.lanes
                .iter_mut()
                .zip(&raw)
                .map(|(lane, level)| {
                    let reading = match ballistics {
                        Some(options) => lane.peak.step(level.peak_db, options, now_ms),
                        None => MeterReading { db: level.peak_db, hold_db: level.peak_db },
                    };
                    let rms_db = match (level.rms_db, ballistics) {
                        (Some(db), Some(_)) => Some(lane.rms.step(db, Ballistics::VU, now_ms).db),
                        (Some(db), None) => Some(db),
                        _ => None,
                    };
                    let clip_active = if ballistics.is_some() {
                        lane.clip.report(level.peak_db, now_ms, policy)
                    } else {
                        false
                    };
                    Frame { reading, rms_db, clip_active }
                })
                .collect()
        })
    }

    fn state_id(&self, ui: &Ui) -> egui::Id {
        self.id.unwrap_or_else(|| ui.id().with("noda-level-meter"))
    }
}

impl<'a> egui::Widget for LevelMeter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let desired = self.desired_size(&theme);
        let (rect, response) = ui.allocate_at_least(desired, Sense::hover());
        if ui.is_rect_visible(rect) && !self.channels.is_empty() {
            self.paint(ui, &theme, rect);
            if self.ballistics.is_some() {
                ui.ctx().request_repaint();
            }
        }
        response
    }
}

/// A zone's band, in 0..1 positions along the bar.
#[derive(Clone, Copy, Debug)]
struct Slab {
    from: f32,
    to: f32,
    zone: Zone,
}

/// What one frame shows for one channel.
#[derive(Clone, Copy, Debug)]
struct Frame {
    reading: MeterReading,
    rms_db: Option<f32>,
    clip_active: bool,
}

/// One channel's integrators and lamp.
#[derive(Clone, Copy, Debug, Default)]
struct Lane {
    peak: MeterState,
    rms: MeterState,
    clip: ClipLatch,
}

/// The per-meter state kept in egui's context.
#[derive(Clone, Debug, Default)]
struct MeterBank {
    lanes: Vec<Lane>,
}

/// Space between `count` lanes.
#[must_use]
fn gap_between(count: usize, gap: f32) -> f32 {
    count.saturating_sub(1) as f32 * gap
}

/// The wall-clock time ballistics and clip holds are measured against, in
/// milliseconds since egui started.
fn now_ms(ui: &Ui) -> f32 {
    (ui.ctx().input(|input| input.time) as f32) * 1000.0
}

/// A color scaled toward transparent.
#[must_use]
fn fade(color: Color32, opacity: f32) -> Color32 {
    let mut rgba = Rgba::from(color).to_rgba_unmultiplied();
    rgba[3] *= clamp(opacity, 0.0, 1.0);
    Color32::from(Rgba::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
}

/// Linear blend between two colors, in gamma space.
#[must_use]
fn mix(from: Color32, to: Color32, t: f32) -> Color32 {
    let (a, b) = (Rgba::from(from).to_rgba_unmultiplied(), Rgba::from(to).to_rgba_unmultiplied());
    let t = clamp(t, 0.0, 1.0);
    let at = |from: f32, to: f32| from + (to - from) * t;
    Color32::from(Rgba::from_rgba_unmultiplied(at(a[0], b[0]), at(a[1], b[1]), at(a[2], b[2]), at(a[3], b[3])))
}

/// The color at `position` along a list of `(position, color)` stops.
#[must_use]
fn ramp(stops: &[(f32, Color32)], position: f32) -> Color32 {
    match stops {
        [] => Color32::TRANSPARENT,
        [(_, color)] => *color,
        _ => {
            let position = clamp(position, 0.0, 1.0);
            if position <= stops[0].0 {
                return stops[0].1;
            }
            stops
                .windows(2)
                .find(|window| position <= window[1].0)
                .map_or_else(
                    || stops.last().expect("at least two stops").1,
                    |window| {
                        let span = window[1].0 - window[0].0;
                        let t = if span > 0.0 { (position - window[0].0) / span } else { 0.0 };
                        mix(window[0].1, window[1].1, t)
                    },
                )
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::theme::Palette;
    use crate::zones::CLIP_THRESHOLD_DB;

    use super::*;

    fn meter<'a>(channels: &'a [ChannelLevel]) -> LevelMeter<'a> {
        LevelMeter::new(channels).range(-60.0, 0.0)
    }

    #[test]
    fn a_normalized_peak_becomes_decibels() {
        assert_eq!(ChannelLevel::from_linear(0.0).peak_db, SILENCE_DB);
        assert!((ChannelLevel::from_linear(1.0).peak_db - 0.0).abs() < 1e-4);
        assert!(
            (ChannelLevel::from_linear(0.5).peak_db + 6.0206).abs() < 1e-3,
            "half amplitude is -6 dBFS"
        );
    }

    #[test]
    fn slabs_cover_the_range_in_order() {
        let levels = [ChannelLevel::silence()];
        let slabs = meter(&levels).slabs();
        assert_eq!(slabs.len(), DEFAULT_ZONES.len());
        assert_eq!(slabs[0].from, 0.0);
        assert_eq!(slabs.last().expect("slabs").to, 1.0);
        for pair in slabs.windows(2) {
            assert_eq!(pair[0].to, pair[1].from, "bands must meet without gaps");
            assert!(pair[0].to > pair[0].from, "bands must have width");
        }
        assert_eq!(slabs[0].zone, Zone::Ok);
        assert_eq!(slabs.last().expect("slabs").zone, Zone::Clip);
    }

    #[test]
    fn a_zone_boundary_sits_at_its_db() {
        let levels = [ChannelLevel::silence()];
        let slabs = meter(&levels).slabs();
        let warn = &slabs[1];
        assert!(
            (warn.from - db_to_level(-20.0, -60.0, 0.0)).abs() < 1e-6,
            "the warn band starts at -20 dBFS"
        );
    }

    #[test]
    fn gradient_stops_ascend_through_the_zones() {
        let levels = [ChannelLevel::silence()];
        let theme = Theme::dark();
        let stops = meter(&levels).stops(&theme);
        assert_eq!(stops.len(), DEFAULT_ZONES.len());
        for pair in stops.windows(2) {
            assert!(pair[1].0 > pair[0].0, "stops must ascend");
        }
        assert!(stops.first().expect("stops").0 > 0.0);
        assert!(stops.last().expect("stops").0 < 1.0);
    }

    #[test]
    fn the_ramp_holds_each_zone_color_then_blends() {
        let levels = [ChannelLevel::silence()];
        let stops = meter(&levels).stops(&Theme::dark());
        assert_eq!(ramp(&stops, 0.0), stops[0].1);
        assert_eq!(ramp(&stops, 1.0), stops.last().expect("stops").1);
        let mid = (stops[0].0 + stops[1].0) / 2.0;
        assert_ne!(ramp(&stops, mid), stops[0].1, "the blend must actually move");
    }

    #[test]
    fn lanes_stack_across_the_meter() {
        let levels = [ChannelLevel::new(-20.0), ChannelLevel::new(-30.0)];
        let m = meter(&levels);
        let bars = Rect::from_min_max(pos2(0.0, 0.0), pos2(20.0, 100.0));
        let left = m.lane_rect(bars, 0, 8.0);
        let right = m.lane_rect(bars, 1, 8.0);
        assert_eq!(left.width(), 8.0);
        assert_eq!(right.left(), 10.0, "one 8pt lane plus a 2pt gap");
        assert_eq!(right.top(), 0.0);
        assert_eq!(left.bottom(), 100.0);

        let wide = LevelMeter::stereo(&levels).orientation(Orientation::Horizontal);
        let bars = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 20.0));
        assert_eq!(wide.lane_rect(bars, 1, 8.0).top(), 10.0);
    }

    #[test]
    fn the_clip_lamp_follows_the_threshold() {
        let policy = ClipPolicy { threshold_db: CLIP_THRESHOLD_DB, hold_ms: 100.0 };
        let mut latch = ClipLatch::default();
        assert!(latch.report(-0.5, 0.0, policy));
        assert!(!latch.report(-12.0, 500.0, policy), "the lamp falls once the hold expires");
        assert_eq!(latch.count(), 1);
    }

    #[test]
    fn more_channels_need_more_room() {
        let levels = [ChannelLevel::new(-6.0), ChannelLevel::new(-12.0)];
        let theme = Theme::dark();
        let stereo = LevelMeter::stereo(&levels).desired_size(&theme);
        let mono = LevelMeter::mono(&levels[0]).desired_size(&theme);
        assert!(stereo.x > mono.x, "a second channel widens a vertical meter");

        let flat_stereo = LevelMeter::stereo(&levels)
            .orientation(Orientation::Horizontal)
            .desired_size(&theme);
        let flat_mono = LevelMeter::mono(&levels[0])
            .orientation(Orientation::Horizontal)
            .desired_size(&theme);
        assert!(flat_stereo.y > flat_mono.y, "and heightens a horizontal one");
    }

    #[test]
    fn a_scale_and_a_readout_take_extra_room() {
        let levels = [ChannelLevel::new(-6.0)];
        let theme = Theme::dark();
        let bare = LevelMeter::mono(&levels[0]).desired_size(&theme);
        let dressed = LevelMeter::mono(&levels[0]).scale(true).readout(true).clip(true).desired_size(&theme);
        assert!(dressed.x > bare.x, "the scale sits beside the bar");
        assert!(dressed.y > bare.y, "the lamp and readout stack with it");
    }

    /// Render one headless frame and return what the meter drew.
    fn paint(levels: &[ChannelLevel]) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(LevelMeter::new(levels).raw().scale(true).readout(true).clip(true));
        });
        output.shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// The filled rect painted in a zone's color, if there is one.
    fn zone_fill(shapes: &[egui::Shape], color: egui::Color32) -> Option<Rect> {
        shapes.iter().find_map(|shape| match shape {
            egui::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
            _ => None,
        })
    }

    #[test]
    fn a_level_paints_a_bar_up_to_its_zone() {
        let palette = Palette::dark();
        let shapes = paint(&[ChannelLevel::new(-12.0)]);
        let warn = zone_fill(&shapes, palette.meter_warn).expect("a -12 dB peak fills into the warn zone");
        let ok = zone_fill(&shapes, palette.meter_ok).expect("and lights the zone below it");
        assert!(warn.height() > 0.0 && warn.bottom() <= ok.top() + 1.0, "bands must stack");
        assert_eq!(zone_fill(&shapes, palette.meter_clip), None, "the clip zone stays dark");
    }

    #[test]
    fn silence_paints_no_bar_but_still_shows_the_track() {
        let palette = Palette::dark();
        let shapes = paint(&[ChannelLevel::silence()]);
        assert!(zone_fill(&shapes, palette.meter_ok).is_none(), "nothing is lit at -inf");
        assert!(!shapes.is_empty(), "the frame, scale and lamp still draw");
    }

    #[test]
    fn full_scale_reaches_the_clip_zone() {
        let palette = Palette::dark();
        let shapes = paint(&[ChannelLevel::new(0.0)]);
        assert!(zone_fill(&shapes, palette.meter_clip).is_some(), "0 dBFS must reach the top zone");
    }
}
