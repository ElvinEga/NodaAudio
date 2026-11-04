//! The fader: a channel's gain, in decibels, under your finger.
//!
//! [`Fader`] is a value control, not a param control. It is handed a
//! `&mut f32` in dBFS and writes the new value back, so it binds to a truce
//! `FloatParam` in a plugin editor exactly as happily as to a plain field in
//! a standalone app or a test.

use egui::{
    Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, TextEdit, Ui, Vec2, pos2,
};

use crate::db::{DEFAULT_MAX_DB, DEFAULT_MIN_DB, FormatOptions, clamp, clamp_db, format_db};
use crate::taper::Taper;
use crate::ui::Orientation;
use crate::ui::meters::db_scale::{DbScale, MarkEdge};
use crate::ui::theme::Theme;
use crate::ui::{Edge, peel};

/// Where a console fader snaps to while you drag it past it.
pub const DEFAULT_DETENTS: [f32; 6] = [-48.0, -36.0, -24.0, -12.0, -6.0, 0.0];

/// How the track is painted around the handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaderStyle {
    /// Both halves of the track are filled toward the handle, as on a
    /// digital console.
    Iso,
    /// Only the quiet side of the handle is filled.
    Plain,
}

/// What the pointer does when a drag starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DragMode {
    /// The handle moves to the pointer. Fast, and it jumps.
    Grab,
    /// The handle moves by how far the pointer moved. Never jumps, which is
    /// what you want on a long-throw fader.
    Relative,
}

/// A fader over a dB range.
///
/// ```ignore
/// let mut db = params.gain.raw_target() as f32;
/// if Fader::new(&mut db).range(-60.0, 6.0).taper(Taper::AUDIO).ui(ui).changed() {
///     params.gain.set_value(db as f64);
/// }
/// ```
#[must_use = "a Fader draws nothing until it is added to a Ui"]
pub struct Fader<'a> {
    value: &'a mut f32,
    min_db: f32,
    max_db: f32,
    taper: Taper,
    orientation: Orientation,
    style: FaderStyle,
    drag: DragMode,
    detents: &'a [f32],
    detent_db: f32,
    reset_db: Option<f32>,
    show_scale: bool,
    show_value: bool,
    track_thickness: Option<f32>,
    thumb_size: Option<f32>,
    id: Option<egui::Id>,
}

