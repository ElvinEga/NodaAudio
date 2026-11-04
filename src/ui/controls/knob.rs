//! A rotary control: one parameter, turned by hand.
//!
//! A [`Knob`] is a value control, not a param control. It is handed a
//! `&mut f32` in the parameter's own engineering units and writes back into
//! it, so binding to a truce `FloatParam` in a plugin editor, a plain field
//! in a standalone app, or a number in a test all look the same from here.
//! [`Reading`] is what keeps those units honest on screen: a pan reads
//! `30L`, a gain reads `-12.3 dB`, and both types back in the way they show.

use std::f32::consts::{PI, TAU};

use egui::epaint::PathStroke;
use egui::{
    Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, Response, Sense, Stroke, TextEdit, Ui,
    Vec2,
};

use crate::db::{FormatOptions, clamp, format_db};
use crate::taper::Taper;
use crate::ui::controls::fader::parse_db;
use crate::ui::theme::Theme;
use crate::ui::{DragMode, Edge, Orientation, peel};

/// Where the travel starts, in radians measured with screen y pointing
/// down: 135° is the lower left, so a knob sweeps up over the top.
const START_ANGLE: f32 = 0.75 * PI;

/// The stock travel: 270°, with the gap under the dial.
const DEFAULT_SWEEP: f32 = 1.5 * PI;

/// The longest arc one polyline segment may span, in radians.
const SEGMENT: f32 = PI / 24.0;

/// How far a pointer travels for a full turn, as a multiple of the dial.
const THROW_DIAMETERS: f32 = 3.0;

/// Scroll sensitivity, as a fraction of the travel per pixel.
const SCROLL_RATE: f32 = 0.004;

/// Room the graduation marks take outside the ring, in points.
const TICK_ROOM: f32 = 7.0;

/// Marks around the ring, so a tenth of the travel sits between two.
const MARKS: usize = 10;

/// How close to centre, as a fraction of the range, before a bipolar knob
/// catches.
const DETENT_FRACTION: f32 = 0.015;

/// How a value is written on the dial, and read back from the text box.
#[derive(Clone, Copy, Debug)]
pub enum Reading<'a> {
    /// A plain number, optionally with a unit: `-12.35`, or `-12.35 %`.
    Number { decimals: usize, unit: Option<&'a str> },
    /// Decibels through [`format_db`]: `-12.3 dB`, `-inf dB`.
    Db,
    /// A balance over -1..1: `C`, `30L`, `50R`.
    Pan,
}

impl Reading<'_> {
    /// Two decimals, no unit.
    #[must_use]
    pub fn plain() -> Self {
        Reading::Number { decimals: 2, unit: None }
    }

    /// The value as displayed text.
    #[must_use]
    pub fn format(&self, value: f32) -> String {
        match self {
            Reading::Number { decimals, unit } => {
                let number = format!("{:.*}", *decimals, value);
                match unit {
                    Some(unit) => format!("{number} {unit}"),
                    None => number,
                }
            }
            Reading::Db => {
                format_db(value, FormatOptions { decimals: 1, ..FormatOptions::default() })
            }
            Reading::Pan => format_pan(value),
        }
    }

    /// What the text box opens with: the number on its own, so what you
    /// type is what the control stores.
    #[must_use]
    pub fn edit_text(&self, value: f32) -> String {
        match self {
            Reading::Number { decimals, .. } => format!("{:.*}", *decimals, value),
            Reading::Db | Reading::Pan => format!("{value:.2}"),
        }
    }

    /// Typed text read back as a value. Junk gives `None` and leaves the
    /// control where it was.
    #[must_use]
    pub fn parse(&self, text: &str) -> Option<f32> {
        match self {
            Reading::Number { unit, .. } => {
                let trimmed = text.trim();
                let bare = match unit {
                    Some(unit) => trimmed.trim_end_matches(unit).trim(),
                    None => trimmed,
                };
                bare.replace('−', "-").parse::<f32>().ok().filter(|value| value.is_finite())
            }
            Reading::Db => parse_db(text),
            Reading::Pan => parse_pan(text),
        }
    }
}

