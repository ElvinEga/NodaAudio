//! A complete audio player transport widget with scrub timeline, volume, and playback rates.
//!
//! Visual language and behavior directly follow audiocn's `audio-player.tsx`.
//! Composes play/pause transport controls, timestamp readouts, interactive seek bar,
//! speed multipliers (0.5x, 1x, 1.25x, 1.5x, 2x), loop toggle, and media volume slider.

use egui::{
    Button, FontFamily, FontId, Rect, Response, RichText, Sense, Stroke,
    Ui, Vec2, Widget, pos2,
};

use crate::ui::controls::volume_control::VolumeControl;
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::waveform::format_time;

/// Common playback speed presets.
pub const PLAYBACK_RATES: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// An interactive audio player transport component.
///
/// ```ignore
/// let mut playing = false;
/// let mut time = 14.5f32;
/// let mut volume = 0.8f32;
/// let mut muted = false;
/// ui.add(
///     AudioPlayer::new(&mut playing, &mut time, 180.0)
///         .volume(&mut volume, &mut muted)
///         .title("Midnight Reverie"),
/// );
/// ```
#[must_use = "an AudioPlayer draws nothing until it is added to a Ui"]
pub struct AudioPlayer<'a> {
    playing: &'a mut bool,
    current_time: &'a mut f32,
    duration: f32,
    volume: Option<&'a mut f32>,
    muted: Option<&'a mut bool>,
    rate: Option<&'a mut f32>,
    looping: Option<&'a mut bool>,
    title: Option<&'a str>,
    artist: Option<&'a str>,
    width: Option<f32>,
    size: Option<Size>,
}

impl<'a> AudioPlayer<'a> {
    /// Create a new audio player transport widget with play state, current time, and total duration.
    pub fn new(playing: &'a mut bool, current_time: &'a mut f32, duration: f32) -> Self {
        Self {
            playing,
            current_time,
            duration: duration.max(0.1),
            volume: None,
            muted: None,
            rate: None,
            looping: None,
            title: None,
            artist: None,
            width: None,
            size: None,
        }
    }

    /// Track title.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Artist / composer name.
    pub fn artist(mut self, artist: &'a str) -> Self {
        self.artist = Some(artist);
        self
    }

    /// Attach volume gain (0.0..=1.0) and mute states.
    pub fn volume(mut self, volume: &'a mut f32, muted: &'a mut bool) -> Self {
        self.volume = Some(volume);
        self.muted = Some(muted);
        self
    }