impl<'a> Fader<'a> {
    /// A fader driving `value`, in dBFS over -60..0 with the console law.
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_MAX_DB,
            taper: Taper::AUDIO,
            orientation: Orientation::Vertical,
            style: FaderStyle::Iso,
            drag: DragMode::Relative,
            detents: &DEFAULT_DETENTS,
            detent_db: 0.9,
            reset_db: Some(0.0),
            show_scale: true,
            show_value: true,
            track_thickness: None,
            thumb_size: None,
            id: None,
        }
    }

    /// The dB range the throw covers.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// The law mapping the throw to decibels.
    #[must_use]
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
        self
    }

    /// Which way the fader runs.
    #[must_use]
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Track painting.
    #[must_use]
    pub fn style(mut self, style: FaderStyle) -> Self {
        self.style = style;
        self
    }

    /// How a drag picks up the handle.
    #[must_use]
    pub fn drag(mut self, mode: DragMode) -> Self {
        self.drag = mode;
        self
    }

    /// The dB values the handle snaps to. Empty for a smooth throw.
    #[must_use]
    pub fn detents(mut self, detents: &'a [f32]) -> Self {
        self.detents = detents;
        self
    }

    /// How close to a detent, in dB, before the handle catches.
    #[must_use]
    pub fn detent_width(mut self, detent_db: f32) -> Self {
        self.detent_db = detent_db;
        self
    }

    /// The value a double-click returns to, or `None` to disable it.
    #[must_use]
    pub fn reset_db(mut self, reset_db: Option<f32>) -> Self {
        self.reset_db = reset_db;
        self
    }

    /// Draw graduations beside the throw.
    #[must_use]
    pub fn scale(mut self, show: bool) -> Self {
        self.show_scale = show;
        self
    }

    /// Show the value, click to type a new one.
    #[must_use]
    pub fn value_readout(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Track thickness in points.
    #[must_use]
    pub fn track_thickness(mut self, thickness: f32) -> Self {
        self.track_thickness = Some(thickness);
        self
    }

    /// Handle size in points.
    #[must_use]
    pub fn thumb_size(mut self, size: f32) -> Self {
        self.thumb_size = Some(size);
        self
    }

    /// Pin the identity the interaction state is stored under.
    #[must_use]
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Room the readout needs along the fader's long axis.
    fn readout_extent(&self, theme: &Theme) -> f32 {
        let font = theme.size.value_font();
        match self.orientation {
            // A horizontal readout prints a full "-12.00 dB" beside the
            // track, so it needs text width, not just a line height.
            Orientation::Horizontal => font * 5.5 + theme.size.gap(),
            Orientation::Vertical => font + theme.size.gap(),
        }
    }

    /// Space the fader would like.
    #[must_use]
    pub fn desired_size(&self, theme: &Theme) -> Vec2 {
        let across = self.thickness(theme).max(self.thumb(theme));
        let scale = if self.show_scale { self.scale_for(theme).desired_thickness(theme) + theme.size.gap() } else { 0.0 };
        let readout = if self.show_value { self.readout_extent(theme) } else { 0.0 };
        match self.orientation {
            Orientation::Vertical => Vec2::new(across + scale, 160.0 + readout),
            Orientation::Horizontal => Vec2::new(220.0 + readout, across + scale),
        }
    }

    /// Position the handle at the current value.
    #[must_use]
    pub fn position(&self) -> f32 {
        self.taper.to_position(*self.value, self.min_db, self.max_db)
    }

    fn thickness(&self, theme: &Theme) -> f32 {
        self.track_thickness.unwrap_or(theme.size.track_thickness() * 3.0)
    }

    fn thumb(&self, theme: &Theme) -> f32 {
        self.thumb_size.unwrap_or(theme.size.thumb_size())
    }

    fn scale_for(&self, theme: &Theme) -> DbScale<'a> {
        let major = 5.0 + theme.size.label_font() * 0.3;
        let scale = match self.orientation {
            Orientation::Vertical => DbScale::vertical().mark_edge(MarkEdge::Start),
            Orientation::Horizontal => DbScale::horizontal().mark_edge(MarkEdge::End),
        };
        scale
            .range(self.min_db, self.max_db)
            .taper(self.taper)
            .labels(true)
            .mark_lengths(major, major - 2.0)
    }
}

impl<'a> egui::Widget for Fader<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let desired = self.desired_size(&theme);
        let (rect, response) = ui.allocate_at_least(desired, Sense::click_and_drag());
        let id = self.id.unwrap_or_else(|| response.id);
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let gap = theme.size.gap();
        let (quiet_end, scale_side) = match self.orientation {
            Orientation::Vertical => (Edge::Bottom, Edge::Right),
            Orientation::Horizontal => (Edge::Left, Edge::Bottom),
        };
        let mut rest = rect;
        let readout_rect = if self.show_value {
            let (strip, leftover) = peel(rest, quiet_end, self.readout_extent(&theme));
            rest = leftover;
            Some(strip)
        } else {
            None
        };
        let scale_rect = if self.show_scale {
            let thickness = self.scale_for(&theme).desired_thickness(&theme);
            let (strip, leftover) = peel(rest, scale_side, thickness + gap);
            rest = leftover;
            Some(strip)
        } else {
            None
        };
        let track_area = rest;

        let before = *self.value;
        let mut state = take_state(ui, id);

        if let Some(strip) = readout_rect {
            self.paint_readout(ui, &theme, strip, &mut state);
        } else {
            state.editing = false;
        }

        if !state.editing {
            if response.drag_started() {
                state.drag_start = ui.ctx().pointer_interact_pos().map(|p| (p, self.position()));
            }
            if response.dragged() {
                let position = drag_position(&state, self.drag, self.orientation, track_area, ui);
                self.apply_position(&mut state, position);
            }
            if response.drag_stopped() {
                state.drag_start = None;
            }
            if response.double_clicked_by(PointerButton::Primary) {
                if let Some(db) = self.reset_db {
                    *self.value = clamp_db(db, self.min_db, self.max_db, false);
                    state.engaged_detent = None;
                }
            }
            self.apply_keys(ui, &response, &mut state);
        }

        let highlight = response.hovered() || response.has_focus();
        self.paint_track(ui, &theme, track_area, self.position(), highlight);
        if let Some(strip) = scale_rect {
            self.scale_for(&theme).paint(ui, strip);
        }

        write_state(ui, id, state);
        if *self.value != before {
            let mut response = response;
            response.mark_changed();
            return response;
        }
        response
    }
}