/// A balance value, -1..1, as `C`, `30L` or `50R`.
///
/// Values past full left or right print as 100 rather than wrapping, because
/// a knob that has been dragged past its stop should say so.
#[must_use]
pub fn format_pan(value: f32) -> String {
    let percent = (value.clamp(-1.0, 1.0) * 100.0).round() as i32;
    match percent.cmp(&0) {
        std::cmp::Ordering::Equal => "C".to_string(),
        std::cmp::Ordering::Less => format!("{}L", -percent),
        std::cmp::Ordering::Greater => format!("{percent}R"),
    }
}

/// [`format_pan`] backwards: `30L`, `50R`, `C`, or a bare value.
#[must_use]
pub fn parse_pan(text: &str) -> Option<f32> {
    let cleaned = text.trim().to_lowercase().replace(' ', "");
    if cleaned.is_empty() {
        return None;
    }
    if cleaned == "c" {
        return Some(0.0);
    }
    let bytes = cleaned.as_bytes();
    match bytes.last()? {
        b'l' => cleaned[..cleaned.len() - 1].parse::<f32>().ok().map(|p| -p / 100.0),
        b'r' => cleaned[..cleaned.len() - 1].parse::<f32>().ok().map(|p| p / 100.0),
        _ => cleaned.parse::<f32>().ok().filter(|value| value.is_finite()),
    }
}

/// A knob over any range, drawn as a ring, a cap and a pointer.
///
/// ```ignore
/// let mut pan = params.pan.raw_target() as f32;
/// if Knob::pan(&mut pan).label("PAN").ui(ui).changed() {
///     params.pan.set_value(pan as f64);
/// }
/// ```
#[must_use = "a Knob draws nothing until it is added to a Ui"]
pub struct Knob<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    taper: Taper,
    reading: Reading<'a>,
    label: Option<&'a str>,
    sweep: f32,
    bipolar: bool,
    center_detent: bool,
    show_scale: bool,
    show_value: bool,
    drag: DragMode,
    throw: Option<f32>,
    diameter: Option<f32>,
    reset: Option<f32>,
    id: Option<egui::Id>,
}

impl<'a> Knob<'a> {
    /// A knob over 0..1, reading plain numbers.
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            min: 0.0,
            max: 1.0,
            taper: Taper::Linear,
            reading: Reading::plain(),
            label: None,
            sweep: DEFAULT_SWEEP,
            bipolar: false,
            center_detent: false,
            show_scale: true,
            show_value: true,
            drag: DragMode::Relative,
            throw: None,
            diameter: None,
            reset: None,
            id: None,
        }
    }

    /// A centre-detented balance knob over -1..1.
    pub fn pan(value: &'a mut f32) -> Self {
        Self::new(value)
            .range(-1.0, 1.0)
            .reading(Reading::Pan)
            .bipolar(true)
            .center_detent(true)
            .reset(Some(0.0))
    }

    /// The range the dial covers, in the value's own units.
    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// How the range maps to the turn.
    #[must_use]
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
        self
    }

    /// How the value reads and types back.
    #[must_use]
    pub fn reading(mut self, reading: Reading<'a>) -> Self {
        self.reading = reading;
        self
    }

    /// The name printed under the dial.
    #[must_use]
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Travel in degrees. 270 is the stock sweep; 360 makes an endless dial
    /// with the gap closed under it.
    #[must_use]
    pub fn sweep_degrees(mut self, degrees: f32) -> Self {
        self.sweep = degrees.to_radians();
        self
    }

    /// Fill from the centre rather than from the start, for a knob whose
    /// quiet or neutral point is in the middle.
    #[must_use]
    pub fn bipolar(mut self, bipolar: bool) -> Self {
        self.bipolar = bipolar;
        self
    }

    /// Hold the pointer at the centre of the range while the value is near
    /// it, so a balance can be returned to exactly neutral.
    #[must_use]
    pub fn center_detent(mut self, detent: bool) -> Self {
        self.center_detent = detent;
        self
    }

    /// Draw graduations around the ring.
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

    /// How a drag picks up the pointer: `Grab` turns the dial to where the
    /// pointer is, `Relative` turns it by how far the pointer travelled.
    #[must_use]
    pub fn drag(mut self, mode: DragMode) -> Self {
        self.drag = mode;
        self
    }

    /// Points of vertical drag for a full turn.
    #[must_use]
    pub fn throw(mut self, points: f32) -> Self {
        self.throw = Some(points);
        self
    }

    /// Dial diameter in points, before the graduations are added.
    #[must_use]
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = Some(diameter);
        self
    }

    /// The value a secondary click returns to, or `None` to disable it.
    #[must_use]
    pub fn reset(mut self, reset: Option<f32>) -> Self {
        self.reset = reset;
        self
    }

    /// Pin the identity the interaction state is stored under.
    #[must_use]
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Space the knob would like.
    #[must_use]
    pub fn desired_size(&self, theme: &Theme) -> Vec2 {
        let gap = theme.size.gap();
        let dial = self.dial(theme);
        let mut height = dial;
        if self.label.is_some() {
            height += theme.size.label_font() + gap;
        }
        if self.show_value {
            height += theme.size.value_font() + gap;
        }
        Vec2::new(dial, height)
    }

    /// Where the value sits along the travel, 0..1.
    #[must_use]
    pub fn position(&self) -> f32 {
        self.taper.to_position(*self.value, self.min, self.max)
    }

    /// The ring radius including the graduation marks, in points.
    #[must_use]
    pub fn dial(&self, theme: &Theme) -> f32 {
        let cap = self.diameter.unwrap_or(theme.size.knob_diameter());
        if self.show_scale { cap + TICK_ROOM * 2.0 } else { cap }
    }

    /// The angle a position turns the pointer to.
    #[must_use]
    pub fn angle(&self, position: f32) -> f32 {
        START_ANGLE + self.sweep * position
    }

    fn throw_points(&self, theme: &Theme) -> f32 {
        self.throw.unwrap_or(self.dial(theme) * THROW_DIAMETERS).max(1.0)
    }
}

