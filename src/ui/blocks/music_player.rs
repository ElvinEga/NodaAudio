//! A high-level music player and queue management block.
//!
//! Directly inspired by audiocn's `music-player.tsx`. Combines interactive waveform
//! scrubbing with cue markers, full transport controls (`AudioPlayer`), queue playlist
//! management (`TrackList`), and an optional real-time spectrum frequency drawer.

use egui::{
    Align, Align2, FontFamily, FontId, Frame, Layout, Response, RichText, Sense, Stroke,
    Ui, Vec2, Widget,
};

use crate::ui::{
    AudioPlayer, Spectrum, Theme, TrackItem, TrackList, Waveform, WaveformMarker,
    WaveformRegion,
};

/// High-level music player station block.
pub struct MusicPlayerBlock<'a> {
    tracks: &'a [TrackItem<'a>],
    current_track: &'a mut Option<usize>,
    is_playing: &'a mut bool,
    current_time: &'a mut f32,
    volume: &'a mut f32,
    muted: &'a mut bool,
    playback_rate: &'a mut f32,
    is_looping: &'a mut bool,
    waveform_peaks: &'a [f32],
    markers: &'a [WaveformMarker<'a>],
    regions: &'a [WaveformRegion<'a>],
    spectrum_bands: Option<&'a [f32]>,
    show_spectrum: &'a mut bool,
    width: Option<f32>,
}

impl<'a> MusicPlayerBlock<'a> {
    /// Create a new music player block.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tracks: &'a [TrackItem<'a>],
        current_track: &'a mut Option<usize>,
        is_playing: &'a mut bool,
        current_time: &'a mut f32,
        volume: &'a mut f32,
        muted: &'a mut bool,
        playback_rate: &'a mut f32,
        is_looping: &'a mut bool,
        waveform_peaks: &'a [f32],
        markers: &'a [WaveformMarker<'a>],
        regions: &'a [WaveformRegion<'a>],
        spectrum_bands: Option<&'a [f32]>,
        show_spectrum: &'a mut bool,
    ) -> Self {
        Self {
            tracks,
            current_track,
            is_playing,
            current_time,
            volume,
            muted,
            playback_rate,
            is_looping,
            waveform_peaks,
            markers,
            regions,
            spectrum_bands,
            show_spectrum,
            width: None,
        }
    }

    /// Set an explicit block width (default 560.0).
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for MusicPlayerBlock<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let block_w = self.width.unwrap_or(560.0);

        let card_frame = Frame::canvas(ui.style())
            .fill(theme.palette.surface)
            .stroke(Stroke::new(1.0_f32, theme.palette.border))
            .corner_radius(theme.radius.lg())
            .inner_margin(18.0);

        card_frame
            .show(ui, |ui| {
                ui.set_width(block_w - 36.0);

                // Now Playing Hero Header
                let active_item = self.current_track.and_then(|idx| self.tracks.get(idx));
                let title = active_item.map_or("No Track Loaded", |t| t.title);
                let artist = active_item.and_then(|t| t.artist).unwrap_or("Unknown Artist");
                let duration = active_item.map_or(180.0, |t| t.duration_secs);

                ui.horizontal(|ui| {
                    // Artwork album square badge
                    let art_size = 52.0;
                    let (art_rect, _) = ui.allocate_exact_size(Vec2::splat(art_size), Sense::hover());
                    ui.painter().rect_filled(art_rect, theme.radius.md(), theme.palette.track);
                    ui.painter().rect_stroke(
                        art_rect,
                        theme.radius.md(),
                        Stroke::new(1.0_f32, theme.palette.border),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        art_rect.center(),
                        Align2::CENTER_CENTER,
                        "♪",
                        FontId::new(theme.size.label_font() + 6.0, FontFamily::Proportional),
                        theme.palette.primary,
                    );

                    ui.add_space(8.0);

                    // Track & Artist Text
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).strong().size(16.0).color(theme.palette.surface_foreground));
                        ui.label(RichText::new(artist).size(13.0).color(theme.palette.text_dim));
                    });

                    // Spectrum Drawer Toggle
                    if self.spectrum_bands.is_some() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = if *self.show_spectrum { "Hide Spectrum" } else { "Show Spectrum" };
                            if ui.button(label).clicked() {
                                *self.show_spectrum = !*self.show_spectrum;
                            }
                        });
                    }
                });

                ui.add_space(14.0);

                // Interactive Waveform with markers and loop regions
                ui.add(
                    Waveform::new(self.waveform_peaks, self.current_time)
                        .duration(duration)
                        .markers(self.markers)
                        .regions(self.regions)
                        .height(72.0),
                );

                ui.add_space(8.0);

                // Composable Transport Controls
                ui.add(
                    AudioPlayer::new(
                        self.is_playing,
                        self.current_time,
                        duration,
                    )
                    .volume(self.volume, self.muted)
                    .rate(self.playback_rate)
                    .looping(self.is_looping),
                );

                // Optional Spectrum Frequency Drawer
                if *self.show_spectrum {
                    if let Some(bands) = self.spectrum_bands {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(RichText::new("Frequency Spectrum").strong().size(12.0).color(theme.palette.surface_foreground));
                        ui.add_space(4.0);
                        ui.add(Spectrum::new(bands).height(80.0));
                    }
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);

                // Playlist Queue Header
                ui.label(RichText::new("Playlist Queue").strong().size(13.0).color(theme.palette.surface_foreground));
                ui.add_space(6.0);

                // Track List Widget
                ui.add(TrackList::new(self.tracks, self.current_track));
            })
            .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_player_renders_headlessly() {
        let tracks = [
            TrackItem::new(1, "Neon Horizons", 214.0).artist("Synthwave Collective"),
            TrackItem::new(2, "Midnight Drive", 185.0).artist("Kavinsky Tribute"),
        ];
        let mut current_track = Some(0);
        let mut is_playing = false;
        let mut current_time = 45.0;
        let mut volume = 0.8;
        let mut muted = false;
        let mut playback_rate = 1.0;
        let mut is_looping = false;
        let peaks = [0.2, 0.4, 0.8, 0.6, 0.3, 0.9, 0.5, 0.1];
        let markers = [WaveformMarker::new(60.0).label("Drop")];
        let regions = [WaveformRegion::new(30.0, 90.0).label("Chorus")];
        let spectrum = [0.2; 16];
        let mut show_spectrum = true;

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                MusicPlayerBlock::new(
                    &tracks,
                    &mut current_track,
                    &mut is_playing,
                    &mut current_time,
                    &mut volume,
                    &mut muted,
                    &mut playback_rate,
                    &mut is_looping,
                    &peaks,
                    &markers,
                    &regions,
                    Some(&spectrum),
                    &mut show_spectrum,
                )
                .width(500.0),
            );
        });
    }
}

