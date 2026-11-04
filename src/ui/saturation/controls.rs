//! Parameter banks for saturators and distortion units.
//!
//! Both banks are reusable in a rack, a popover, or beside a transfer
//! curve: they borrow the values and draw the controls, and never own
//! signal or open a device.

use egui::{Response, Ui, Widget};

use crate::ui::controls::parameter_slider::ParameterSlider;
use crate::ui::saturation::SaturationType;
use crate::ui::saturation::type_select::SaturationTypeSelect;

/// Gap between the two parameter columns.
const COLUMN_GAP: f32 = 24.0;

/// Slider width in a two-column bank.
const COLUMN: f32 = 150.0;

/// Controls for an analog-style saturator: Drive, Mix, Tone and Bias.
///
/// Drive is a linear gain into the waveshaper (1.0 is unity); mix is a
/// percentage; tone tilts the post-shaper balance from dark (-1) to bright
/// (+1); bias offsets the shaper's operating point.
///
/// ```ignore
/// ui.add(SaturatorControls::new(&mut drive, &mut mix, &mut tone, &mut bias));
/// ```
#[must_use = "SaturatorControls draw nothing until they are added to a Ui"]
pub struct SaturatorControls<'a> {
    drive: &'a mut f32,
    mix_percent: &'a mut f32,
    tone: &'a mut f32,
    bias: &'a mut f32,
    sat_type: Option<&'a mut SaturationType>,
    output_db: Option<&'a mut f32>,
    width: Option<f32>,
}

impl<'a> SaturatorControls<'a> {
    /// Create a saturator control bank.
    pub fn new(
        drive: &'a mut f32,
        mix_percent: &'a mut f32,
        tone: &'a mut f32,
        bias: &'a mut f32,
    ) -> Self {
        Self {
            drive,
            mix_percent,
            tone,
            bias,
            sat_type: None,
            output_db: None,
            width: None,
        }
    }

    /// Add a character selector above the knobs.
    #[must_use]
    pub fn sat_type(mut self, sat_type: &'a mut SaturationType) -> Self {
        self.sat_type = Some(sat_type);
        self
    }

    /// Add an output trim in dB.
    #[must_use]
    pub fn output_db(mut self, output_db: &'a mut f32) -> Self {
        self.output_db = Some(output_db);
        self
    }

    /// Width of one column of sliders. Default 150 points.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(90.0));
        self
    }
}

impl Widget for SaturatorControls<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or(COLUMN);
        ui.vertical(|ui| {
            if let Some(sat_type) = self.sat_type {
                ui.add(SaturationTypeSelect::new(sat_type).width(width * 2.0 + COLUMN_GAP));
                ui.add_space(6.0);
            }

            egui::Grid::new(ui.id().with("saturator_controls"))
                .num_columns(2)
                .spacing([COLUMN_GAP, 4.0])
                .show(ui, |ui| {
                    ui.add(
                        ParameterSlider::new(self.drive, 1.0, 10.0)
                            .label("Drive")
                            .unit("x")
                            .decimals(1)
                            .width(width),
                    );
                    ui.add(
                        ParameterSlider::new(self.mix_percent, 0.0, 100.0)
                            .label("Mix")
                            .unit("%")
                            .decimals(0)
                            .width(width),
                    );
                    ui.end_row();

                    ui.add(
                        ParameterSlider::new(self.tone, -1.0, 1.0)
                            .label("Tone")
                            .decimals(2)
                            .origin(0.0)
                            .width(width),
                    );
                    ui.add(
                        ParameterSlider::new(self.bias, -0.8, 0.8)
                            .label("Bias")
                            .decimals(2)
                            .origin(0.0)
                            .width(width),
                    );
                    ui.end_row();
                });

            if let Some(output_db) = self.output_db {
                ui.add_space(4.0);
                ui.add(
                    ParameterSlider::new(output_db, -24.0, 24.0)
                        .label("Output")
                        .unit("dB")
                        .decimals(1)
                        .origin(0.0)
                        .width(width * 2.0 + COLUMN_GAP),
                );
            }
        })
        .response
    }
}

/// Controls for a distortion unit: Drive, Type, Tone and Output.
///
/// ```ignore
/// ui.add(DistortionControls::new(&mut drive, &mut kind, &mut tone, &mut output_db));
/// ```
#[must_use = "DistortionControls draw nothing until they are added to a Ui"]
pub struct DistortionControls<'a> {
    drive: &'a mut f32,
    sat_type: &'a mut SaturationType,
    tone: &'a mut f32,
    output_db: &'a mut f32,
    width: Option<f32>,
}

impl<'a> DistortionControls<'a> {
    /// Create a distortion control bank.
    pub fn new(
        drive: &'a mut f32,
        sat_type: &'a mut SaturationType,
        tone: &'a mut f32,
        output_db: &'a mut f32,
    ) -> Self {
        Self {
            drive,
            sat_type,
            tone,
            output_db,
            width: None,
        }
    }

    /// Width of one column of sliders. Default 150 points.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(90.0));
        self
    }
}

impl Widget for DistortionControls<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or(COLUMN);
        ui.vertical(|ui| {
            ui.add(SaturationTypeSelect::new(self.sat_type).width(width * 2.0 + COLUMN_GAP));
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = COLUMN_GAP;
                ui.add(
                    ParameterSlider::new(self.drive, 1.0, 20.0)
                        .label("Drive")
                        .unit("x")
                        .decimals(1)
                        .width(width),
                );
                ui.add(
                    ParameterSlider::new(self.tone, -1.0, 1.0)
                        .label("Tone")
                        .decimals(2)
                        .origin(0.0)
                        .width(width),
                );
            });
            ui.add_space(4.0);
            ui.add(
                ParameterSlider::new(self.output_db, -24.0, 24.0)
                    .label("Output")
                    .unit("dB")
                    .decimals(1)
                    .origin(0.0)
                    .width(width * 2.0 + COLUMN_GAP),
            );
        })
        .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturator_controls_render_headlessly() {
        let mut drive = 3.0;
        let mut mix = 100.0;
        let mut tone = 0.0;
        let mut bias = 0.0;
        let mut kind = SaturationType::Tape;
        let mut output = 0.0;

        let ctx = egui::Context::default();
        let output_frames = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                SaturatorControls::new(&mut drive, &mut mix, &mut tone, &mut bias)
                    .sat_type(&mut kind)
                    .output_db(&mut output),
            );
        });
        assert!(
            !output_frames.shapes.is_empty(),
            "saturator controls should emit shapes"
        );
    }

    #[test]
    fn distortion_controls_render_headlessly() {
        let mut drive = 6.0;
        let mut kind = SaturationType::Fuzz;
        let mut tone = 0.2;
        let mut output = -3.0;

        let ctx = egui::Context::default();
        let output_frames = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(DistortionControls::new(
                &mut drive,
                &mut kind,
                &mut tone,
                &mut output,
            ));
        });
        assert!(
            !output_frames.shapes.is_empty(),
            "distortion controls should emit shapes"
        );
    }
}
