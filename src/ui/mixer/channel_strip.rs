//! A channel strip: one lane of a mixer composing meters, fader, pan and toggles.
//!
//! Directly inspired by audiocn's `channel-strip.tsx`.

use egui::{
    Color32, FontFamily, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use crate::taper::Taper;
use crate::ui::Orientation;
use crate::ui::controls::channel_toggle::ChannelToggle;
use crate::ui::controls::fader::Fader;
use crate::ui::controls::knob::Knob;
use crate::ui::meters::db_readout::DbReadout;
use crate::ui::meters::level_meter::{ChannelLevel, LevelMeter, MeterStyle};
use crate::ui::theme::{Size, Theme};

/// Visual variant for a channel strip container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelStripVariant {
    /// Default surface with subtle border.
    Default,
    /// Card with defined background, border, and rounded corners.
    Card,
    /// Master channel with accent border.
    Master,
    /// Ghost background (minimalist, no border).
    Ghost,
}

impl Default for ChannelStripVariant {
    fn default() -> Self {
        Self::Card
    }
}

/// One channel of a mixer: a source with its meter, fader, pan and toggles.
///
/// ```ignore
/// ui.add(
///     ChannelStrip::new("Lead Vocal")
///         .gain(&mut gain_db)
///         .pan(&mut pan)
///         .mute(&mut muted)
///         .solo(&mut solo)
///         .levels(&levels)
///         .accent(Color32::from_rgb(0, 180, 255)),
/// );
/// ```
#[must_use = "a ChannelStrip draws nothing until it is added to a Ui"]
pub struct ChannelStrip<'a> {
    name: &'a str,
    number: Option<usize>,
    accent: Option<Color32>,
    orientation: Orientation,
    variant: ChannelStripVariant,
    size: Option<Size>,
    muted: Option<&'a mut bool>,
    solo: Option<&'a mut bool>,
    dimmed: bool,
    gain_db: Option<&'a mut f32>,
    pan: Option<&'a mut f32>,
    levels: &'a [ChannelLevel],
    meter_style: MeterStyle,
    min_db: f32,
    max_db: f32,
    show_scale: bool,
    show_clip: bool,
    show_readout: bool,
}

