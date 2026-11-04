//! A mixer container arranging channel strips and a master section with shared settings.
//!
//! Inspired directly by audiocn's `mixer.tsx`. Handles multi-channel layout,
//! solo exclusivity & dimming, master bus routing representation, and keyboard navigation.

use egui::{
    Color32, FontFamily, FontId, Response, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, Widget,
};

use crate::ui::Orientation;
use crate::ui::meters::level_meter::{ChannelLevel, MeterStyle};
use crate::ui::mixer::channel_strip::{ChannelStrip, ChannelStripVariant};
use crate::ui::theme::{Size, Theme};

/// Data representation of a channel within the mixer.
pub struct MixerChannelData<'a> {
    /// Track name (e.g. "Drums", "Bass", "Vocals").
    pub name: &'a str,
    /// Channel track index (1-based).
    pub number: usize,
    /// Optional color accent tag.
    pub accent: Option<Color32>,
    /// Gain parameter in dB.
    pub gain_db: &'a mut f32,
    /// Stereo pan parameter (-1.0..=1.0).
    pub pan: &'a mut f32,
    /// Mute state.
    pub muted: &'a mut bool,
    /// Solo state.
    pub solo: &'a mut bool,
    /// Live meter levels for this channel.
    pub levels: &'a [ChannelLevel],
}

/// Optional master channel representation.
pub struct MixerMasterData<'a> {
    pub name: &'a str,
    pub gain_db: Option<&'a mut f32>,
    pub pan: Option<&'a mut f32>,
    pub muted: Option<&'a mut bool>,
    pub levels: &'a [ChannelLevel],
}

/// A mixer container arranging multiple channels and an optional master strip.
///
/// ```ignore
/// ui.add(
///     Mixer::new()
///         .orientation(Orientation::Vertical)
///         .channels(&mut my_channels)
///         .master(&mut master_data),
/// );
/// ```
#[must_use = "a Mixer draws nothing until it is added to a Ui"]
pub struct Mixer<'a> {
    title: Option<&'a str>,
    orientation: Orientation,
    size: Option<Size>,
    min_db: f32,
    max_db: f32,
    meter_style: MeterStyle,
    scrollable: bool,
    show_scale: bool,
    show_clip: bool,
    show_readout: bool,
    channels: Vec<MixerChannelData<'a>>,
    master: Option<MixerMasterData<'a>>,
}

impl<'a> Default for Mixer<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Mixer<'a> {
    /// Create an empty mixer container.
    pub fn new() -> Self {
        Self {
            title: Some("Mixer"),
            orientation: Orientation::Vertical,
            size: None,
            min_db: -60.0,
            max_db: 6.0,
            meter_style: MeterStyle::Segmented,
            scrollable: true,
            show_scale: false,
            show_clip: true,
            show_readout: true,
            channels: Vec::new(),
            master: None,
        }
    }

    /// Set mixer title (or None to hide).
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Orientation: Vertical console layout or Horizontal track rows.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Size ladder for all strips.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Shared metering range in dB (default -60.0..6.0).
    pub fn range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Shared meter style (Solid, Segmented, LCD).
    pub fn meter_style(mut self, style: MeterStyle) -> Self {
        self.meter_style = style;
        self
    }

    /// Whether the channels area scrolls when overflowing. Default is true.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Add a channel to the mixer.
    pub fn channel(mut self, channel: MixerChannelData<'a>) -> Self {
        self.channels.push(channel);
        self
    }

    /// Add a master output channel.
    pub fn master(mut self, master: MixerMasterData<'a>) -> Self {
        self.master = Some(master);
        self
    }

