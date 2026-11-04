//! A compact audio popover trigger and configuration panel.
//!
//! Directly inspired by audiocn's `quick-audio-popover.tsx`. Displays a status
//! icon alongside a miniature 5-bar visualizer that expands into an audio device
//! and master volume configuration popover.

use egui::{
    Align, Frame, Layout, Response, RichText, Stroke,
    Ui, Vec2, Widget,
};

use crate::ui::{
    AudioDevice, AudioDeviceSelect, BarVisualizer, ChannelLevel, LevelMeter, Theme,
    VolumeControl,
};

/// High-level compact audio popover block.
pub struct QuickAudioPopover<'a> {
    id_source: &'a str,
    devices: &'a [AudioDevice],
    selected_device: &'a mut Option<String>,
    volume: &'a mut f32,
    muted: &'a mut bool,
    levels: [ChannelLevel; 2],
    bar_levels: &'a [f32],
}

impl<'a> QuickAudioPopover<'a> {
    /// Create a new quick audio popover.
    pub fn new(
        id_source: &'a str,
        devices: &'a [AudioDevice],
        selected_device: &'a mut Option<String>,
        volume: &'a mut f32,
        muted: &'a mut bool,
        levels: [ChannelLevel; 2],
        bar_levels: &'a [f32],
    ) -> Self {
        Self {
            id_source,
            devices,
            selected_device,
            volume,
            muted,
            levels,
            bar_levels,
        }
    }
}

impl Widget for QuickAudioPopover<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let id = ui.make_persistent_id(self.id_source);
        let mut is_open = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));

        // Trigger Button: Speaker icon + Mini Bar Visualizer
        let trigger_resp = ui.horizontal(|ui| {
            let btn_text = if *self.muted { "🔇" } else { "🔊" };
            let trigger = ui.button(btn_text);
            if trigger.clicked() {
                is_open = !is_open;
                ui.data_mut(|d| d.insert_temp(id, is_open));
            }

            // Mini 5-bar visualizer inline
            ui.add(
                BarVisualizer::new(self.bar_levels)
                    .bar_count(5)
                    .bar_width(2.5)
                    .height(16.0),
            );

            trigger
        }).inner;

        // Floating Popover Panel when open
        if is_open {
            egui::Area::new(id.with("popup"))
                .order(egui::Order::Foreground)
                .fixed_pos(trigger_resp.rect.left_bottom() + Vec2::new(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    let card_frame = Frame::popup(ui.style())
                        .fill(theme.palette.surface)
                        .stroke(Stroke::new(1.0_f32, theme.palette.border))
                        .corner_radius(theme.radius.lg())
                        .inner_margin(14.0);

                    card_frame.show(ui, |ui| {
                        ui.set_width(280.0);

                        // Popover Header
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Quick Audio").strong().size(14.0).color(theme.palette.surface_foreground));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button("✕").clicked() {
                                    ui.data_mut(|d| d.insert_temp(id, false));
                                }
                            });
                        });
                        ui.add_space(8.0);

                        // Device Selector
                        ui.label(RichText::new("Output Device").size(11.0).color(theme.palette.text_dim));
                        ui.add_space(2.0);
                        ui.add(AudioDeviceSelect::new(self.selected_device, self.devices));

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Volume Control
                        ui.label(RichText::new("Output Volume").size(11.0).color(theme.palette.text_dim));
                        ui.add_space(4.0);
                        ui.add(VolumeControl::new(self.volume).mute(self.muted).width(260.0));

                        ui.add_space(10.0);

                        // Mini Output Level Meter
                        ui.label(RichText::new("Output Level").size(11.0).color(theme.palette.text_dim));
                        ui.add_space(2.0);
                        ui.add(
                            LevelMeter::stereo(&self.levels)
                                .scale(false)
                                .clip(true)
                                .orientation(crate::ui::Orientation::Horizontal),
                        );
                    });
                });
        }

        trigger_resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_audio_popover_renders_headlessly() {
        let devices = [
            AudioDevice::new("sys_speakers", "MacBook Pro Speakers").default_device(true),
            AudioDevice::new("sys_headphones", "External Headphones"),
        ];
        let mut selected = Some("sys_speakers".to_string());
        let mut volume = 0.75;
        let mut muted = false;
        let levels = [ChannelLevel::new(-12.0), ChannelLevel::new(-14.0)];
        let bar_levels = [0.4, 0.7, 0.5, 0.8, 0.3];

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(QuickAudioPopover::new(
                "test_popover",
                &devices,
                &mut selected,
                &mut volume,
                &mut muted,
                levels,
                &bar_levels,
            ));
        });
    }
}