impl<'a> ChannelStrip<'a> {
    /// Create a channel strip with a track name.
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            number: None,
            accent: None,
            orientation: Orientation::Vertical,
            variant: ChannelStripVariant::Card,
            size: None,
            muted: None,
            solo: None,
            dimmed: false,
            gain_db: None,
            pan: None,
            levels: &[],
            meter_style: MeterStyle::Segmented,
            min_db: -60.0,
            max_db: 6.0,
            show_scale: false,
            show_clip: true,
            show_readout: true,
        }
    }

    /// Set an optional 1-based channel track number.
    pub fn number(mut self, number: usize) -> Self {
        self.number = Some(number);
        self
    }

    /// Set a color tag for the channel (e.g. Drums blue, Vox yellow).
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Set orientation (Vertical console strip or Horizontal track row).
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Set container variant style.
    pub fn variant(mut self, variant: ChannelStripVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set size ladder.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Bind the mute state.
    pub fn mute(mut self, muted: &'a mut bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// Bind the solo state.
    pub fn solo(mut self, solo: &'a mut bool) -> Self {
        self.solo = Some(solo);
        self
    }

    /// Set whether this strip is dimmed by another channel's solo.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Bind the gain parameter in dB.
    pub fn gain(mut self, gain_db: &'a mut f32) -> Self {
        self.gain_db = Some(gain_db);
        self
    }

    /// Bind the pan parameter (-1.0..=1.0).
    pub fn pan(mut self, pan: &'a mut f32) -> Self {
        self.pan = Some(pan);
        self
    }

    /// Supply channel metering levels.
    pub fn levels(mut self, levels: &'a [ChannelLevel]) -> Self {
        self.levels = levels;
        self
    }

    /// Meter display style (Segmented, Continuous, or Lcd).
    pub fn meter_style(mut self, style: MeterStyle) -> Self {
        self.meter_style = style;
        self
    }

    /// Metering range in dB (default -60.0..6.0).
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Whether to show the dB scale beside the meter/fader.
    pub fn scale(mut self, show: bool) -> Self {
        self.show_scale = show;
        self
    }

    /// Whether to show the clip lamp.
    pub fn clip(mut self, show: bool) -> Self {
        self.show_clip = show;
        self
    }

    /// Whether to show the numeric dB readout.
    pub fn readout(mut self, show: bool) -> Self {
        self.show_readout = show;
        self
    }

    /// Natural dimensions of the channel strip.
    pub fn desired_size(&self, size: Size) -> Vec2 {
        match self.orientation {
            Orientation::Vertical => match size {
                Size::Small => Vec2::new(76.0, 240.0),
                Size::Medium => Vec2::new(96.0, 300.0),
                Size::Large => Vec2::new(118.0, 360.0),
            },
            Orientation::Horizontal => match size {
                Size::Small => Vec2::new(340.0, 52.0),
                Size::Medium => Vec2::new(420.0, 64.0),
                Size::Large => Vec2::new(500.0, 78.0),
            },
        }
    }

    /// Add this channel strip to `ui`.
    pub fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(size);

        let (rect, mut response) = ui.allocate_exact_size(desired, Sense::hover());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        // Draw background container
        let is_muted = self.muted.as_ref().map_or(false, |m| **m);
        let is_dimmed = self.dimmed || is_muted;

        let (bg, stroke) = match self.variant {
            ChannelStripVariant::Card => {
                let stroke_color = if self.variant == ChannelStripVariant::Master {
                    theme.palette.primary.linear_multiply(0.4)
                } else {
                    theme.palette.border
                };
                (theme.palette.surface, Stroke::new(1.0_f32, stroke_color))
            }
            ChannelStripVariant::Master => (
                theme.palette.surface,
                Stroke::new(1.5_f32, theme.palette.meter_ok.linear_multiply(0.6)),
            ),
            ChannelStripVariant::Default => (
                theme.palette.track.linear_multiply(0.5),
                Stroke::new(1.0_f32, theme.palette.border),
            ),
            ChannelStripVariant::Ghost => (Color32::TRANSPARENT, Stroke::NONE),
        };

        let rounding = 8.0;
        ui.painter().rect(rect, rounding, bg, stroke, StrokeKind::Inside);

        // Draw accent bar if provided
        if let Some(accent) = self.accent {
            let accent_color = if is_dimmed {
                accent.linear_multiply(0.4)
            } else {
                accent
            };
            match self.orientation {
                Orientation::Vertical => {
                    let bar_rect = Rect::from_min_size(
                        egui::pos2(rect.left() + 10.0, rect.top() + 3.0),
                        Vec2::new(rect.width() - 20.0, 2.5),
                    );
                    ui.painter().rect_filled(bar_rect, 1.5, accent_color);
                }
                Orientation::Horizontal => {
                    let bar_rect = Rect::from_min_size(
                        egui::pos2(rect.left() + 3.0, rect.top() + 8.0),
                        Vec2::new(2.5, rect.height() - 16.0),
                    );
                    ui.painter().rect_filled(bar_rect, 1.5, accent_color);
                }
            }
        }

        // Inner layout
        let inner_rect = rect.shrink(6.0);
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );

        match self.orientation {
            Orientation::Vertical => {
                self.paint_vertical(&mut child_ui, &theme, size, &mut response);
            }
            Orientation::Horizontal => {
                self.paint_horizontal(&mut child_ui, &theme, size, &mut response);
            }
        }

        response
    }

    fn paint_vertical(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        size: Size,
        outer_response: &mut Response,
    ) {
        let is_muted = self.muted.as_ref().map_or(false, |m| **m);
        let is_dimmed = self.dimmed || is_muted;

        // 1. Header: track number / badge & title
        ui.vertical_centered(|ui| {
            ui.add_space(3.0);
            let title_color = if is_dimmed {
                theme.palette.text_dim
            } else {
                theme.palette.foreground
            };

            let label_text = if let Some(num) = self.number {
                format!("{num}  {}", self.name)
            } else {
                self.name.to_string()
            };

            let font_id = FontId::new(theme.size.label_font() + 1.0, FontFamily::Proportional);
            ui.label(egui::RichText::new(label_text).font(font_id).color(title_color).strong());
        });

        ui.add_space(4.0);

        // 2. Pan Knob
        if let Some(pan_val) = self.pan.as_deref_mut() {
            let knob_diam = match size {
                Size::Small => 28.0,
                Size::Medium => 34.0,
                Size::Large => 40.0,
            };
            let resp = ui.add(
                Knob::pan(pan_val)
                    .diameter(knob_diam)
                    .label("Pan")
                    .value_readout(false),
            );
            if resp.changed() {
                outer_response.mark_changed();
            }
        }

        ui.add_space(4.0);

        // 3. Fader + Meter side-by-side in center
        let meter_levels = if is_muted {
            // Render quiet levels when muted
            vec![]
        } else {
            self.levels.to_vec()
        };

        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), ui.available_height() - 50.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.columns(2, |columns| {
                    // Left column: Level meter
                    columns[0].vertical_centered(|ui| {
                        let meter = LevelMeter::new(&meter_levels)
                            .orientation(Orientation::Vertical)
                            .style(self.meter_style)
                            .range(self.min_db, self.max_db)
                            .scale(self.show_scale)
                            .clip(self.show_clip)
                            .readout(false);
                        ui.add(meter);
                    });

                    // Right column: Fader
                    columns[1].vertical_centered(|ui| {
                        if let Some(gain) = self.gain_db.as_deref_mut() {
                            let fader = Fader::new(gain)
                                .orientation(Orientation::Vertical)
                                .range(self.min_db, self.max_db)
                                .taper(Taper::AUDIO)
                                .scale(false)
                                .value_readout(false);
                            let resp = ui.add(fader);
                            if resp.changed() {
                                outer_response.mark_changed();
                            }
                        }
                    });
                });
            },
        );

        ui.add_space(2.0);

        // 4. Numeric dB Readout
        if self.show_readout {
            let peak_val = if is_muted || self.levels.is_empty() {
                -60.0
            } else {
                self.levels
                    .iter()
                    .map(|c| c.peak_db)
                    .fold(-60.0f32, |a, b| a.max(b))
            };
            ui.add(DbReadout::new(peak_val).badge(true).unit(false));
        }

        ui.add_space(4.0);

        // 5. Mute and Solo Toggles at the bottom
        ui.horizontal(|ui| {
            let toggle_w = (ui.available_width() - 4.0) * 0.5;
            let toggle_size = Vec2::new(toggle_w, 20.0);

            if let Some(m) = self.muted.as_deref_mut() {
                let resp = ui.add(ChannelToggle::mute(m).min_size(toggle_size));
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }

            if let Some(s) = self.solo.as_deref_mut() {
                let resp = ui.add(ChannelToggle::solo(s).min_size(toggle_size));
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }
        });
    }

    fn paint_horizontal(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        size: Size,
        outer_response: &mut Response,
    ) {
        let is_muted = self.muted.as_ref().map_or(false, |m| **m);
        let is_dimmed = self.dimmed || is_muted;

        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);

            // 1. Header (number + title)
            let title_color = if is_dimmed {
                theme.palette.text_dim
            } else {
                theme.palette.foreground
            };
            let label_text = if let Some(num) = self.number {
                format!("{num}  {}", self.name)
            } else {
                self.name.to_string()
            };
            let font_id = FontId::new(theme.size.label_font() + 1.0, FontFamily::Proportional);
            ui.allocate_ui_with_layout(
                Vec2::new(70.0, ui.available_height()),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(egui::RichText::new(label_text).font(font_id).color(title_color).strong());
                },
            );

            // 2. Mute / Solo buttons
            if let Some(m) = self.muted.as_deref_mut() {
                let resp = ui.add(ChannelToggle::mute(m).min_size(Vec2::new(22.0, 22.0)));
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }
            if let Some(s) = self.solo.as_deref_mut() {
                let resp = ui.add(ChannelToggle::solo(s).min_size(Vec2::new(22.0, 22.0)));
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }

            ui.add_space(4.0);

            // 3. Pan knob
            if let Some(pan_val) = self.pan.as_deref_mut() {
                let knob_diam = match size {
                    Size::Small => 24.0,
                    Size::Medium => 28.0,
                    Size::Large => 32.0,
                };
                let resp = ui.add(
                    Knob::pan(pan_val)
                        .diameter(knob_diam)
                        .value_readout(false),
                );
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }

            ui.add_space(6.0);

            // 4. Horizontal Level meter
            let meter_levels = if is_muted {
                vec![]
            } else {
                self.levels.to_vec()
            };
            ui.add(
                LevelMeter::new(&meter_levels)
                    .orientation(Orientation::Horizontal)
                    .style(self.meter_style)
                    .range(self.min_db, self.max_db)
                    .clip(self.show_clip)
                    .readout(false),
            );

            ui.add_space(6.0);

            // 5. Horizontal Fader
            if let Some(gain) = self.gain_db.as_deref_mut() {
                let fader = Fader::new(gain)
                    .orientation(Orientation::Horizontal)
                    .range(self.min_db, self.max_db)
                    .taper(Taper::AUDIO)
                    .scale(false)
                    .value_readout(false);
                let resp = ui.add(fader);
                if resp.changed() {
                    outer_response.mark_changed();
                }
            }

            // 6. Readout
            if self.show_readout {
                ui.add_space(4.0);
                let peak_val = if is_muted || self.levels.is_empty() {
                    -60.0
                } else {
                    self.levels
                        .iter()
                        .map(|c| c.peak_db)
                        .fold(-60.0f32, |a, b| a.max(b))
                };
                ui.add(DbReadout::new(peak_val).badge(true).unit(false));
            }
        });
    }
}

impl<'a> Widget for ChannelStrip<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_renders_headlessly() {
        let mut gain = 0.0f32;
        let mut pan = 0.0f32;
        let mut muted = false;
        let mut solo = false;
        let levels = [ChannelLevel::new(-12.0), ChannelLevel::new(-14.0)];

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                ChannelStrip::new("Track 1")
                    .number(1)
                    .gain(&mut gain)
                    .pan(&mut pan)
                    .mute(&mut muted)
                    .solo(&mut solo)
                    .levels(&levels),
            );
        });
    }

    #[test]
    fn strip_horizontal_renders() {
        let mut gain = -6.0f32;
        let mut pan = 0.5f32;
        let mut muted = false;
        let mut solo = false;
        let levels = [ChannelLevel::new(-6.0)];

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(
                ChannelStrip::new("Master")
                    .orientation(Orientation::Horizontal)
                    .variant(ChannelStripVariant::Master)
                    .gain(&mut gain)
                    .pan(&mut pan)
                    .mute(&mut muted)
                    .solo(&mut solo)
                    .levels(&levels),
            );
        });
    }
}
