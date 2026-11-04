//! A high-level microphone configuration and level checking block.
//!
//! Directly inspired by audiocn's `mic-setup.tsx`. Combines device selection,
//! live waveform preview, level metering with clip indicators, input gain adjustment,
//! and automated level verification diagnostics.

use egui::{
    Align, Align2, Color32, FontFamily, FontId, Frame, Layout, Pos2, Response, RichText, Sense,
    Stroke, Ui, Vec2, Widget,
};

use crate::ui::{
    AudioDevice, AudioDeviceSelect, ChannelLevel, ChannelToggle, DbReadout, LevelMeter,
    LiveWaveform, ParameterSlider, Theme,
};

/// Result of an automated microphone level test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MicCheckResult {
    /// Level is well-balanced within the nominal recording zone (-30 dBFS to -1 dBFS).
    Good,
    /// Level is too low (< -30 dBFS).
    Quiet,
    /// Voice is clipping or nearly clipping (>= -1 dBFS).
    Loud,
    /// No detectable signal (< -55 dBFS).
    Silent,
}

impl MicCheckResult {
    /// Evaluate a peak dBFS value into a diagnostic verdict.
    #[must_use]
    pub fn judge(peak_db: f32) -> Self {
        if peak_db < -55.0 {
            Self::Silent
        } else if peak_db >= -1.0 {
            Self::Loud
        } else if peak_db < -30.0 {
            Self::Quiet
        } else {
            Self::Good
        }
    }

    /// Title for the alert message.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Good => "Sounds good",
            Self::Quiet => "Too quiet",
            Self::Loud => "Too loud",
            Self::Silent => "No signal",
        }
    }

    /// Explanatory description.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Good => "Your level sits in the right range.",
            Self::Quiet => "Raise the input gain or move closer to the mic.",
            Self::Loud => "Your voice is clipping. Lower the input gain.",
            Self::Silent => "Check that your microphone is plugged in and unmuted.",
        }
    }

    /// Primary accent color for the alert box.
    #[must_use]
    pub fn color(self, theme: &Theme) -> Color32 {
        match self {
            Self::Good => theme.palette.meter_ok,
            Self::Quiet => theme.palette.meter_warn,
            Self::Loud => theme.palette.meter_clip,
            Self::Silent => theme.palette.text_dim,
        }
    }
}

/// A high-level microphone setup card block.
pub struct MicSetupBlock<'a> {
    devices: &'a [AudioDevice],
    selected_device: &'a mut Option<String>,
    gain_db: &'a mut f32,
    muted: &'a mut bool,
    active: &'a mut bool,
    level: ChannelLevel,
    waveform_samples: &'a [f32],
    check_result: &'a mut Option<MicCheckResult>,
    width: Option<f32>,
}