impl egui::Widget for Knob<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let desired = self.desired_size(&theme);
        let (rect, response) = ui.allocate_at_least(desired, Sense::click_and_drag());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let id = self.id.unwrap_or_else(|| response.id);
        let gap = theme.size.gap();

        // The dial, its name and its value are one group, kept together in
        // the middle of whatever rect was handed over: a knob stranded at
        // the top of a tall mixer lane reads as a layout mistake.
        let height = desired.y.min(rect.height());
        let band = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), height));
        let mut rest = band;
        let value_rect = if self.show_value {
            let (strip, left) = peel(rest, Edge::Bottom, theme.size.value_font() + gap);
            rest = left;
            Some(strip)
        } else {
            None
        };
        let label_rect = self.label.map(|name| {
            let (strip, left) = peel(rest, Edge::Bottom, theme.size.label_font() + gap);
            rest = left;
            (strip, name)
        });
        let side = rest.width().min(rest.height()).min(self.dial(&theme));
        let dial = Rect::from_min_max(
            rest.center() - Vec2::splat(side * 0.5),
            rest.center() + Vec2::splat(side * 0.5),
        );

        let before = *self.value;
        let mut state = take_state(ui, id);

        if let Some(strip) = value_rect {
            self.paint_readout(ui, &theme, strip, &mut state, id);
        } else {
            state.editing = false;
        }

        if !state.editing {
            if response.drag_started() {
                state.drag_start = ui.ctx().pointer_interact_pos().map(|p| (p, self.position()));
            }
            if response.dragged() {
                let position = self.drag_position(&state, dial, self.throw_points(&theme), ui);
                self.apply_position(&mut state, position);
            }
            if response.drag_stopped() {
                state.drag_start = None;
            }
            if response.hovered() && state.drag_start.is_none() {
                let scrolled = ui.ctx().input(|input| input.smooth_scroll_delta.y);
                if scrolled != 0.0 {
                    let position = clamp(self.position() + scrolled * SCROLL_RATE, 0.0, 1.0);
                    self.apply_position(&mut state, position);
                }
            }
            if response.double_clicked_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Secondary)
            {
                if let Some(value) = self.reset {
                    *self.value = clamp(value, self.min, self.max);
                    state.caught_centre = false;
                }
            }
            self.apply_keys(ui, &response, &mut state);
        }

        if let Some((strip, name)) = label_rect {
            let font = FontId::proportional(theme.size.label_font());
            painter_text(ui, strip, name, font, theme.palette.text_dim);
        }
        self.paint_dial(
            ui,
            &theme,
            dial,
            self.position(),
            response.hovered() || response.has_focus(),
            response.has_focus(),
        );

        write_state(ui, id, state);
        if *self.value != before {
            let mut response = response;
            response.mark_changed();
            return response;
        }
        response
    }
}

