//! A high-level drum pad soundboard and trigger performance block.
//!
//! Directly inspired by audiocn's `soundboard.tsx`. Combines responsive drum pad
//! trigger matrices (`SoundPadGrid`), master volume control, panic stop-all triggers,
//! and sound bank navigation.

use egui::{
    Align, Frame, Layout, Response, RichText, Sense, Stroke,
    Ui, Vec2, Widget,
};

use crate::ui::{SoundPadGrid, SoundPadItem, Theme, VolumeControl};

/// High-level soundboard launchpad block.
pub struct SoundboardBlock<'a, 'p> {
    title: &'a str,
    pads: &'a mut [SoundPadItem<'p>],
    volume: &'a mut f32,
    muted: &'a mut bool,
    columns: usize,
    width: Option<f32>,
}

impl<'a, 'p> SoundboardBlock<'a, 'p> {
    /// Create a new soundboard block.
    pub fn new(
        title: &'a str,
        pads: &'a mut [SoundPadItem<'p>],
        volume: &'a mut f32,
        muted: &'a mut bool,
    ) -> Self {
        Self {
            title,
            pads,
            volume,
            muted,
            columns: 4,
            width: None,
        }
    }

    /// Set number of pad columns (default 4).
    #[must_use]
    pub fn columns(mut self, cols: usize) -> Self {
        self.columns = cols.max(1);
        self
    }

    /// Set an explicit block width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for SoundboardBlock<'_, '_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let block_w = self.width.unwrap_or(380.0);

        let card_frame = Frame::canvas(ui.style())
            .fill(theme.palette.surface)
            .stroke(Stroke::new(1.0_f32, theme.palette.border))
            .corner_radius(theme.radius.lg())
            .inner_margin(18.0);

        card_frame
            .show(ui, |ui| {
                ui.set_width(block_w - 36.0);

                // Header with title and Stop All panic button
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.title).strong().size(16.0).color(theme.palette.surface_foreground));
                        ui.label(RichText::new("Trigger samples with hotkeys or clicks.").size(12.0).color(theme.palette.text_dim));
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("⏹ Stop All").clicked() {
                            for pad in self.pads.iter_mut() {
                                pad.is_playing = false;
                            }
                        }
                    });
                });

                ui.add_space(12.0);

                // Master Volume Bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Master Volume").size(12.0).color(theme.palette.text_dim));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(VolumeControl::new(self.volume).mute(self.muted).width(160.0));
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);

                // Drum Pad Grid
                ui.add(
                    SoundPadGrid::new(self.pads)
                        .columns(self.columns)
                        .gap(8.0),
                );

                ui.add_space(12.0);

                // Active triggers summary footer
                let active_count = self.pads.iter().filter(|p| p.is_playing).count();
                let status_text = if active_count == 0 {
                    "Ready — All pads idle".to_owned()
                } else {
                    format!("Playing {active_count} pad(s) active")
                };

                ui.horizontal(|ui| {
                    let dot_color = if active_count > 0 { theme.palette.primary } else { theme.palette.text_dim };
                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 3.0, dot_color);
                    ui.add_space(2.0);
                    ui.label(RichText::new(status_text).size(11.0).color(theme.palette.text_dim));
                });
            })
            .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundboard_renders_headlessly() {
        let mut pads = [
            SoundPadItem::new(1, "Kick").hotkey("1"),
            SoundPadItem::new(2, "Snare").hotkey("2"),
            SoundPadItem::new(3, "Hi-Hat").hotkey("3"),
            SoundPadItem::new(4, "Clap").hotkey("4"),
        ];
        let mut volume = 0.9;
        let mut muted = false;

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                SoundboardBlock::new("Live Soundboard", &mut pads, &mut volume, &mut muted)
                    .columns(2)
                    .width(360.0),
            );
        });
    }
}

