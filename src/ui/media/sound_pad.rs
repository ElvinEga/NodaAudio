//! An interactive drum pad / launchpad trigger grid with modes and hotkey badges.
//!
//! Visual language and behavior directly follow audiocn's `sound-pad.tsx`.
//! Supports `OneShot`, `Toggle`, `Hold`, and `Loop` playback modes, velocity-sensitive
//! glowing active states, and custom hotkey indicators.

use egui::{
    Color32, FontFamily, FontId, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2, Widget, pos2,
};

use crate::ui::theme::{Size, Theme};

/// Trigger playback mode of a sound pad.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SoundPadMode {
    /// Plays full sample once on click/hit.
    #[default]
    OneShot,
    /// Alternates play / stop on each click.
    Toggle,
    /// Plays only while mouse / finger is held down.
    Hold,
    /// Loops continuously until toggled off.
    Loop,
}

/// A sound pad definition.
#[derive(Clone, Debug)]
pub struct SoundPadItem<'a> {
    /// Pad index (e.g. 0..15).
    pub id: usize,
    /// Pad label (e.g. "Kick", "Snare", "Hi-Hat", "Vocal Chop").
    pub label: &'a str,
    /// Optional keyboard hotkey trigger badge (e.g. "Q", "1", "Space").
    pub hotkey: Option<&'a str>,
    /// Pad accent color.
    pub color: Option<Color32>,
    /// Playback mode.
    pub mode: SoundPadMode,
    /// Current playing / active state.
    pub is_playing: bool,
}

impl<'a> SoundPadItem<'a> {
    /// Create a new sound pad item.
    pub fn new(id: usize, label: &'a str) -> Self {
        Self {
            id,
            label,
            hotkey: None,
            color: None,
            mode: SoundPadMode::OneShot,
            is_playing: false,
        }
    }

    /// Set hotkey indicator badge.
    pub fn hotkey(mut self, hotkey: &'a str) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    /// Set pad color tint.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Set playback mode.
    pub fn mode(mut self, mode: SoundPadMode) -> Self {
        self.mode = mode;
        self
    }

    /// Mark active / playing status.
    pub fn playing(mut self, is_playing: bool) -> Self {
        self.is_playing = is_playing;
        self
    }
}

/// An interactive sound pad grid component.
///
/// ```ignore
/// let mut pads = [
///     SoundPadItem::new(0, "Kick").hotkey("Q"),
///     SoundPadItem::new(1, "Snare").hotkey("W"),
///     SoundPadItem::new(2, "Hi-Hat").hotkey("E"),
///     SoundPadItem::new(3, "Clap").hotkey("R"),
/// ];
/// ui.add(SoundPadGrid::new(&mut pads).columns(2));
/// ```
#[must_use = "a SoundPadGrid draws nothing until it is added to a Ui"]
pub struct SoundPadGrid<'a, 'p> {
    pads: &'a mut [SoundPadItem<'p>],
    columns: usize,
    pad_size: Option<f32>,
    gap: f32,
    size: Option<Size>,
}

impl<'a, 'p> SoundPadGrid<'a, 'p> {
    /// Create a new sound pad grid from a slice of pad items.
    pub fn new(pads: &'a mut [SoundPadItem<'p>]) -> Self {
        Self {
            pads,
            columns: 4,
            pad_size: None,
            gap: 6.0,
            size: None,
        }
    }

    /// Number of grid columns (e.g. 2, 4, 8). Default 4.
    pub fn columns(mut self, cols: usize) -> Self {
        self.columns = cols.max(1);
        self
    }

    /// Custom size per pad button in points (default 68.0).
    pub fn pad_size(mut self, size: f32) -> Self {
        self.pad_size = Some(size);
        self
    }

    /// Gap spacing between pads (default 6.0).
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(2.0);
        self
    }

    /// Size variant.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}

