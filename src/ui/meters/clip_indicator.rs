//! The clip light, and the latch that decides when it is on.

use egui::{Align2, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};

use crate::ui::theme::Theme;
use crate::zones::{CLIP_HOLD_MS, CLIP_THRESHOLD_DB};

/// Whether a level counts as a clip. The threshold is inclusive: a peak
/// that reaches it is a peak that hit full scale.
#[must_use]
pub fn is_clipping(db: f32, threshold_db: f32) -> bool {
    !db.is_nan() && db >= threshold_db
}

/// When a clip light fires and how long it stays lit.
#[derive(Clone, Copy, Debug)]
pub struct ClipPolicy {
    /// Levels at or above this count as a clip.
    pub threshold_db: f32,
    /// How long the light stays on after the last clip. `f32::INFINITY`
    /// latches it until [`ClipLatch::reset`].
    pub hold_ms: f32,
}

impl Default for ClipPolicy {
    fn default() -> Self {
        Self { threshold_db: CLIP_THRESHOLD_DB, hold_ms: CLIP_HOLD_MS }
    }
}

impl ClipPolicy {
    /// A light that stays on until someone resets it.
    #[must_use]
    pub fn latching() -> Self {
        Self { hold_ms: f32::INFINITY, ..Self::default() }
    }
}

/// A clip light that keeps its own memory.
///
/// A one-sample hit at full scale is a real event but an invisible blink,
/// so the latch holds the light on and counts how many times it fired. That
/// lets a plugin report "you clipped three times" rather than only "you are
/// clipping right now".
#[derive(Clone, Copy, Debug, Default)]
pub struct ClipLatch {
    on_until_ms: f32,
    active: bool,
    count: u32,
}

impl ClipLatch {
    /// Feed a reading and get back whether the light should be on now.
    #[must_use]
    pub fn report(&mut self, db: f32, now_ms: f32, policy: ClipPolicy) -> bool {
        if is_clipping(db, policy.threshold_db) {
            if !self.active {
                self.count += 1;
            }
            self.active = true;
            // Adding an infinite hold gives an infinite deadline, which no
            // clock reading can pass - that is the latch.
            self.on_until_ms = now_ms + policy.hold_ms;
        } else if now_ms >= self.on_until_ms {
            self.active = false;
        }
        self.active
    }

    /// Clip events since the last reset.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Put the light out and zero the count.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// True while the light is on.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// A clip light.
///
/// Presentational: it is told whether to glow. Drive it from a
/// [`ClipLatch`] to get the hold and the count, or from a boolean if the
/// plugin's DSP already detects clipping.
#[must_use = "a ClipIndicator draws nothing until it is added to a Ui"]
pub struct ClipIndicator {
    active: bool,
    count: Option<u32>,
    dot_size: f32,
}

impl Default for ClipIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipIndicator {
    /// An unlit dot.
    pub fn new() -> Self {
        Self { active: false, count: None, dot_size: 8.0 }
    }

    /// Show the light in this state.
    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Show a latch's state and event count.
    #[must_use]
    pub fn latch(mut self, latch: &ClipLatch) -> Self {
        self.active = latch.is_active();
        self.count = Some(latch.count());
        self
    }

    /// Show the number of clip events next to the dot.
    #[must_use]
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// Dot diameter, in points.
    #[must_use]
    pub fn dot_size(mut self, size: f32) -> Self {
        self.dot_size = size;
        self
    }

    /// Space the indicator needs, in points.
    #[must_use]
    pub fn desired_size(&self, theme: &Theme) -> Vec2 {
        let count_width = if self.count.is_some() { 22.0 } else { 0.0 };
        Vec2::new(self.dot_size + count_width, (self.dot_size + 6.0).max(theme.size.value_font()))
    }

    /// Add the indicator to the current layout.
    ///
    /// The response reports clicks: resetting is the caller's job, which is
    /// how a console's clip button behaves and keeps the widget stateless.
    pub fn ui(self, ui: &mut egui::Ui) -> Response {
        let theme = Theme::of(ui);
        let size = self.desired_size(&theme);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        if ui.is_rect_visible(rect) {
            self.paint_at(ui, &theme, rect);
        }
        response.on_hover_text(if self.active {
            "Clipping"
        } else {
            "No clips since the last reset"
        })
    }

    /// Paint into a rect somebody else reserved.
    pub fn paint_at(&self, ui: &egui::Ui, theme: &Theme, rect: Rect) {
        let painter = ui.painter_at(rect);
        let palette = &theme.palette;
        let center = Pos2::new(rect.left() + self.dot_size / 2.0, rect.center().y);
        let radius = self.dot_size / 2.0;

        if self.active {
            painter.circle(center, radius, palette.meter_clip, Stroke::NONE);
        } else {
            // A dark dot with a ring still reads as a lamp, rather than as
            // a smudge on the panel.
            painter.circle(center, radius, palette.track, Stroke::NONE);
            painter.circle_stroke(center, radius, Stroke::new(1.0_f32, palette.border));
        }

        if let Some(count) = self.count {
            painter.text(
                Pos2::new(center.x + radius + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                count.to_string(),
                FontId::monospace(theme.size.value_font()),
                if self.active { palette.meter_clip } else { palette.text_dim },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SILENCE_DB;

    #[test]
    fn the_threshold_is_inclusive() {
        assert!(is_clipping(-1.0, -1.0));
        assert!(!is_clipping(-1.01, -1.0));
        assert!(!is_clipping(SILENCE_DB, -1.0));
        assert!(!is_clipping(f32::NAN, -1.0));
    }

    #[test]
    fn the_light_holds_then_falls() {
        let policy = ClipPolicy::default();
        let mut latch = ClipLatch::default();
        assert!(latch.report(0.0, 0.0, policy));
        assert_eq!(latch.count(), 1);
        // Inside the hold window the light stays, but it is one event.
        assert!(latch.report(SILENCE_DB, 1000.0, policy));
        assert_eq!(latch.count(), 1);
        assert!(!latch.report(SILENCE_DB, 2000.0, policy));
    }

    #[test]
    fn a_latched_light_waits_for_a_reset() {
        let mut latch = ClipLatch::default();
        assert!(latch.report(0.0, 0.0, ClipPolicy::latching()));
        assert!(latch.report(SILENCE_DB, 100_000.0, ClipPolicy::latching()));
        latch.reset();
        assert!(!latch.report(SILENCE_DB, 100_000.0, ClipPolicy::latching()));
        assert_eq!(latch.count(), 0);
    }

    #[test]
    fn separate_bursts_count_separately() {
        let policy = ClipPolicy { hold_ms: 100.0, ..ClipPolicy::default() };
        let mut latch = ClipLatch::default();
        let _ = latch.report(0.0, 0.0, policy);
        let _ = latch.report(SILENCE_DB, 500.0, policy);
        let _ = latch.report(0.0, 900.0, policy);
        assert_eq!(latch.count(), 2);
    }
}