impl Knob<'_> {
    /// A turn position, 0..1, from the current gesture.
    fn drag_position(&self, state: &KnobState, dial: Rect, throw: f32, ui: &Ui) -> f32 {
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            return state.drag_start.map_or(self.position(), |(_, position)| position);
        };
        match self.drag {
            DragMode::Grab => angle_to_position(dial.center(), pointer, self.sweep)
                .unwrap_or_else(|| state.drag_start.map_or(self.position(), |(_, p)| p)),
            DragMode::Relative => state.drag_start.map_or(self.position(), |(start, from)| {
                clamp(from + Orientation::Vertical.axis_delta(pointer - start) / throw, 0.0, 1.0)
            }),
        }
    }

    /// A value, in the knob's own units, from a position along the turn.
    fn apply_position(&mut self, state: &mut KnobState, position: f32) {
        let value = self.taper.to_value(position, self.min, self.max);
        *self.value = clamp(self.snap(state, value), self.min, self.max);
    }

    /// Catch on centre and stay caught until the pointer leaves the band.
    fn snap(&self, state: &mut KnobState, value: f32) -> f32 {
        if !self.center_detent {
            state.caught_centre = false;
            return value;
        }
        let centre = (self.min + self.max) * 0.5;
        let band = (self.max - self.min).abs() * DETENT_FRACTION;
        if state.caught_centre {
            if (value - centre).abs() <= band {
                return centre;
            }
            state.caught_centre = false;
        }
        if (value - centre).abs() <= band {
            state.caught_centre = true;
            centre
        } else {
            value
        }
    }

    /// The value a keyboard step moves by: a hundredth of the range.
    fn key_step(&self) -> f32 {
        ((self.max - self.min) / 100.0).abs().max(f32::EPSILON)
    }

    /// Arrows nudge, Page keys jump, Home and End go to the stops.
    fn apply_keys(&mut self, ui: &Ui, response: &Response, state: &mut KnobState) {
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
            let value = clamp(*self.value + delta, self.min, self.max);
            *self.value = self.snap(state, value);
        }
        if pressed(Key::Home) {
            *self.value = self.min;
            state.caught_centre = false;
        }
        if pressed(Key::End) {
            *self.value = self.max;
            state.caught_centre = false;
        }
    }

    /// The readout, or the text box once it has been clicked.
    fn paint_readout(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        strip: Rect,
        state: &mut KnobState,
        base_id: egui::Id,
    ) {
        let palette = &theme.palette;
        let font = FontId::monospace(theme.size.value_font());
        let id = base_id.with("noda-knob-value");

        if state.editing {
            let response = ui.put(
                strip,
                TextEdit::singleline(&mut state.buffer)
                    .hint_text("value")
                    .font(font.clone()),
            );
            if !state.focused {
                response.request_focus();
                state.focused = true;
            }
            let submitted = ui.ctx().input(|input| input.key_pressed(Key::Enter));
            if submitted || response.lost_focus() {
                if let Some(value) = self.reading.parse(&state.buffer) {
                    *self.value = clamp(value, self.min, self.max);
                }
                state.editing = false;
                state.focused = false;
            }
            return;
        }

        painter_text(ui, strip, &self.reading.format(*self.value), font, palette.foreground);
        let response = ui.interact(strip, id, Sense::click());
        if response.clicked_by(PointerButton::Primary) {
            state.buffer = self.reading.edit_text(*self.value);
            state.editing = true;
            state.focused = false;
            response.request_focus();
        }
        if response.clicked_by(PointerButton::Secondary) {
            if let Some(value) = self.reset {
                *self.value = clamp(value, self.min, self.max);
            }
        }
        if response.hovered() {
            response.on_hover_text("Click to type a value");
        }
    }

    /// Ring, cap, pointer and graduations.
    fn paint_dial(
        &self,
        ui: &Ui,
        theme: &Theme,
        area: Rect,
        position: f32,
        highlight: bool,
        focused: bool,
    ) {
        let palette = &theme.palette;
        let painter = ui.painter_at(area);
        let side = area.width().min(area.height());
        // A ring scaled with the dial, so a small lane knob still reads.
        let width = (side / 14.0).clamp(3.0, 6.0);
        let centre = area.center();
        let room = if self.show_scale { TICK_ROOM } else { 0.0 };
        let ring = (area.width().min(area.height()) * 0.5 - room - width * 0.5).max(2.0);

        if self.show_scale {
            self.paint_ticks(&painter, theme, centre, ring + width * 0.5 + 1.0);
        }

        painter.line(
            arc(centre, ring, self.angle(0.0), self.angle(1.0)),
            PathStroke::new(width, palette.track),
        );

        let from = if self.bipolar { 0.5 } else { 0.0 };
        if (position - from).abs() > 1e-4 {
            let value = arc(centre, ring, self.angle(from), self.angle(position));
            painter.line(value, PathStroke::new(width, palette.primary));
        }

        let cap = (ring - width * 0.5 - 3.0).max(1.0);
        let edge = if highlight { palette.ring } else { palette.border };
        painter.circle(centre, cap, palette.surface, Stroke::new(1.0_f32, edge));

        if focused {
            painter.circle_stroke(
                centre,
                ring + width * 0.5 + 1.0,
                Stroke::new(1.0_f32, palette.ring),
            );
        }

        let dir = direction(self.angle(position));
        painter.line_segment(
            [centre + dir * (cap * 0.30), centre + dir * (cap * 0.88)],
            Stroke::new(width.max(1.5), palette.foreground),
        );
    }

    /// Marks every tenth of the travel, with the stops and the centre
    /// printed longer than the rest.
    fn paint_ticks(&self, painter: &egui::Painter, theme: &Theme, centre: Pos2, from: f32) {
        let palette = &theme.palette;
        for mark in 0..=MARKS {
            let position = mark as f32 / MARKS as f32;
            let end = position == 0.0 || (position - 1.0).abs() < 1e-6;
            let middle = self.bipolar && (position - 0.5).abs() < 1e-6;
            let major = end || middle;
            let length = if major { 5.0 } else { 3.0 };
            let color = if major { palette.foreground } else { palette.text_dim };
            let dir = direction(self.angle(position));
            painter.line_segment(
                [centre + dir * from, centre + dir * (from + length)],
                Stroke::new(1.0_f32, color),
            );
        }
    }
}