    /// Attach playback rate multiplier (e.g. 1.0).
    pub fn rate(mut self, rate: &'a mut f32) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Attach loop state toggle.
    pub fn looping(mut self, looping: &'a mut bool) -> Self {
        self.looping = Some(looping);
        self
    }

    /// Explicit player card width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set size variant.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}

impl Widget for AudioPlayer<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let default_w = match size {
            Size::Small => 280.0,
            Size::Medium => 360.0,
            Size::Large => 460.0,
        };
        let card_w = self.width.unwrap_or(default_w);

        let rounding = theme.radius.md();
        let bg_color = theme.palette.surface;
        let border_color = theme.palette.border;

        let frame = egui::Frame::group(ui.style())
            .fill(bg_color)
            .stroke(Stroke::new(1.0f32, border_color))
            .corner_radius(rounding)
            .inner_margin(12.0);

        let mut changed = false;

        let frame_response = frame.show(ui, |ui| {
            ui.set_width(card_w);

            // 1. Header: Track title and artist
            if self.title.is_some() || self.artist.is_some() {
                ui.horizontal(|ui| {
                    if let Some(title) = self.title {
                        ui.label(RichText::new(title).strong().size(theme.size.value_font()));
                    }
                    if let Some(artist) = self.artist {
                        ui.label(
                            RichText::new(format!("• {artist}"))
                                .color(theme.palette.text_dim)
                                .size(theme.size.label_font()),
                        );
                    }
                });
                ui.add_space(4.0);
            }

            // 2. Timeline seek bar
            let mut progress = (*self.current_time / self.duration).clamp(0.0, 1.0);
            let time_str = format_time(*self.current_time);
            let dur_str = format_time(self.duration);

            let bar_h = 6.0;
            let (bar_rect, bar_resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), bar_h + 8.0),
                Sense::click_and_drag(),
            );

            if bar_resp.dragged() || bar_resp.clicked() {
                if let Some(pos) = bar_resp.interact_pointer_pos() {
                    let norm = ((pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
                    *self.current_time = norm * self.duration;
                    progress = norm;
                    changed = true;
                }
            }

            let painter = ui.painter();
            let track_rect = Rect::from_min_size(
                pos2(bar_rect.min.x, bar_rect.center().y - bar_h * 0.5),
                Vec2::new(bar_rect.width(), bar_h),
            );
            painter.rect_filled(track_rect, bar_h * 0.5, theme.palette.track);

            // Played fill
            let fill_w = bar_rect.width() * progress;
            let fill_rect = Rect::from_min_size(
                track_rect.min,
                Vec2::new(fill_w, bar_h),
            );
            painter.rect_filled(fill_rect, bar_h * 0.5, theme.palette.primary);

            // Playhead thumb
            let thumb_x = bar_rect.min.x + fill_w;
            painter.circle_filled(
                pos2(thumb_x, track_rect.center().y),
                bar_h * 0.8,
                theme.palette.foreground,
            );

            // Time stamps below seek bar
            ui.horizontal(|ui| {
                let font_id = FontId::new(theme.size.label_font() * 0.9, FontFamily::Monospace);
                ui.label(RichText::new(time_str).font(font_id.clone()).color(theme.palette.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(dur_str).font(font_id).color(theme.palette.text_dim));
                });
            });

            ui.add_space(4.0);

            // 3. Transport controls row
            ui.horizontal(|ui| {
                // Play / Pause button
                let play_icon = if *self.playing { "⏸" } else { "▶" };
                let play_btn = Button::new(RichText::new(play_icon).size(theme.size.value_font() + 2.0));
                if ui.add(play_btn).clicked() {
                    *self.playing = !*self.playing;
                    changed = true;
                }

                // Loop toggle
                if let Some(looping) = self.looping {
                    let loop_col = if *looping {
                        theme.palette.primary
                    } else {
                        theme.palette.text_dim
                    };
                    if ui.selectable_label(*looping, RichText::new("🔁").color(loop_col)).clicked() {
                        *looping = !*looping;
                        changed = true;
                    }
                }

                // Speed rate cycle
                if let Some(rate) = self.rate {
                    let current_rate = *rate;
                    let rate_label = format!("{:.2}x", current_rate);
                    if ui.button(rate_label).clicked() {
                        // Cycle to next rate preset
                        let next_rate = match PLAYBACK_RATES.iter().position(|&r| (r - current_rate).abs() < 1e-3) {
                            Some(idx) => PLAYBACK_RATES[(idx + 1) % PLAYBACK_RATES.len()],
                            None => 1.0,
                        };
                        *rate = next_rate;
                        changed = true;
                    }
                }

                // Volume slider
                if let (Some(vol), Some(mute)) = (self.volume, self.muted) {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(VolumeControl::new(vol).mute(mute).width(90.0));
                    });
                }
            });
        });

        let mut resp = frame_response.response;
        if changed {
            resp.mark_changed();
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_player_renders() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut playing = false;
            let mut time = 30.0f32;
            let mut vol = 0.8f32;
            let mut mute = false;
            let mut rate = 1.0f32;
            let mut looping = true;

            let resp = ui.add(
                AudioPlayer::new(&mut playing, &mut time, 120.0)
                    .title("Chill Beat")
                    .artist("Antigravity")
                    .volume(&mut vol, &mut mute)
                    .rate(&mut rate)
                    .looping(&mut looping),
            );
            assert!(resp.rect.width() > 0.0);
        });
    }
}
