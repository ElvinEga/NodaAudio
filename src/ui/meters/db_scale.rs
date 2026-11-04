//! A dB graduation strip: marks, numbers, or both.

use egui::{Align2, FontId, Pos2, Rect, Stroke};

use crate::ui::Orientation;
use crate::ui::theme::Theme;
use crate::db::{DEFAULT_MAX_DB, DEFAULT_MIN_DB, FormatOptions, format_db};
use crate::taper::Taper;

/// Tick values drawn by default, spanning a console meter's useful range.
pub const DEFAULT_TICKS: [f32; 12] =
    [12.0, 6.0, 0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0, -72.0, -90.0];

/// How far the outermost tick is pulled from the end of the strip, as a
/// fraction of its length, so an edge label never gets clipped.
const EDGE: f32 = 0.02;

/// Space kept between two labels, in points.
const LABEL_GAP: f32 = 3.0;

/// A label at this multiple of dB outranks one that is not.
const MAJOR_STEP: f32 = 12.0;

/// Gap between a mark and its label, in points.
const MARK_TO_LABEL: f32 = 2.0;

/// Which edge of the scale strip the marks hug.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkEdge {
    /// Left for a vertical scale, top for a horizontal one.
    Start,
    /// Right for a vertical scale, bottom for a horizontal one.
    End,
}

/// A label that survived thinning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    /// The value it names.
    pub db: f32,
    /// Where it sits along the travel, 0..1.
    pub position: f32,
    /// Print priority: `0` is unity gain, `1` the ends of the range, `2`
    /// the 12 dB multiples, `3` everything else.
    pub rank: u8,
}

/// A dB scale drawn into a rect somebody else reserved.
///
/// A scale never lays itself out: a meter or a fader owns the strip and
/// hands it over, which is what keeps the graduations true against the bar
/// they measure.
#[must_use = "a DbScale draws nothing until it is painted"]
pub struct DbScale<'a> {
    orientation: Orientation,
    min_db: f32,
    max_db: f32,
    taper: Taper,
    ticks: &'a [f32],
    mark_edge: MarkEdge,
    show_labels: bool,
    major_length: f32,
    minor_length: f32,
}

impl DbScale<'_> {
    /// A vertical scale over the meter default of -60 to 0 dBFS.
    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_MAX_DB,
            taper: Taper::Linear,
            ticks: &DEFAULT_TICKS,
            mark_edge: MarkEdge::End,
            show_labels: true,
            major_length: 6.0,
            minor_length: 4.0,
        }
    }

    /// A horizontal scale.
    pub fn horizontal() -> Self {
        Self { orientation: Orientation::Horizontal, ..Self::vertical() }
    }
}

impl<'a> DbScale<'a> {
    /// The dB range the strip spans.
    #[must_use]
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// How the range maps to the strip's length. A fader hands over its own
    /// taper so the graduations line up with the handle.
    #[must_use]
    pub fn taper(mut self, taper: Taper) -> Self {
        self.taper = taper;
        self
    }

    /// Which values get a mark. Values outside the range are dropped.
    #[must_use]
    pub fn ticks(mut self, ticks: &'a [f32]) -> Self {
        self.ticks = ticks;
        self
    }

    /// Which edge carries the marks.
    #[must_use]
    pub fn mark_edge(mut self, edge: MarkEdge) -> Self {
        self.mark_edge = edge;
        self
    }

    /// Draw numbers, or marks only.
    #[must_use]
    pub fn labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Length of a labelled and an unlabelled mark, in points.
    #[must_use]
    pub fn mark_lengths(mut self, major: f32, minor: f32) -> Self {
        self.major_length = major;
        self.minor_length = minor;
        self
    }

    /// Width (vertical) or height (horizontal) the strip wants, so a caller
    /// can reserve it.
    #[must_use]
    pub fn desired_thickness(&self, theme: &Theme) -> f32 {
        match self.orientation {
            Orientation::Vertical => {
                if self.show_labels {
                    28.0_f32.max(self.major_length + MARK_TO_LABEL + theme.size.label_font() * 3.0)
                } else {
                    self.major_length + 1.0
                }
            }
            Orientation::Horizontal => {
                if self.show_labels {
                    self.major_length + MARK_TO_LABEL + theme.size.label_font() + 2.0
                } else {
                    self.major_length + 1.0
                }
            }
        }
    }