/// A unit vector at `angle`, measured with screen y pointing down.
#[must_use]
fn direction(angle: f32) -> Vec2 {
    Vec2::new(angle.cos(), angle.sin())
}

/// The arc from `from` to `to` as a polyline, which is how egui draws a
/// curve it has no primitive for.
#[must_use]
fn arc(centre: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    let span = to - from;
    if span.abs() < 1e-4 {
        return vec![centre + direction(from) * radius];
    }
    let steps = (span.abs() / SEGMENT).ceil() as usize + 1;
    (0..=steps)
        .map(|step| {
            let angle = from + span * (step as f32 / steps as f32);
            centre + direction(angle) * radius
        })
        .collect()
}

/// Where a pointer sits on the turn, 0..1, or `None` when it is too near
/// the middle of the dial to have an angle worth trusting.
#[must_use]
fn angle_to_position(centre: Pos2, pointer: Pos2, sweep: f32) -> Option<f32> {
    let delta = pointer - centre;
    if delta.length_sq() < 4.0 {
        return None;
    }
    let swept = (delta.y.atan2(delta.x) - START_ANGLE).rem_euclid(TAU);
    let gap = (TAU - sweep).max(0.0);
    if swept <= sweep {
        Some(swept / sweep)
    } else if swept < sweep + gap * 0.5 {
        // The dead band under the dial splits by which side the pointer is
        // on: left of the gap is the bottom of the range, right is the top.
        Some(1.0)
    } else {
        Some(0.0)
    }
}