    /// Add this mixer to `ui`.
    pub fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);

        // Precompute global solo state: if ANY channel is soloed, non-soloed channels are dimmed!
        let any_solo = self.channels.iter().any(|c| *c.solo);

        let initial_avail = ui.available_size();
        let (rect, mut response) = ui.allocate_exact_size(initial_avail, Sense::hover());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let rounding = 8.0;
        ui.painter().rect(
            rect,
            rounding,
            theme.palette.background,
            Stroke::new(1.0_f32, theme.palette.border),
            StrokeKind::Inside,
        );

        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink(8.0))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );

        // 1. Mixer Header
        if let Some(title_text) = self.title {
            child_ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title_text)
                        .font(FontId::new(theme.size.label_font() + 2.0, FontFamily::Proportional))
                        .color(theme.palette.foreground)
                        .strong(),
                );

                if any_solo {
                    ui.add_space(8.0);
                    let clear_btn = ui.small_button("Clear Solos");
                    if clear_btn.clicked() {
                        for c in &mut self.channels {
                            *c.solo = false;
                        }
                        response.mark_changed();
                    }
                }
            });
            child_ui.add_space(6.0);
        }

        // 2. Channels + Master Layout
        let content_avail = child_ui.available_size();

        match self.orientation {
            Orientation::Vertical => {
                // Console layout: Horizontal row of vertical strips + master on right
                child_ui.allocate_ui_with_layout(
                    content_avail,
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        // Channels scroll area
                        let master_reserve_w = if self.master.is_some() { 120.0 } else { 0.0 };
                        let channels_avail_w = (ui.available_width() - master_reserve_w).max(50.0);

                        ScrollArea::horizontal()
                            .auto_shrink([false, false])
                            .max_width(channels_avail_w)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if self.channels.is_empty() {
                                        ui.vertical_centered(|ui| {
                                            ui.add_space(40.0);
                                            ui.label(
                                                egui::RichText::new("No audio channels")
                                                    .color(theme.palette.text_dim),
                                            );
                                        });
                                    } else {
                                        for ch in &mut self.channels {
                                            let is_dimmed = any_solo && !*ch.solo;
                                            let strip = ChannelStrip::new(ch.name)
                                                .number(ch.number)
                                                .orientation(Orientation::Vertical)
                                                .variant(ChannelStripVariant::Card)
                                                .size(size)
                                                .meter_style(self.meter_style)
                                                .range(self.min_db, self.max_db)
                                                .gain(ch.gain_db)
                                                .pan(ch.pan)
                                                .mute(ch.muted)
                                                .solo(ch.solo)
                                                .dimmed(is_dimmed)
                                                .levels(ch.levels)
                                                .scale(self.show_scale)
                                                .clip(self.show_clip)
                                                .readout(self.show_readout);

                                            let strip_resp = ui.add(strip);
                                            if strip_resp.changed() {
                                                response.mark_changed();
                                            }
                                            ui.add_space(4.0);
                                        }
                                    }
                                });
                            });

                        // Separator & Master Section
                        if let Some(mut master) = self.master {
                            ui.separator();
                            ui.add_space(4.0);

                            let mut master_strip = ChannelStrip::new(master.name)
                                .orientation(Orientation::Vertical)
                                .variant(ChannelStripVariant::Master)
                                .size(size)
                                .meter_style(self.meter_style)
                                .range(self.min_db, self.max_db)
                                .levels(master.levels)
                                .scale(true)
                                .clip(self.show_clip)
                                .readout(true);

                            if let Some(gain) = master.gain_db.as_deref_mut() {
                                master_strip = master_strip.gain(gain);
                            }
                            if let Some(pan) = master.pan.as_deref_mut() {
                                master_strip = master_strip.pan(pan);
                            }
                            if let Some(mute) = master.muted.as_deref_mut() {
                                master_strip = master_strip.mute(mute);
                            }

                            let master_resp = ui.add(master_strip);
                            if master_resp.changed() {
                                response.mark_changed();
                            }
                        }
                    },
                );
            }
            Orientation::Horizontal => {
                // DAW track row layout: Vertical stack of horizontal rows + master on bottom
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut child_ui, |ui| {
                        if self.channels.is_empty() {
                            ui.label("No audio channels");
                        } else {
                            for ch in &mut self.channels {
                                let is_dimmed = any_solo && !*ch.solo;
                                let strip = ChannelStrip::new(ch.name)
                                    .number(ch.number)
                                    .orientation(Orientation::Horizontal)
                                    .variant(ChannelStripVariant::Card)
                                    .size(size)
                                    .meter_style(self.meter_style)
                                    .range(self.min_db, self.max_db)
                                    .gain(ch.gain_db)
                                    .pan(ch.pan)
                                    .mute(ch.muted)
                                    .solo(ch.solo)
                                    .dimmed(is_dimmed)
                                    .levels(ch.levels)
                                    .clip(self.show_clip)
                                    .readout(self.show_readout);

                                let strip_resp = ui.add(strip);
                                if strip_resp.changed() {
                                    response.mark_changed();
                                }
                                ui.add_space(4.0);
                            }
                        }

                        if let Some(mut master) = self.master {
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(6.0);

                            let mut master_strip = ChannelStrip::new(master.name)
                                .orientation(Orientation::Horizontal)
                                .variant(ChannelStripVariant::Master)
                                .size(size)
                                .meter_style(self.meter_style)
                                .range(self.min_db, self.max_db)
                                .levels(master.levels)
                                .clip(self.show_clip)
                                .readout(true);

                            if let Some(gain) = master.gain_db.as_deref_mut() {
                                master_strip = master_strip.gain(gain);
                            }
                            if let Some(pan) = master.pan.as_deref_mut() {
                                master_strip = master_strip.pan(pan);
                            }
                            if let Some(mute) = master.muted.as_deref_mut() {
                                master_strip = master_strip.mute(mute);
                            }

                            let master_resp = ui.add(master_strip);
                            if master_resp.changed() {
                                response.mark_changed();
                            }
                        }
                    });
            }
        }

        response
    }
}

impl<'a> Widget for Mixer<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_renders_empty_state() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(Mixer::new());
        });
    }

    #[test]
    fn mixer_vertical_renders_channels_and_master() {
        let mut gain1 = 0.0f32;
        let mut pan1 = 0.0f32;
        let mut mute1 = false;
        let mut solo1 = false;
        let levels1 = [ChannelLevel::new(-12.0)];

        let mut mgain = 0.0f32;
        let mlevels = [ChannelLevel::new(-6.0)];

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mixer = Mixer::new()
                .channel(MixerChannelData {
                    name: "Track 1",
                    number: 1,
                    accent: None,
                    gain_db: &mut gain1,
                    pan: &mut pan1,
                    muted: &mut mute1,
                    solo: &mut solo1,
                    levels: &levels1,
                })
                .master(MixerMasterData {
                    name: "Master",
                    gain_db: Some(&mut mgain),
                    pan: None,
                    muted: None,
                    levels: &mlevels,
                });

            let _ = ui.add(mixer);
        });
    }
}