    /// Every tick inside the range, with its position and print rank.
    #[must_use]
    pub fn ticks_in_range(&self) -> Vec<Tick> {
        let values: Vec<f32> =
            self.ticks.iter().copied().filter(|db| *db >= self.min_db && *db <= self.max_db).collect();
        if values.is_empty() {
            return Vec::new();
        }
        let top = values.iter().copied().fold(f32::MIN, f32::max);
        let bottom = values.iter().copied().fold(f32::MAX, f32::min);

        values
            .into_iter()
            .map(|db| Tick {
                db,
                position: self.taper.to_position(db, self.min_db, self.max_db),
                rank: rank_for(db, top, bottom),
            })
            .collect()
    }

    /// The labels that fit into `strip_length` points, weakest dropped first.
    #[must_use]
    pub fn kept_labels(&self, strip_length: f32) -> Vec<Tick> {
        if !self.show_labels {
            return Vec::new();
        }
        thin_labels(self.ticks_in_range(), strip_length, self.label_footprint(), LABEL_GAP)
    }

    /// How much room one label claims along the strip, in points.
    #[must_use]
    pub fn label_footprint(&self) -> f32 {
        // Measured before painting, so there is no galley to ask: a dB
        // number is at most four glyphs wide, and one line tall.
        match self.orientation {
            Orientation::Vertical => 12.0,
            Orientation::Horizontal => 26.0,
        }
    }

    /// Paint the scale into `rect`.
    ///
    /// Labels that would collide drop out in rank order - unity first, then
    /// the ends of the range, then the 12 dB multiples - so a short strip
    /// stays readable instead of turning to grey soup.
    pub fn paint(self, ui: &egui::Ui, rect: Rect) {
        if !ui.is_rect_visible(rect) || rect.area() <= 0.0 {
            return;
        }
        let theme = Theme::of(ui);
        let painter = ui.painter_at(rect);
        let font = FontId::proportional(theme.size.label_font());
        let vertical = self.orientation == Orientation::Vertical;
        let ticks = self.ticks_in_range();
        let kept =
            thin_labels(ticks.clone(), self.orientation.length(rect), self.label_footprint(), LABEL_GAP);

        let (mark_from, mark_dir, label_at, label_anchor) = self.geometry(rect, vertical);
        // Labels are centre-anchored, so the outermost one needs half a text
        // box of margin at each end or the strip's clip rect cuts it off.
        let half_label = if vertical {
            theme.size.label_font() * 0.7
        } else {
            theme.size.label_font() * 1.1
        };
        let label_inset = (half_label / self.orientation.length(rect)).clamp(0.0, 0.45);

        for tick in &ticks {
            let position = tick.position.clamp(EDGE, 1.0 - EDGE);
            let along = self.orientation.along(rect, position);
            let labelled = kept.iter().any(|candidate| candidate.db == tick.db);
            let length =
                if labelled || !self.show_labels { self.major_length } else { self.minor_length };
            let color = if tick.db == 0.0 { theme.palette.foreground } else { theme.palette.text_dim };
            let stroke = Stroke::new(1.0_f32, color);
            let end = mark_from + length * mark_dir;

            if vertical {
                painter.line_segment([Pos2::new(mark_from, along), Pos2::new(end, along)], stroke);
            } else {
                painter.line_segment([Pos2::new(along, mark_from), Pos2::new(along, end)], stroke);
            }
        }

        for label in &kept {
            let position = label.position.clamp(label_inset, 1.0 - label_inset);
            let along = self.orientation.along(rect, position);
            let text = format_db(
                label.db,
                FormatOptions { decimals: 0, unit: false, ..FormatOptions::default() },
            );
            let color = if label.db == 0.0 { theme.palette.foreground } else { theme.palette.text_dim };
            let at =
                if vertical { Pos2::new(label_at, along) } else { Pos2::new(along, label_at) };
            painter.text(at, label_anchor, text, font.clone(), color);
        }
    }

    /// Where marks start, which way they point, and where a label sits.
    fn geometry(&self, rect: Rect, vertical: bool) -> (f32, f32, f32, Align2) {
        match (vertical, self.mark_edge) {
            (true, MarkEdge::Start) => (
                rect.left(),
                1.0,
                rect.left() + self.major_length + MARK_TO_LABEL,
                Align2::LEFT_CENTER,
            ),
            (true, MarkEdge::End) => (
                rect.right(),
                -1.0,
                rect.right() - self.major_length - MARK_TO_LABEL,
                Align2::RIGHT_CENTER,
            ),
            (false, MarkEdge::Start) => (
                rect.top(),
                1.0,
                rect.top() + self.major_length + MARK_TO_LABEL,
                Align2::CENTER_TOP,
            ),
            (false, MarkEdge::End) => (
                rect.bottom(),
                -1.0,
                rect.bottom() - self.major_length - MARK_TO_LABEL,
                Align2::CENTER_BOTTOM,
            ),
        }
    }
}