/// The interaction state a knob keeps between frames.
#[derive(Clone, Debug, Default)]
struct KnobState {
    /// Where the pointer was when the drag began, and the position it was
    /// showing. A relative turn moves by the pointer's travel from here.
    drag_start: Option<(Pos2, f32)>,
    /// The pointer is caught on the centre detent.
    caught_centre: bool,
    /// The value box is open.
    editing: bool,
    /// The value box has been given focus.
    focused: bool,
    /// What has been typed so far.
    buffer: String,
}

fn take_state(ui: &Ui, id: egui::Id) -> KnobState {
    ui.ctx().data(|data| data.get_temp::<KnobState>(id.with("noda-knob"))).unwrap_or_default()
}

fn write_state(ui: &Ui, id: egui::Id, state: KnobState) {
    ui.ctx().data_mut(|data| data.insert_temp(id.with("noda-knob"), state));
}

/// Draw centered text without needing a layout row.
fn painter_text(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    ui.painter_at(rect).text(rect.center(), Align2::CENTER_CENTER, text, font, color);
}

#[cfg(test)]
mod tests {
    use egui::epaint::ColorMode;

    use super::*;

    fn knob(value: &mut f32) -> Knob<'_> {
        Knob::new(value).range(0.0, 100.0)
    }

    #[test]
    fn the_pointer_turns_with_the_value() {
        let mut value = 0.0;
        let k = knob(&mut value);
        assert!((k.angle(0.0) - START_ANGLE).abs() < 1e-6);
        assert!((k.angle(1.0) - (START_ANGLE + DEFAULT_SWEEP)).abs() < 1e-6);
        assert!((k.angle(0.5) - (START_ANGLE + DEFAULT_SWEEP * 0.5)).abs() < 1e-6);
        // Half way round points straight up, which on a screen is -y.
        let up = direction(k.angle(0.5));
        assert!(up.y < -0.99 && up.x.abs() < 1e-5, "{up}");
    }

    #[test]
    fn a_pointer_angle_maps_back_onto_the_travel() {
        let centre = Pos2::new(50.0, 50.0);
        for position in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let angle = START_ANGLE + DEFAULT_SWEEP * position;
            let pointer = centre + direction(angle) * 20.0;
            let back = angle_to_position(centre, pointer, DEFAULT_SWEEP).expect("an angle");
            assert!((back - position).abs() < 1e-4, "{position} did not round trip");
        }
    }

    #[test]
    fn the_dead_band_under_the_dial_splits_at_the_bottom() {
        let centre = Pos2::new(0.0, 0.0);
        let pointer = |x: f32| centre + Vec2::new(x, 20.0);
        let turn = |x: f32| angle_to_position(centre, pointer(x), DEFAULT_SWEEP).unwrap();
        assert_eq!((turn(-6.0), turn(6.0)), (0.0, 1.0), "bottom left is the quiet end");
        assert_eq!(angle_to_position(centre, centre, DEFAULT_SWEEP), None, "no angle at the hub");
    }

    #[test]
    fn a_bipolar_detent_holds_the_centre_and_releases() {
        let mut value = 0.0;
        let mut state = KnobState::default();
        let k = Knob::pan(&mut value);
        assert_eq!(k.snap(&mut state, -0.01), 0.0, "inside the band it catches");
        assert_eq!(k.snap(&mut state, 0.005), 0.0, "and stays caught");
        assert_eq!(k.snap(&mut state, 0.04), 0.04, "0.04 is past the band");
        let loose = Knob::pan(&mut value).center_detent(false);
        assert_eq!(loose.snap(&mut state, -0.002), -0.002, "switched off, it never catches");
    }

    #[test]
    fn keyboard_steps_scale_with_the_range() {
        let mut value = 0.0;
        assert_eq!(knob(&mut value).key_step(), 1.0);
        assert_eq!(Knob::pan(&mut value).key_step(), 0.02, "a tight range steps finer");
    }

    #[test]
    fn a_pan_reads_its_sides_and_types_back() {
        assert_eq!(format_pan(0.0), "C");
        assert_eq!(format_pan(-0.3), "30L");
        assert_eq!(format_pan(0.5), "50R");
        assert_eq!(format_pan(-1.0), "100L", "past the stop reads as the stop");
        assert_eq!(parse_pan("30L"), Some(-0.3));
        assert_eq!(parse_pan("50 r"), Some(0.5));
        assert_eq!(parse_pan("c"), Some(0.0));
        assert_eq!(parse_pan("0.25"), Some(0.25), "a bare number is the value");
        assert_eq!(parse_pan("pan"), None);
        assert_eq!(parse_pan(""), None);

        let reading: Reading<'_> = Reading::Pan;
        let typed = reading.edit_text(-0.3);
        assert_eq!(reading.parse(&typed), Some(-0.3), "the text box round trips");
    }

    #[test]
    fn readings_format_and_parse_their_own_units() {
        let db: Reading<'_> = Reading::Db;
        assert_eq!(db.format(-12.34), "\u{2212}12.3 dB");
        assert_eq!(db.parse("-6"), Some(-6.0));

        let percent: Reading<'_> = Reading::Number { decimals: 1, unit: Some("%") };
        assert_eq!(percent.format(45.0), "45.0 %");
        assert_eq!(percent.parse("45.0 %"), Some(45.0));
        assert_eq!(percent.parse("\u{2212}8.5"), Some(-8.5), "a typographic minus still parses");
        assert_eq!(percent.parse("%"), None);

        assert_eq!(Reading::plain().format(-12.34), "-12.34");
    }

    #[test]
    fn an_arc_is_sampled_finely_and_in_order() {
        let points = arc(Pos2::new(0.0, 0.0), 10.0, START_ANGLE, START_ANGLE + DEFAULT_SWEEP);
        assert!(points.len() > 30, "a coarse arc reads as a polygon: {}", points.len());
        assert!(points.windows(2).all(|pair| pair[0] != pair[1]), "no repeated vertices");
        assert!(
            points.iter().all(|p| ((*p - Pos2::ZERO).length() - 10.0).abs() < 1e-3),
            "every point sits on the ring"
        );
        assert_eq!(arc(Pos2::ZERO, 10.0, 0.0, 0.0).len(), 1, "a zero span is one point");
    }

    #[test]
    fn graduations_and_a_readout_grow_the_knob() {
        let mut value = 0.0;
        let theme = Theme::dark();
        let bare = knob(&mut value).scale(false).value_readout(false).desired_size(&theme);
        let dressed = knob(&mut value).label("MIX").desired_size(&theme);
        assert!(dressed.x > bare.x, "marks ring the dial");
        assert!(dressed.y > bare.y, "a label and a readout take rows");
    }

    fn paint(position: f32) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        let mut value = position;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(Knob::new(&mut value).range(0.0, 1.0).id(egui::Id::new("test-knob")));
        });
        output.shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    fn lit_points(shapes: &[egui::Shape]) -> usize {
        let primary = Theme::dark().palette.primary;
        shapes
            .iter()
            .filter_map(|shape| match shape {
                egui::Shape::Path(path)
                    if path.stroke.color == ColorMode::Solid(primary) =>
                {
                    Some(path.points.len())
                }
                _ => None,
            })
            .sum()
    }

    #[test]
    fn a_turned_knob_paints_its_value_arc() {
        assert_eq!(lit_points(&paint(0.0)), 0, "at the stop there is no arc to draw");
        assert!(lit_points(&paint(0.5)) > 10, "half round is a real arc");
    }

    #[test]
    fn a_bipolar_knob_fills_out_from_the_centre() {
        assert_eq!(lit_points(&pan_paint(0.0)), 0, "balanced has nothing to fill");
        let left = lit_points(&pan_paint(-0.6));
        let right = lit_points(&pan_paint(0.6));
        assert!(left > 10, "a turn left still draws an arc: {left} points");
        assert!(left.abs_diff(right) <= 2, "the same turn is the same arc either way");
        assert!(lit_points(&pan_paint(-1.0)) > left, "a full turn is longer than a partial one");
    }

    fn pan_paint(balance: f32) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        let mut value = balance;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(Knob::pan(&mut value).id(egui::Id::new("test-pan")));
        });
        output.shapes.into_iter().map(|clipped| clipped.shape).collect()
    }
}