impl Fader<'_> {
    /// A drag position, in 0..1 along the throw, with the detents applied.
    fn apply_position(&mut self, state: &mut FaderState, position: f32) {
        let value = self.taper.to_value(position, self.min_db, self.max_db);
        *self.value = self.snap(state, value);
    }

    /// The dB a keyboard step moves by: one hundredth of the throw, never
    /// finer than a tenth of a dB, and a decade per page.
    fn key_step(&self) -> f32 {
        ((self.max_db - self.min_db) / 100.0).max(0.1)
    }

    /// Snap toward the nearest detent, and stay caught on one until the
    /// pointer leaves it.
    fn snap(&self, state: &mut FaderState, value: f32) -> f32 {
        if self.detents.is_empty() || self.detent_db <= 0.0 {
            state.engaged_detent = None;
            return value;
        }
        if let Some(caught) = state.engaged_detent {
            if (caught - value).abs() <= self.detent_db {
                return caught;
            }
            state.engaged_detent = None;
        }
        let nearest = self
            .detents
            .iter()
            .copied()
            .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()));
        match nearest {
            Some(detent) if (detent - value).abs() <= self.detent_db => {
                state.engaged_detent = Some(detent);
                detent
            }
            _ => value,
        }
    }

    /// Arrow keys nudge, Page keys jump, Home and End go to the ends.
    fn apply_keys(&mut self, ui: &Ui, response: &Response, state: &mut FaderState) {
        if !response.has_focus() {
            if response.clicked_by(PointerButton::Primary) {
                response.request_focus();
            }
            return;
        }
        let step = self.key_step();
        let pressed = |key| ui.ctx().input(|input| input.key_pressed(key));
        let mut delta = 0.0;
        if pressed(Key::ArrowUp) || pressed(Key::ArrowRight) {
            delta += step;
        }
        if pressed(Key::ArrowDown) || pressed(Key::ArrowLeft) {
            delta -= step;
        }
        if pressed(Key::PageUp) {
            delta += step * 10.0;
        }
        if pressed(Key::PageDown) {
            delta -= step * 10.0;
        }
        if delta != 0.0 {
            let from = (*self.value).max(self.min_db);
            let value = self.snap(state, clamp(from + delta, self.min_db, self.max_db));
            *self.value = clamp_db(value, self.min_db, self.max_db, false);
        }
        if pressed(Key::Home) {
            *self.value = self.min_db;
            state.engaged_detent = None;
        }
        if pressed(Key::End) {
            *self.value = self.max_db;
            state.engaged_detent = None;
        }
    }

    /// The readout, or the text box once it has been clicked.
    fn paint_readout(&mut self, ui: &mut Ui, theme: &Theme, strip: Rect, state: &mut FaderState) {
        let palette = &theme.palette;
        let font = FontId::monospace(theme.size.value_font());
        let id = self.id.unwrap_or_else(|| ui.id()).with("noda-fader-value");

        if state.editing {
            let response = ui.put(
                strip,
                TextEdit::singleline(&mut state.buffer)
                    .hint_text("dB")
                    .font(font.clone()),
            );
            if !state.focused {
                response.request_focus();
                state.focused = true;
            }
            let submitted = ui.ctx().input(|input| input.key_pressed(Key::Enter));
            if submitted || response.lost_focus() {
                if let Some(db) = parse_db(&state.buffer) {
                    *self.value = clamp_db(db, self.min_db, self.max_db, false);
                }
                state.editing = false;
                state.focused = false;
            }
            return;
        }

        let text = format_db(
            *self.value,
            FormatOptions { decimals: 2, unit: true, ..FormatOptions::default() },
        );
        painter_text(ui, strip, &text, font, palette.foreground);
        let response = ui.interact(strip, id, Sense::click());
        if response.clicked_by(PointerButton::Primary) {
            state.buffer = format_db(
                *self.value,
                FormatOptions { decimals: 2, unit: false, ..FormatOptions::default() },
            );
            state.editing = true;
            state.focused = false;
            response.request_focus();
        }
        if response.clicked_by(PointerButton::Secondary) {
            if let Some(db) = self.reset_db {
                *self.value = clamp_db(db, self.min_db, self.max_db, false);
            }
        }
        if response.hovered() {
            response.on_hover_text("Click to type a value");
        }
    }

    /// Track, fill and handle.
    fn paint_track(&self, ui: &Ui, theme: &Theme, area: Rect, position: f32, highlight: bool) {
        let palette = &theme.palette;
        let painter = ui.painter_at(area);
        let thickness = self.thickness(theme).min(self.orientation.thickness(area));
        let track = cross_rect(area, self.orientation, thickness);

        painter.rect_filled(track, thickness / 2.0, palette.track);
        painter.rect_stroke(
            track.shrink(0.5),
            thickness / 2.0,
            Stroke::new(1.0_f32, palette.border),
            StrokeKind::Inside,
        );

        let fill = palette.primary.linear_multiply(0.85);
        for (from, to) in fill_spans(self.style, position) {
            let low = self.orientation.along(track, from);
            let high = self.orientation.along(track, to);
            let span = match self.orientation {
                Orientation::Vertical => Rect::from_min_max(
                    pos2(track.left(), high.min(low)),
                    pos2(track.right(), high.max(low)),
                ),
                Orientation::Horizontal => Rect::from_min_max(
                    pos2(high.min(low), track.top()),
                    pos2(high.max(low), track.bottom()),
                ),
            };
            painter.rect_filled(span, thickness / 2.0, fill);
        }

        // The handle: a cap across the throw, with a line at the exact
        // value so the graduation it lines up with stays readable.
        let thumb = self.thumb(theme);
        let along = self.orientation.along(area, position);
        let cap = match self.orientation {
            Orientation::Vertical => Rect::from_center_size(pos2(area.center().x, along), Vec2::new(thumb, thumb * 1.6)),
            Orientation::Horizontal => Rect::from_center_size(pos2(along, area.center().y), Vec2::new(thumb * 1.6, thumb)),
        };
        let radius = theme.radius.sm().min(thumb / 2.0);
        painter.rect_filled(cap, radius, palette.surface);
        let edge = if highlight { palette.ring } else { palette.border };
        painter.rect_stroke(
            cap,
            radius,
            Stroke::new(if highlight { 1.5_f32 } else { 1.0_f32 }, edge),
            StrokeKind::Inside,
        );
        let mark = stroke_span(self.orientation, cap, thickness.max(1.5));
        painter.rect_filled(mark, 0.0, palette.primary);
    }
}

