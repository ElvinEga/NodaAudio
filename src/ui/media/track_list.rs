//! A multi-track playlist and queue list widget with selection and play states.
//!
//! Visual language and behavior directly follow audiocn's `track-list.tsx`.
//! Provides accessible track rows with title, artist, duration, active selection,
//! and transport play/pause triggers.

use egui::{
    Color32, FontFamily, FontId, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2, Widget, pos2,
};

use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::waveform::format_time;

/// A track item entry in a playlist or queue.
#[derive(Clone, Debug)]
pub struct TrackItem<'a> {
    /// Track index / position (1-based).
    pub index: usize,
    /// Title of the song or clip.
    pub title: &'a str,
    /// Optional artist or genre name.
    pub artist: Option<&'a str>,
    /// Track duration in seconds.
    pub duration_secs: f32,
    /// Whether this track is currently active / selected.
    pub is_active: bool,
    /// Whether this track is currently playing.
    pub is_playing: bool,
}

impl<'a> TrackItem<'a> {
    /// Create a new track item.
    pub fn new(index: usize, title: &'a str, duration_secs: f32) -> Self {
        Self {
            index,
            title,
            artist: None,
            duration_secs,
            is_active: false,
            is_playing: false,
        }
    }

    /// Set artist name.
    pub fn artist(mut self, artist: &'a str) -> Self {
        self.artist = Some(artist);
        self
    }

    /// Mark active status.
    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    /// Mark playing status.
    pub fn playing(mut self, is_playing: bool) -> Self {
        self.is_playing = is_playing;
        self
    }
}

/// An interactive track list / playlist widget.
///
/// ```ignore
/// let tracks = [
///     TrackItem::new(1, "Summer Breeze", 195.0).active(true).playing(true),
///     TrackItem::new(2, "Night Drive", 240.0),
/// ];
/// let mut selected = Some(1);
/// ui.add(TrackList::new(&tracks, &mut selected));
/// ```
#[must_use = "a TrackList draws nothing until it is added to a Ui"]
pub struct TrackList<'a> {
    tracks: &'a [TrackItem<'a>],
    selected_index: &'a mut Option<usize>,
    width: Option<f32>,
    row_height: Option<f32>,
    size: Option<Size>,
}

impl<'a> TrackList<'a> {
    /// Create a new track list widget from a slice of track items and mutable selection index.
    pub fn new(tracks: &'a [TrackItem<'a>], selected_index: &'a mut Option<usize>) -> Self {
        Self {
            tracks,
            selected_index,
            width: None,
            row_height: None,
            size: None,
        }
    }

    /// Explicit widget width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Custom row height in points (default 40.0).
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Size variant.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}

impl Widget for TrackList<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let default_row_h = match size {
            Size::Small => 32.0,
            Size::Medium => 40.0,
            Size::Large => 48.0,
        };
        let width = self.width.unwrap_or_else(|| ui.available_width());
        let row_h = self.row_height.unwrap_or(default_row_h);
        let total_h = self.tracks.len() as f32 * (row_h + 2.0);

        let (total_rect, mut response) = ui.allocate_exact_size(
            Vec2::new(width, total_h),
            Sense::hover(),
        );

        let painter = ui.painter();
        let font_title = FontId::new(theme.size.value_font(), FontFamily::Proportional);
        let font_sub = FontId::new(theme.size.label_font(), FontFamily::Proportional);
        let font_time = FontId::new(theme.size.label_font(), FontFamily::Monospace);

        let mut next_selected = *self.selected_index;

        for (i, track) in self.tracks.iter().enumerate() {
            let row_y = total_rect.min.y + i as f32 * (row_h + 2.0);
            let row_rect = Rect::from_min_size(pos2(total_rect.min.x, row_y), Vec2::new(width, row_h));

            let row_id = ui.id().with(("track_row", track.index));
            let row_resp = ui.interact(row_rect, row_id, Sense::click());

            let is_selected = self.selected_index.map_or(track.is_active, |idx| idx == track.index);

            // Row background hover / selection fill
            let bg_color = if is_selected {
                theme.palette.primary.linear_multiply(0.18)
            } else if row_resp.hovered() {
                theme.palette.track.linear_multiply(0.6)
            } else {
                Color32::TRANSPARENT
            };

            painter.rect_filled(row_rect, theme.radius.sm(), bg_color);
            if is_selected {
                painter.rect_stroke(
                    row_rect,
                    theme.radius.sm(),
                    Stroke::new(1.0f32, theme.palette.primary.linear_multiply(0.4)),
                    StrokeKind::Inside,
                );
            }

            // Left icon / track number
            let icon_x = row_rect.min.x + 12.0;
            let icon_y = row_rect.center().y;

            if track.is_playing {
                painter.text(
                    pos2(icon_x, icon_y),
                    egui::Align2::LEFT_CENTER,
                    "▶",
                    font_title.clone(),
                    theme.palette.primary,
                );
            } else {
                painter.text(
                    pos2(icon_x, icon_y),
                    egui::Align2::LEFT_CENTER,
                    format!("{:02}", track.index),
                    font_time.clone(),
                    theme.palette.text_dim,
                );
            }

            // Title and artist
            let text_x = row_rect.min.x + 38.0;
            let title_color = if is_selected {
                theme.palette.foreground
            } else {
                theme.palette.foreground.linear_multiply(0.9)
            };

            if let Some(artist) = track.artist {
                painter.text(
                    pos2(text_x, icon_y - 6.0),
                    egui::Align2::LEFT_CENTER,
                    track.title,
                    font_title.clone(),
                    title_color,
                );
                painter.text(
                    pos2(text_x, icon_y + 8.0),
                    egui::Align2::LEFT_CENTER,
                    artist,
                    font_sub.clone(),
                    theme.palette.text_dim,
                );
            } else {
                painter.text(
                    pos2(text_x, icon_y),
                    egui::Align2::LEFT_CENTER,
                    track.title,
                    font_title.clone(),
                    title_color,
                );
            }

            // Right duration readout
            let time_x = row_rect.right() - 12.0;
            let dur_text = format_time(track.duration_secs);
            painter.text(
                pos2(time_x, icon_y),
                egui::Align2::RIGHT_CENTER,
                dur_text,
                font_time.clone(),
                theme.palette.text_dim,
            );

            if row_resp.clicked() {
                next_selected = Some(track.index);
                response.mark_changed();
            }
        }

        *self.selected_index = next_selected;

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_list_renders_and_selects() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let tracks = [
                TrackItem::new(1, "Track A", 180.0).artist("Artist A").active(true).playing(true),
                TrackItem::new(2, "Track B", 210.0).artist("Artist B"),
            ];
            let mut selected = Some(1);
            let resp = ui.add(TrackList::new(&tracks, &mut selected));
            assert!(resp.rect.width() > 0.0);
        });
    }
}