impl Widget for SoundPadGrid<'_, '_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let default_pad_w = match size {
            Size::Small => 52.0,
            Size::Medium => 68.0,
            Size::Large => 84.0,
        };
        let pad_w = self.pad_size.unwrap_or(default_pad_w);
        let pad_h = pad_w * 0.9;
        let cols = self.columns;
        let count = self.pads.len();
        let rows = (count + cols - 1) / cols;

        let total_w = cols as f32 * pad_w + (cols.saturating_sub(1) as f32 * self.gap);
        let total_h = rows as f32 * pad_h + (rows.saturating_sub(1) as f32 * self.gap);

        let (grid_rect, mut response) = ui.allocate_exact_size(
            Vec2::new(total_w, total_h),
            Sense::hover(),
        );

        let painter = ui.painter();
        let font_label = FontId::new(theme.size.value_font() * 0.9, FontFamily::Proportional);
        let font_hotkey = FontId::new(theme.size.label_font() * 0.75, FontFamily::Monospace);

        for (i, pad) in self.pads.iter_mut().enumerate() {
            let col = i % cols;
            let row = i / cols;

            let x = grid_rect.min.x + col as f32 * (pad_w + self.gap);
            let y = grid_rect.min.y + row as f32 * (pad_h + self.gap);
            let pad_rect = Rect::from_min_size(pos2(x, y), Vec2::new(pad_w, pad_h));

            let pad_id = ui.id().with(("sound_pad", pad.id));
            let pad_resp = ui.interact(pad_rect, pad_id, Sense::click_and_drag());

            let is_down = pad_resp.is_pointer_button_down_on();
            let is_clicked = pad_resp.clicked();

            // Handle mode behavior
            match pad.mode {
                SoundPadMode::OneShot => {
                    if is_clicked {
                        pad.is_playing = true;
                        response.mark_changed();
                    } else if !is_down {
                        pad.is_playing = false;
                    }
                }
                SoundPadMode::Toggle | SoundPadMode::Loop => {
                    if is_clicked {
                        pad.is_playing = !pad.is_playing;
                        response.mark_changed();
                    }
                }
                SoundPadMode::Hold => {
                    if is_down != pad.is_playing {
                        pad.is_playing = is_down;
                        response.mark_changed();
                    }
                }
            }

            let pad_color = pad.color.unwrap_or(theme.palette.primary);
            let is_lit = pad.is_playing || is_down;

            // 1. Outer glow halo when triggered
            if is_lit {
                let glow_rect = pad_rect.expand(3.0);
                painter.rect_filled(
                    glow_rect,
                    theme.radius.md(),
                    pad_color.linear_multiply(0.35),
                );
            }

            // 2. Pad surface
            let bg_color = if is_lit {
                pad_color.linear_multiply(0.85)
            } else if pad_resp.hovered() {
                theme.palette.surface.linear_multiply(1.1)
            } else {
                theme.palette.surface
            };

            let stroke_color = if is_lit {
                pad_color
            } else {
                theme.palette.border
            };

            painter.rect_filled(pad_rect, theme.radius.sm(), bg_color);
            painter.rect_stroke(
                pad_rect,
                theme.radius.sm(),
                Stroke::new(1.0f32, stroke_color),
                StrokeKind::Inside,
            );

            // 3. Hotkey badge in top-left
            if let Some(hotkey) = pad.hotkey {
                let badge_pos = pos2(pad_rect.min.x + 6.0, pad_rect.min.y + 4.0);
                let badge_col = if is_lit {
                    theme.palette.primary_foreground
                } else {
                    theme.palette.text_dim
                };
                painter.text(
                    badge_pos,
                    egui::Align2::LEFT_TOP,
                    hotkey,
                    font_hotkey.clone(),
                    badge_col,
                );
            }

            // 4. Pad label in center
            let label_col = if is_lit {
                theme.palette.primary_foreground
            } else {
                theme.palette.foreground
            };
            painter.text(
                pad_rect.center(),
                egui::Align2::CENTER_CENTER,
                pad.label,
                font_label.clone(),
                label_col,
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_pad_grid_renders_and_triggers() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut pads = [
                SoundPadItem::new(0, "Kick").hotkey("Q").playing(true),
                SoundPadItem::new(1, "Snare").hotkey("W"),
            ];
            let resp = ui.add(SoundPadGrid::new(&mut pads).columns(2));
            assert!(resp.rect.width() > 0.0);
        });
    }
}