/// The spans of track to fill for a handle at `position`.
#[must_use]
fn fill_spans(style: FaderStyle, position: f32) -> Vec<(f32, f32)> {
    match style {
        FaderStyle::Plain => vec![(0.0, position)],
        FaderStyle::Iso => vec![(0.0, position), (position, 1.0)],
    }
}

/// A thin bar across a fader cap, centered on it, for the value line.
fn stroke_span(orientation: Orientation, cap: Rect, thickness: f32) -> Rect {
    match orientation {
        Orientation::Vertical => Rect::from_min_max(
            pos2(cap.left() + 2.0, cap.center().y - thickness / 2.0),
            pos2(cap.right() - 2.0, cap.center().y + thickness / 2.0),
        ),
        Orientation::Horizontal => Rect::from_min_max(
            pos2(cap.center().x - thickness / 2.0, cap.top() + 2.0),
            pos2(cap.center().x + thickness / 2.0, cap.bottom() - 2.0),
        ),
    }
}

/// The rect of `thickness` centered across `area`, spanning its length.
fn cross_rect(area: Rect, orientation: Orientation, thickness: f32) -> Rect {
    let half = thickness / 2.0;
    match orientation {
        Orientation::Vertical => Rect::from_min_max(
            pos2(area.center().x - half, area.top()),
            pos2(area.center().x + half, area.bottom()),
        ),
        Orientation::Horizontal => Rect::from_min_max(
            pos2(area.left(), area.center().y - half),
            pos2(area.right(), area.center().y + half),
        ),
    }
}

