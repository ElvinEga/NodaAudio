//! Controls for dynamics processors: compressors, expander/gates, and limiters.

use egui::{Response, Ui, Widget};

use crate::taper::Taper;
use crate::ui::controls::channel_toggle::ChannelToggle;
use crate::ui::controls::parameter_slider::ParameterSlider;

/// Parameter control bank for a compressor.
pub struct CompressorControls<'a> {
    pub threshold_db: &'a mut f32,
    pub ratio: &'a mut f32,
    pub attack_ms: &'a mut f32,
    pub release_ms: &'a mut f32,
    pub knee_db: &'a mut f32,
    pub makeup_gain_db: &'a mut f32,
    pub sidechain_hz: Option<&'a mut f32>,
    pub sidechain_active: Option<&'a mut bool>,
    width: Option<f32>,
}

impl<'a> CompressorControls<'a> {
    /// Create a new compressor control bank.
    pub fn new(
        threshold_db: &'a mut f32,
        ratio: &'a mut f32,
        attack_ms: &'a mut f32,
        release_ms: &'a mut f32,
        knee_db: &'a mut f32,
        makeup_gain_db: &'a mut f32,
    ) -> Self {
        Self {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            sidechain_hz: None,
            sidechain_active: None,
            width: None,
        }
    }

    /// Add sidechain high-pass filter frequency and toggle.
    #[must_use]
    pub fn sidechain(mut self, hz: &'a mut f32, active: &'a mut bool) -> Self {
        self.sidechain_hz = Some(hz);
        self.sidechain_active = Some(active);
        self
    }

    /// Set slider width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for CompressorControls<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let slider_w = self.width.unwrap_or(220.0);

        ui.vertical(|ui| {
            ui.add(
                ParameterSlider::new(self.threshold_db, -60.0, 0.0)
                    .label("Threshold")
                    .unit("dB")
                    .decimals(1)
                    .width(slider_w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.ratio, 1.0, 20.0)
                    .label("Ratio")
                    .unit(":1")
                    .decimals(1)
                    .width(slider_w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.attack_ms, 0.1, 200.0)
                    .taper(Taper::Log)
                    .label("Attack")
                    .unit("ms")
                    .decimals(1)
                    .width(slider_w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.release_ms, 10.0, 2000.0)
                    .taper(Taper::Log)
                    .label("Release")
                    .unit("ms")
                    .decimals(0)
                    .width(slider_w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.knee_db, 0.0, 24.0)
                    .label("Knee")
                    .unit("dB")
                    .decimals(1)
                    .width(slider_w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.makeup_gain_db, -12.0, 24.0)
                    .label("Makeup Gain")
                    .unit("dB")
                    .decimals(1)
                    .width(slider_w),
            );

            if let (Some(hz), Some(active)) = (self.sidechain_hz, self.sidechain_active) {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(ChannelToggle::new(
                        active,
                        crate::ui::controls::channel_toggle::ChannelToggleTone::Monitor,
                        "SC",
                    ));
                    ui.label("Sidechain");
                    ui.add_space(4.0);
                    ui.add(
                        ParameterSlider::new(hz, 20.0, 1000.0)
                            .taper(Taper::Log)
                            .label("HP Freq")
                            .unit("Hz")
                            .decimals(0)
                            .width((slider_w - 90.0).max(100.0)),
                    );
                });
            }
        })
        .response
    }
}

/// Parameter controls for a brickwall / mastering limiter.
pub struct LimiterControls<'a> {
    pub ceiling_db: &'a mut f32,
    pub release_ms: &'a mut f32,
    pub true_peak: &'a mut bool,
    width: Option<f32>,
}

impl<'a> LimiterControls<'a> {
    /// Create new limiter controls.
    pub fn new(ceiling_db: &'a mut f32, release_ms: &'a mut f32, true_peak: &'a mut bool) -> Self {
        Self {
            ceiling_db,
            release_ms,
            true_peak,
            width: None,
        }
    }

    /// Set control width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for LimiterControls<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = self.width.unwrap_or(220.0);
        ui.vertical(|ui| {
            ui.add(
                ParameterSlider::new(self.ceiling_db, -24.0, 0.0)
                    .label("Ceiling")
                    .unit("dBFS")
                    .decimals(1)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.release_ms, 1.0, 1000.0)
                    .taper(Taper::Log)
                    .label("Release")
                    .unit("ms")
                    .decimals(1)
                    .width(w),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(ChannelToggle::new(
                    self.true_peak,
                    crate::ui::controls::channel_toggle::ChannelToggleTone::Solo,
                    "TP",
                ));
                ui.label("True Peak Limiting");
            });
        })
        .response
    }
}

/// Parameter controls for an expander / noise gate.
pub struct GateControls<'a> {
    pub threshold_db: &'a mut f32,
    pub range_db: &'a mut f32,
    pub attack_ms: &'a mut f32,
    pub hold_ms: &'a mut f32,
    pub release_ms: &'a mut f32,
    pub hysteresis_db: &'a mut f32,
    width: Option<f32>,
}

impl<'a> GateControls<'a> {
    /// Create new noise gate controls.
    pub fn new(
        threshold_db: &'a mut f32,
        range_db: &'a mut f32,
        attack_ms: &'a mut f32,
        hold_ms: &'a mut f32,
        release_ms: &'a mut f32,
        hysteresis_db: &'a mut f32,
    ) -> Self {
        Self {
            threshold_db,
            range_db,
            attack_ms,
            hold_ms,
            release_ms,
            hysteresis_db,
            width: None,
        }
    }

    /// Set control width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for GateControls<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = self.width.unwrap_or(220.0);
        ui.vertical(|ui| {
            ui.add(
                ParameterSlider::new(self.threshold_db, -80.0, 0.0)
                    .label("Threshold")
                    .unit("dB")
                    .decimals(1)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.range_db, -80.0, 0.0)
                    .label("Range / Floor")
                    .unit("dB")
                    .decimals(1)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.attack_ms, 0.05, 50.0)
                    .taper(Taper::Log)
                    .label("Attack")
                    .unit("ms")
                    .decimals(2)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.hold_ms, 0.0, 500.0)
                    .taper(Taper::Log)
                    .label("Hold")
                    .unit("ms")
                    .decimals(1)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.release_ms, 5.0, 2000.0)
                    .taper(Taper::Log)
                    .label("Release")
                    .unit("ms")
                    .decimals(0)
                    .width(w),
            );
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.hysteresis_db, 0.0, 12.0)
                    .label("Hysteresis")
                    .unit("dB")
                    .decimals(1)
                    .width(w),
            );
        })
        .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamics_controls_render_headlessly() {
        let mut thresh = -18.0;
        let mut ratio = 4.0;
        let mut att = 10.0;
        let mut rel = 100.0;
        let mut knee = 6.0;
        let mut makeup = 2.0;

        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(CompressorControls::new(
                &mut thresh,
                &mut ratio,
                &mut att,
                &mut rel,
                &mut knee,
                &mut makeup,
            ));
        });
        assert!(!output.shapes.is_empty(), "Controls should emit shapes");
    }
}