impl<'a> MicSetupBlock<'a> {
    /// Create a new microphone setup block.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        devices: &'a [AudioDevice],
        selected_device: &'a mut Option<String>,
        gain_db: &'a mut f32,
        muted: &'a mut bool,
        active: &'a mut bool,
        level: ChannelLevel,
        waveform_samples: &'a [f32],
        check_result: &'a mut Option<MicCheckResult>,
    ) -> Self {
        Self {
            devices,
            selected_device,
            gain_db,
            muted,
            active,
            level,
            waveform_samples,
            check_result,
            width: None,
        }
    }

    /// Set an explicit width for the card block (default 420.0).
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for MicSetupBlock<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let block_w = self.width.unwrap_or(420.0);

        let card_frame = Frame::canvas(ui.style())
            .fill(theme.palette.surface)
            .stroke(Stroke::new(1.0_f32, theme.palette.border))
            .corner_radius(theme.radius.lg())
            .inner_margin(16.0);

        card_frame
            .show(ui, |ui| {
                ui.set_width(block_w - 32.0);

                // Header
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Microphone").strong().size(16.0).color(theme.palette.surface_foreground));
                        ui.label(RichText::new("Pick an audio input and check your signal level.").size(12.0).color(theme.palette.text_dim));
                    });
                });
                ui.add_space(12.0);

                // Device Selection & Live Waveform
                ui.label(RichText::new("Input Device").strong().size(12.0).color(theme.palette.surface_foreground));
                ui.add_space(4.0);
                ui.add(AudioDeviceSelect::new(self.selected_device, self.devices));
                ui.add_space(8.0);

                // Live waveform preview banner
                let preview_h = 44.0;
                let (preview_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), preview_h), Sense::hover());
                ui.painter().rect_filled(preview_rect, theme.radius.md(), theme.palette.track);
                ui.painter().rect_stroke(
                    preview_rect,
                    theme.radius.md(),
                    Stroke::new(1.0_f32, theme.palette.border),
                    egui::StrokeKind::Inside,
                );

                if *self.active && !*self.muted {
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(preview_rect.shrink(4.0))
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    child.add(LiveWaveform::new(self.waveform_samples).bar_width(2.0));
                } else {
                    let text = if *self.muted { "Microphone Muted" } else { "Microphone Inactive" };
                    ui.painter().text(
                        preview_rect.center(),
                        Align2::CENTER_CENTER,
                        text,
                        FontId::new(theme.size.label_font(), FontFamily::Proportional),
                        theme.palette.text_dim,
                    );
                }

                // Active toggle button
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let btn_label = if *self.active { "Turn Off Microphone" } else { "Turn On Microphone" };
                    if ui.button(btn_label).clicked() {
                        *self.active = !*self.active;
                    }
                    if *self.active {
                        ui.add(ChannelToggle::mute(self.muted));
                    }
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);

                // Level Meter Section
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Signal Level").strong().size(12.0).color(theme.palette.surface_foreground));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(DbReadout::new(self.level.peak_db));
                    });
                });
                ui.add_space(4.0);

                let channel_slice = [self.level];
                ui.add(
                    LevelMeter::new(&channel_slice)
                        .scale(true)
                        .clip(true)
                        .orientation(crate::ui::Orientation::Horizontal),
                );

                ui.add_space(14.0);

                // Input Gain Slider
                ui.add(
                    ParameterSlider::new(self.gain_db, -48.0, 12.0)
                        .label("Input Gain")
                        .unit("dB")
                        .reset_value(0.0),
                );

                ui.add_space(12.0);

                // Diagnostic Test Section
                ui.horizontal(|ui| {
                    if ui.button("Test Level").clicked() {
                        *self.check_result = Some(MicCheckResult::judge(self.level.peak_db));
                    }
                    if self.check_result.is_some() && ui.button("Clear").clicked() {
                        *self.check_result = None;
                    }
                });

                if let Some(verdict) = *self.check_result {
                    ui.add_space(8.0);
                    let alert_rect = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover()).0;
                    let alert_color = verdict.color(&theme);
                    ui.painter().rect_filled(
                        alert_rect,
                        theme.radius.md(),
                        crate::oklab::with_alpha(alert_color, 0.11),
                    );
                    ui.painter().rect_stroke(
                        alert_rect,
                        theme.radius.md(),
                        Stroke::new(1.0_f32, alert_color),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        Pos2::new(alert_rect.min.x + 10.0, alert_rect.min.y + 14.0),
                        Align2::LEFT_CENTER,
                        verdict.title(),
                        FontId::new(theme.size.label_font(), FontFamily::Proportional),
                        alert_color,
                    );
                    ui.painter().text(
                        Pos2::new(alert_rect.min.x + 10.0, alert_rect.min.y + 32.0),
                        Align2::LEFT_CENTER,
                        verdict.description(),
                        FontId::new(theme.size.value_font(), FontFamily::Proportional),
                        theme.palette.surface_foreground,
                    );
                }
            })
            .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_check_result_verdicts() {
        assert_eq!(MicCheckResult::judge(-60.0), MicCheckResult::Silent);
        assert_eq!(MicCheckResult::judge(-40.0), MicCheckResult::Quiet);
        assert_eq!(MicCheckResult::judge(-12.0), MicCheckResult::Good);
        assert_eq!(MicCheckResult::judge(0.0), MicCheckResult::Loud);
        assert_eq!(MicCheckResult::judge(2.0), MicCheckResult::Loud);
    }

    #[test]
    fn mic_setup_renders_headlessly() {
        let devices = [
            AudioDevice::new("mic_built_in", "MacBook Pro Microphone").default_device(true),
            AudioDevice::new("mic_usb", "USB Studio Condenser"),
        ];
        let mut selected = Some("mic_built_in".to_string());
        let mut gain = 6.0;
        let mut muted = false;
        let mut active = true;
        let level = ChannelLevel::new(-14.0);
        let waveform = [0.1, 0.3, -0.2, 0.5, -0.4, 0.2];
        let mut check_result = Some(MicCheckResult::Good);

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                MicSetupBlock::new(
                    &devices,
                    &mut selected,
                    &mut gain,
                    &mut muted,
                    &mut active,
                    level,
                    &waveform,
                    &mut check_result,
                )
                .width(400.0),
            );
        });
    }
}