/// Read a typed value: `"-12"`, `"+3.5 dB"`, `"6"`; nothing else counts.
#[must_use]
pub fn parse_db(text: &str) -> Option<f32> {
    let cleaned = text.trim().trim_end_matches("dB").trim().replace('−', "-");
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f32>().ok().filter(|value| value.is_finite())
}

/// The interaction state a fader keeps between frames.
#[derive(Clone, Debug, Default)]
struct FaderState {
    /// Where the pointer was when the drag began, and the position the
    /// handle was showing. A relative throw moves by the pointer's travel
    /// from here, so it never jumps when grabbed.
    drag_start: Option<(Pos2, f32)>,
    /// The detent the handle is currently caught on.
    engaged_detent: Option<f32>,
    /// The value box is open.
    editing: bool,
    /// The value box has been given focus.
    focused: bool,
    /// What has been typed so far.
    buffer: String,
}

fn take_state(ui: &Ui, id: egui::Id) -> FaderState {
    ui.ctx().data(|data| data.get_temp::<FaderState>(id.with("noda-fader"))).unwrap_or_default()
}

fn write_state(ui: &Ui, id: egui::Id, state: FaderState) {
    ui.ctx().data_mut(|data| data.insert_temp(id.with("noda-fader"), state));
}

/// Where the handle was when the drag began: the pointer, and the position
/// it was showing.
fn drag_position(state: &FaderState, mode: DragMode, orientation: Orientation, area: Rect, ui: &Ui) -> f32 {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return state.drag_start.map_or(0.0, |(_, position)| position);
    };
    let position = along_to_position(orientation, area, pointer);
    match mode {
        DragMode::Grab => position,
        DragMode::Relative => state.drag_start.map_or(position, |(start_pointer, start_position)| {
            let travel = orientation.length(area).max(1.0);
            let moved = orientation.axis_delta(pointer - start_pointer) / travel;
            clamp(start_position + moved, 0.0, 1.0)
        }),
    }
}

/// Where a pointer sits along the throw, 0..1.
fn along_to_position(orientation: Orientation, area: Rect, pointer: Pos2) -> f32 {
    let length = orientation.length(area).max(1.0);
    let along = match orientation {
        Orientation::Vertical => area.bottom() - pointer.y,
        Orientation::Horizontal => pointer.x - area.left(),
    };
    clamp(along / length, 0.0, 1.0)
}