/// The print rank of one tick value.
#[must_use]
fn rank_for(db: f32, top: f32, bottom: f32) -> u8 {
    if db == 0.0 {
        0
    } else if db == top || db == bottom {
        1
    } else if (db / MAJOR_STEP).fract() == 0.0 {
        2
    } else {
        3
    }
}

/// Keep as many labels as fit, strongest first.
///
/// Ties go to the louder value, because a scale reads top-down.
#[must_use]
fn thin_labels(ticks: Vec<Tick>, strip_length: f32, footprint: f32, gap: f32) -> Vec<Tick> {
    if strip_length <= 0.0 {
        return Vec::new();
    }
    let mut ordered = ticks;
    ordered.sort_by(|a, b| a.rank.cmp(&b.rank).then(b.db.total_cmp(&a.db)));

    let min_spacing = (footprint + gap) / strip_length;
    let mut kept: Vec<Tick> = Vec::with_capacity(ordered.len());
    for tick in ordered {
        let collides =
            kept.iter().any(|other: &Tick| (tick.position - other.position).abs() < min_spacing);
        if !collides {
            kept.push(tick);
        }
    }
    kept.sort_by(|a, b| b.db.total_cmp(&a.db));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(min: f32, max: f32) -> DbScale<'static> {
        DbScale::vertical().range(min, max)
    }

    #[test]
    fn ticks_outside_the_range_are_dropped() {
        let ticks = scale(-60.0, 0.0).ticks_in_range();
        assert!(ticks.iter().all(|tick| tick.db >= -60.0 && tick.db <= 0.0));
        assert!(ticks.iter().any(|tick| tick.db == 0.0));
        assert!(!ticks.iter().any(|tick| tick.db == 6.0));
        assert!(!ticks.iter().any(|tick| tick.db == -72.0));
    }

    #[test]
    fn ranks_prefer_unity_then_ends_then_decades() {
        let ticks = scale(-60.0, 0.0).ticks_in_range();
        let rank = |db: f32| ticks.iter().find(|tick| tick.db == db).map(|tick| tick.rank);
        assert_eq!(rank(0.0), Some(0));
        assert_eq!(rank(-60.0), Some(1));
        assert_eq!(rank(-12.0), Some(2));
        assert_eq!(rank(-48.0), Some(2));
        assert_eq!(rank(-18.0), Some(3));
        assert_eq!(rank(-6.0), Some(3));
    }

    #[test]
    fn a_short_strip_drops_the_weakest_labels_first() {
        let ticks = scale(-60.0, 0.0).ticks_in_range();
        let kept = thin_labels(ticks.clone(), 90.0, 12.0, LABEL_GAP);
        let values: Vec<f32> = kept.iter().map(|tick| tick.db).collect();
        assert!(values.contains(&0.0), "unity gain always survives: {values:?}");
        assert!(values.contains(&-60.0), "the bottom of the range survives: {values:?}");
        assert!(!values.contains(&-6.0), "a mid-scale minor label goes: {values:?}");
        assert!(kept.windows(2).all(|pair| pair[0].db > pair[1].db), "kept labels sort loud to quiet");
    }

    #[test]
    fn a_long_strip_keeps_everything_it_can_show() {
        let ticks = scale(-60.0, 0.0).ticks_in_range();
        let kept = thin_labels(ticks.clone(), 600.0, 12.0, LABEL_GAP);
        assert_eq!(kept.len(), ticks.len());
    }

    #[test]
    fn labels_off_keeps_nothing() {
        assert!(scale(-60.0, 0.0).labels(false).kept_labels(600.0).is_empty());
    }

    #[test]
    fn positions_follow_the_taper() {
        let audio = scale(-60.0, 6.0).taper(Taper::AUDIO).ticks_in_range();
        let unity = audio.iter().find(|tick| tick.db == 0.0).expect("0 dB tick");
        assert!((unity.position - 0.75).abs() < 1e-5);
        let linear = scale(-60.0, 0.0).ticks_in_range();
        let low = linear.iter().find(|tick| tick.db == -36.0).expect("-36 tick");
        assert!((low.position - 0.4).abs() < 1e-5, "linear in dB: -36 of -60..0");
    }
}
