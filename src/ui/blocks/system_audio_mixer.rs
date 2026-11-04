//! A complete broadcast and system audio mixing desk block.
//!
//! Directly inspired by audiocn's `system-audio-mixer.tsx`. Assembles multi-channel
//! routing lanes (`ChannelStrip`) with live metering, faders, pan controls, mute/solo
//! exclusive bus management, and a dedicated master section (`Mixer`).

use egui::{
    Align, Frame, Layout, Response, RichText, Stroke,
    Ui, Widget,
};

use crate::ui::{
    Mixer, MixerChannelData, MixerMasterData, Orientation, Theme,
};

/// High-level system audio mixing desk block.
pub struct SystemAudioMixerBlock<'a> {
    title: &'a str,
    channels: Vec<MixerChannelData<'a>>,
    master: Option<MixerMasterData<'a>>,
    orientation: &'a mut Orientation,
    width: Option<f32>,
}

impl<'a> SystemAudioMixerBlock<'a> {
    /// Create a new system audio mixer block.
    pub fn new(
        title: &'a str,
        channels: Vec<MixerChannelData<'a>>,
        master: Option<MixerMasterData<'a>>,
        orientation: &'a mut Orientation,
    ) -> Self {
        Self {
            title,
            channels,
            master,
            orientation,
            width: None,
        }
    }

    /// Set an explicit block width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for SystemAudioMixerBlock<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);

        let card_frame = Frame::canvas(ui.style())
            .fill(theme.palette.surface)
            .stroke(Stroke::new(1.0_f32, theme.palette.border))
            .corner_radius(theme.radius.lg())
            .inner_margin(18.0);

        card_frame
            .show(ui, |ui| {
                if let Some(w) = self.width {
                    ui.set_width(w - 36.0);
                }

                // Mixer Header Toolbar
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.title).strong().size(16.0).color(theme.palette.surface_foreground));
                        ui.label(RichText::new("Multi-source routing, solo bus, and master limiter.").size(12.0).color(theme.palette.text_dim));
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Orientation toggle
                        let is_vert = *self.orientation == Orientation::Vertical;
                        let toggle_label = if is_vert { "Horizontal View" } else { "Console View" };
                        if ui.button(toggle_label).clicked() {
                            *self.orientation = if is_vert {
                                Orientation::Horizontal
                            } else {
                                Orientation::Vertical
                            };
                        }

                        // Reset unity button
                        if ui.button("Reset Unity").clicked() {
                            for ch in self.channels.iter_mut() {
                                *ch.gain_db = 0.0;
                                *ch.pan = 0.0;
                                *ch.muted = false;
                                *ch.solo = false;
                            }
                            if let Some(ref mut m) = self.master {
                                if let Some(ref mut g) = m.gain_db {
                                    **g = 0.0;
                                }
                            }
                        }
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);

                // Multi-channel Mixer Component
                let mut mixer = Mixer::new().orientation(*self.orientation);
                for ch in self.channels {
                    mixer = mixer.channel(ch);
                }
                if let Some(m) = self.master {
                    mixer = mixer.master(m);
                }
                ui.add(mixer);
            })
            .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::ChannelLevel;

    #[test]
    fn system_audio_mixer_renders_headlessly() {
        let mut gain1 = -4.0;
        let mut pan1 = -0.3;
        let mut muted1 = false;
        let mut solo1 = false;
        let levels1 = [ChannelLevel::new(-6.0)];

        let mut orientation = Orientation::Vertical;

        let mut master_gain = 0.0;
        let mut master_mute = false;
        let master_levels = [ChannelLevel::new(-6.0), ChannelLevel::new(-6.0)];

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let channels = vec![
                MixerChannelData {
                    name: "Desktop Audio",
                    number: 1,
                    accent: None,
                    gain_db: &mut gain1,
                    pan: &mut pan1,
                    muted: &mut muted1,
                    solo: &mut solo1,
                    levels: &levels1,
                },
            ];

            let master = Some(MixerMasterData {
                name: "Master Out",
                gain_db: Some(&mut master_gain),
                pan: None,
                muted: Some(&mut master_mute),
                levels: &master_levels,
            });

            let _ = ui.add(
                SystemAudioMixerBlock::new("System Audio Routing", channels, master, &mut orientation)
                    .width(480.0),
            );
        });
    }
}