/// Draw centered text without needing a layout row.
fn painter_text(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    ui.painter_at(rect).text(rect.center(), Align2::CENTER_CENTER, text, font, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fader(value: &mut f32) -> Fader<'_> {
        Fader::new(value).range(-60.0, 6.0)
    }

    #[test]
    fn the_handle_sits_at_unity_on_a_console_law() {
        let mut value = 0.0;
        assert!((fader(&mut value).position() - 0.75).abs() < 1e-5);
        let mut deep = -60.0;
        assert_eq!(fader(&mut deep).position(), 0.0);
        let mut loud = 6.0;
        assert_eq!(fader(&mut loud).position(), 1.0);
    }

    #[test]
    fn drag_positions_round_trip_through_the_taper() {
        let mut value = -12.0;
        let f = fader(&mut value);
        for position in [0.0, 0.2, 0.5, 0.75, 0.9, 1.0] {
            let back = f.taper.to_position(f.taper.to_value(position, f.min_db, f.max_db), f.min_db, f.max_db);
            assert!((back - position).abs() < 1e-4, "{position} did not round trip");
        }
    }

    #[test]
    fn a_caught_detent_holds_until_the_pointer_leaves() {
        let mut value = 0.0;
        let mut state = FaderState::default();
        let f = fader(&mut value).detents(&[0.0, -12.0]);
        assert_eq!(f.snap(&mut state, -0.5), 0.0, "close enough to catch");
        assert_eq!(f.snap(&mut state, -0.8), 0.0, "still caught inside the window");
        assert_eq!(f.snap(&mut state, -1.4), -1.4, "released past it");
        assert_eq!(f.snap(&mut state, -12.4), -12.0, "and caught by the next one");
    }

    #[test]
    fn detents_can_be_switched_off() {
        let mut value = 0.0;
        let mut state = FaderState::default();
        let f = fader(&mut value).detents(&[]);
        assert_eq!(f.snap(&mut state, -0.5), -0.5);
    }

    #[test]
    fn typed_values_are_parsed_and_junk_rejected() {
        assert_eq!(parse_db("-12"), Some(-12.0));
        assert_eq!(parse_db("+3.5 dB"), Some(3.5));
        assert_eq!(parse_db(" -0.25dB "), Some(-0.25));
        assert_eq!(parse_db(""), None);
        assert_eq!(parse_db("unity"), None);
        assert_eq!(parse_db("-inf"), None, "silence is not a fader position");
    }

    #[test]
    fn the_iso_style_fills_both_sides_of_the_handle() {
        assert_eq!(fill_spans(FaderStyle::Plain, 0.4), vec![(0.0, 0.4)]);
        assert_eq!(fill_spans(FaderStyle::Iso, 0.4), vec![(0.0, 0.4), (0.4, 1.0)]);
    }

    #[test]
    fn a_pointer_position_maps_from_the_quiet_end() {
        let tall = Rect::from_min_max(pos2(0.0, 0.0), pos2(20.0, 100.0));
        assert_eq!(along_to_position(Orientation::Vertical, tall, pos2(10.0, 100.0)), 0.0);
        assert_eq!(along_to_position(Orientation::Vertical, tall, pos2(10.0, 25.0)), 0.75);
        let wide = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 20.0));
        assert_eq!(along_to_position(Orientation::Horizontal, wide, pos2(50.0, 10.0)), 0.5);
    }

    #[test]
    fn the_track_is_centred_across_the_area() {
        let tall = Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 200.0));
        let track = cross_rect(tall, Orientation::Vertical, 6.0);
        assert_eq!(track.width(), 6.0);
        assert_eq!(track.height(), 200.0);
        assert_eq!(track.center().x, 20.0);
    }

    #[test]
    fn a_scale_and_a_readout_widen_the_fader() {
        let mut value = 0.0;
        let theme = Theme::dark();
        let bare = fader(&mut value).scale(false).value_readout(false).desired_size(&theme);
        let dressed = fader(&mut value).desired_size(&theme);
        assert!(dressed.x > bare.x);
        assert!(dressed.y > bare.y);
    }

    #[test]
    fn keyboard_steps_scale_with_the_throw() {
        let mut value = 0.0;
        assert_eq!(fader(&mut value).key_step(), 0.66);
        let narrow = Fader::new(&mut value).range(-24.0, 0.0).key_step();
        assert_eq!(narrow, 0.24, "a shorter throw steps finer");
    }
}
