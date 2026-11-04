//! A window that shows every NodaAudio component doing its job.
//!
//! ```text
//! cargo run --example showcase
//! ```
//!
//! No audio device is opened. [`Source`] stands in for a `process()` call
//! and hands the widgets the numbers they expect - peak and RMS in dBFS -
//! which is the whole contract a plugin signs up to. Everything on screen
//! is live: move a fader and the meters downstream answer.

use std::f32::consts::FRAC_PI_4;
use std::time::Duration;

use egui::{
    Align, Color32, FontId, Layout, RichText, Response, ScrollArea, Sense, Slider, Ui, Vec2, Widget,
};

use noda_audio::ballistics::{Ballistics, MeterState};
use noda_audio::db::{SILENCE_DB, clamp, db_to_gain, gain_to_db};
use noda_audio::taper::Taper;
use noda_audio::ui::meters::clip_indicator::{ClipLatch, ClipPolicy};
use noda_audio::ui::controls::fader::DEFAULT_DETENTS;
use noda_audio::ui::theme::{Palette, Size, Theme};
use noda_audio::ui::blocks::{
    MicCheckResult, MicSetupBlock, MusicPlayerBlock, QuickAudioPopover, SoundboardBlock,
    SystemAudioMixerBlock,
};
use noda_audio::ui::{
    AbState, AudioDevice, AudioDeviceSelect, AudioPlayer, BarAlign, BarIdle, BarVisualizer,
    BusStrip, ChannelLevel, ClipIndicator, ClipIndicators, CompressorControls, CompressorCurve, DbScale,
    DistortionControls, DragMode, ElectricBarVisualizer, ElectricWaveform, EqBand, EqCurve, Fader,
    FaderStyle, FilterSlope, FilterType, FilterTypeSelector, GainReductionMeter, GateControls,
    HarmonicsSpectrum, Knob, LcdAppearance, LcdDisplay, LcdDisplayRow, LcdMeter, LcdStatus,
    LcdVariant, LevelMeter, LimiterControls, LiveWaveform, LiveWaveformVariant, MarkEdge,
    MasterSection, MetallicKnob, MeterStyle, Mixer, MixerChannelData, MixerMasterData, Orientation,
    Oversampling,
    PanControl, ParameterSlider, PluginHeaderBar, Preset, PresetBrowser, RackFrame, RackStyle,
    Reading, SaturationCurve, SaturationType, SaturationTypeSelect, SaturatorControls, SectionHeader,
    SmoothWaveform, SmoothWaveformMode, SoundPadGrid, SoundPadItem, Spectrum, SpectrumVariant,
    TrackItem, TrackList, VolumeControl, VuAppearance, VuMeter, Waveform, WaveformMarker,
    WaveformRegion, WaveformVariant, format_pan, show_drag_value,
};

/// Height the tall controls get in a gallery row.
const TALL: f32 = 230.0;

/// Height of a standalone dB scale.
const SCALE_HEIGHT: f32 = 170.0;

/// How long the clip demo's overdrive lasts, in seconds.
const BLIP_SECONDS: f32 = 0.25;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1160.0, 820.0])
            .with_min_inner_size([700.0, 520.0])
            .with_title("Noda Audio - component showcase"),
        ..Default::default()
    };
    eframe::run_native(
        "Noda Audio Showcase",
        options,
        Box::new(|cc| Ok(Box::new(Showcase::new(&cc.egui_ctx)))),
    )
}

/// The showcase's stand-in for an audio thread.
///
/// It makes what a `process()` call would measure from a buffer: a peak and
/// an RMS per channel. A pattern with a pulse and a bed gives the meters
/// something to attack and release, which is the only way to show that the
/// ballistics and the peak hold do anything.
struct Source {
    /// Seconds of simulated time.
    t: f32,
    /// Level of the pattern before any strip gain, in dBFS.
    drive_db: f32,
    /// Simulated time the clip demo's overdrive runs until.
    blip_until: f32,
}

impl Source {
    fn new() -> Self {
        Self { t: 0.0, drive_db: -14.0, blip_until: -1.0 }
    }

    /// The instantaneous amplitude of one channel of one strip, 0..1.
    fn shape(&self, strip: usize, channel: usize) -> f32 {
        let beat = (self.t * 2.0 + strip as f32 * 0.12 + channel as f32 * 0.25).fract();
        let pulse = (-beat * 7.0).exp();
        let tick = if (self.t * 4.0 + strip as f32 * 0.5).fract() < 0.08 { 0.45 } else { 0.0 };
        let bed = 0.05 + 0.03 * (self.t * 0.6 + (strip + channel) as f32 * 1.1).sin();
        (pulse * 0.75 + tick + bed).min(1.0)
    }

    /// One channel's peak and RMS after a gain, in dBFS.
    fn level(&self, strip: usize, channel: usize, gain_db: f32) -> ChannelLevel {
        let boost = if self.t < self.blip_until { 14.0 } else { 0.0 };
        let amp = self.shape(strip, channel) * db_to_gain(self.drive_db + gain_db + boost);
        let peak = gain_to_db(amp);
        ChannelLevel::new(peak).rms(peak - 5.0)
    }

    /// A stereo pair, panned with the equal-power law.
    fn stereo(&self, strip: usize, gain_db: f32, balance: f32) -> [ChannelLevel; 2] {
        let (left, right) = equal_power(balance);
        [
            self.level(strip, 0, gain_db + gain_to_db(left)),
            self.level(strip, 1, gain_db + gain_to_db(right)),
        ]
    }
}

/// Equal-power pan: the two sides trade amplitude along a quarter circle, so
/// their squares always sum to unity and -3 dB lands on each at centre.
fn equal_power(balance: f32) -> (f32, f32) {
    let theta = (clamp(balance, -1.0, 1.0) + 1.0) * FRAC_PI_4;
    (theta.cos(), theta.sin())
}

/// Two channels of levels joined as a mixer bus would join them: energy
/// adds, so the sum is the square root of the sum of squares in amplitude.
fn sum(a: [ChannelLevel; 2], b: [ChannelLevel; 2]) -> [ChannelLevel; 2] {
    let join = |x: ChannelLevel, y: ChannelLevel| {
        let peak = db_to_gain(x.peak_db) + db_to_gain(y.peak_db);
        let rms = db_to_gain(x.rms_db.unwrap_or(SILENCE_DB))
            + db_to_gain(y.rms_db.unwrap_or(SILENCE_DB));
        ChannelLevel::new(gain_to_db(peak)).rms(gain_to_db(rms))
    };
    [join(a[0], b[0]), join(a[1], b[1])]
}

/// Apply a post-sum trim to both sides of a stereo pair.
fn trim(levels: [ChannelLevel; 2], gain_db: f32) -> [ChannelLevel; 2] {
    levels.map(|level| {
        ChannelLevel::new(level.peak_db + gain_db)
            .rms(level.rms_db.unwrap_or(SILENCE_DB) + gain_db)
    })
}

/// One lane of the mini mixer: the controls a channel strip is made of.
struct Strip {
    gain_db: f32,
    balance: f32,
    mute: bool,
    solo: bool,
    clip: ClipLatch,
}

impl Strip {
    fn new(gain_db: f32, balance: f32) -> Self {
        Self { gain_db, balance, mute: false, solo: false, clip: ClipLatch::default() }
    }
}

/// The whole window.
struct Showcase {
    theme: Theme,
    light: bool,
    /// Set when the theme has changed and not yet reached the context.
    theme_stale: bool,
    live: bool,
    source: Source,

    style: MeterStyle,
    ballistics: Ballistics,
    show_scale: bool,
    show_readout: bool,
    show_hold: bool,
    show_rms: bool,

    shared_db: f32,
    master_db: f32,
    iso: bool,
    detents: bool,

    freq_hz: f32,
    width_pct: f32,
    pan_only: f32,

    latching: bool,
    decaying: ClipLatch,
    latched: ClipLatch,

    strips: [Strip; 2],

    // Phase 2 components
    param_freq: f32,
    param_decay: f32,
    param_ratio: f32,
    slider_pan: f32,
    media_vol: f32,
    media_mute: bool,
    selected_device: Option<String>,
    mixer_gain: [f32; 3],
    mixer_pan: [f32; 3],
    mixer_mute: [bool; 3],
    mixer_solo: [bool; 3],
    mixer_master_gain: f32,
    mixer_master_mute: bool,

    // Bus strips & master output
    bus_gain: [f32; 2],
    bus_mute: [bool; 2],
    bus_solo: [bool; 2],
    master_out_gain: f32,
    master_out_mute: bool,
    master_out_dim: bool,
    master_out_mono: bool,

    // Phase 3 visualizers
    spectrum_variant: SpectrumVariant,
    spectrum_peaks: [f32; 32],
    waveform_progress: f32,
    vis_loading: bool,
    bar_idle: BarIdle,
    smooth_mode: SmoothWaveformMode,

    // Phase 4 components
    electric_intensity: f32,
    lcd_variant: LcdVariant,
    lcd_appearance: LcdAppearance,
    lcd_holds: [MeterState; 2],
    player_playing: bool,
    player_time: f32,
    player_volume: f32,
    player_muted: bool,
    player_rate: f32,
    player_loop: bool,
    playlist_selected: Option<usize>,
    sound_pads: [SoundPadItem<'static>; 8],

    // Composite blocks state
    mic_device: Option<String>,
    mic_gain: f32,
    mic_muted: bool,
    mic_active: bool,
    mic_check: Option<MicCheckResult>,

    block_music_playing: bool,
    block_music_time: f32,
    block_music_vol: f32,
    block_music_muted: bool,
    block_music_rate: f32,
    block_music_loop: bool,
    block_music_track: Option<usize>,
    block_music_show_spec: bool,

    block_sound_vol: f32,
    block_sound_muted: bool,
    block_sound_pads: [SoundPadItem<'static>; 8],

    block_popover_dev: Option<String>,
    block_popover_vol: f32,
    block_popover_muted: bool,

    block_sys_orientation: Orientation,
    block_sys_gain: [f32; 3],
    block_sys_pan: [f32; 3],
    block_sys_mute: [bool; 3],
    block_sys_solo: [bool; 3],
    block_sys_master_gain: f32,
    block_sys_master_mute: bool,

    // Metallic Volume Dial & Mini Metal knobs
    metallic_vol: f32,
    mini_gain: f32,
    mini_pan: f32,
    mini_mix: f32,

    // EQ and Filter
    eq_bands: [EqBand; 4],
    selected_eq_band: Option<usize>,

    // Pro Dynamics & Meters
    vu_reading: f32,
    gr_reading: f32,
    comp_threshold: f32,
    comp_ratio: f32,
    comp_attack: f32,
    comp_release: f32,
    comp_knee: f32,
    comp_makeup: f32,
    comp_sc_hz: f32,
    comp_sc_active: bool,
    dynamics_tab: usize,
    limiter_ceiling: f32,
    limiter_release: f32,
    limiter_true_peak: bool,
    gate_threshold: f32,
    gate_range: f32,
    gate_attack: f32,
    gate_hold: f32,
    gate_release: f32,
    gate_hysteresis: f32,

    // Saturation & Distortion
    sat_type: SaturationType,
    sat_drive: f32,
    sat_mix: f32,
    sat_tone: f32,
    sat_bias: f32,
    dist_drive: f32,
    dist_type: SaturationType,
    dist_tone: f32,
    dist_output: f32,

    // Pro Plugin Framework & Rack
    plugin_bypassed: bool,
    plugin_ab: AbState,
    plugin_oversampling: Oversampling,
    plugin_in_gain: f32,
    plugin_out_gain: f32,
    plugin_preset_idx: usize,
    plugin_presets: Vec<Preset>,
    rack_style: RackStyle,
    rack_section_eq: bool,
    rack_section_comp: bool,
    rack_section_sat: bool,
}

impl Showcase {
    fn new(ctx: &egui::Context) -> Self {
        let theme = Theme::dark();
        theme.install(ctx);
        Self {
            theme,
            light: false,
            theme_stale: false,
            live: true,
            source: Source::new(),
            style: MeterStyle::Gradient,
            ballistics: Ballistics::PEAK,
            show_scale: true,
            show_readout: true,
            show_hold: true,
            show_rms: true,
            shared_db: -18.0,
            master_db: -6.0,
            iso: true,
            detents: true,
            freq_hz: 1200.0,
            width_pct: 35.0,
            pan_only: -0.25,
            latching: false,
            decaying: ClipLatch::default(),
            latched: ClipLatch::default(),
            strips: [Strip::new(0.0, -0.3), Strip::new(-12.0, 0.45)],

            param_freq: 1200.0,
            param_decay: 250.0,
            param_ratio: 45.0,
            slider_pan: 0.0,
            media_vol: 0.75,
            media_mute: false,
            selected_device: Some("built_in".to_string()),
            mixer_gain: [-2.0, -6.0, -10.0],
            mixer_pan: [-0.25, 0.35, 0.0],
            mixer_mute: [false, false, false],
            mixer_solo: [false, false, false],
            mixer_master_gain: -3.0,
            mixer_master_mute: false,

            bus_gain: [-4.0, -8.0],
            bus_mute: [false, false],
            bus_solo: [false, false],
            master_out_gain: -3.0,
            master_out_mute: false,
            master_out_dim: false,
            master_out_mono: false,

            spectrum_variant: SpectrumVariant::Area,
            spectrum_peaks: [0.0; 32],
            waveform_progress: 0.35,
            vis_loading: false,
            bar_idle: BarIdle::Wave,
            smooth_mode: SmoothWaveformMode::Wave,

            electric_intensity: 0.6,
            lcd_variant: LcdVariant::Amber,
            lcd_appearance: LcdAppearance::Auto,
            lcd_holds: Default::default(),
            player_playing: false,
            player_time: 24.5,
            player_volume: 0.85,
            player_muted: false,
            player_rate: 1.0,
            player_loop: false,
            playlist_selected: Some(1),
            sound_pads: [
                SoundPadItem::new(1, "Kick").hotkey("1").color(Color32::from_rgb(239, 68, 68)),
                SoundPadItem::new(2, "Snare").hotkey("2").color(Color32::from_rgb(59, 130, 246)),
                SoundPadItem::new(3, "Hi-Hat").hotkey("3").color(Color32::from_rgb(16, 185, 129)),
                SoundPadItem::new(4, "Clap").hotkey("4").color(Color32::from_rgb(245, 158, 11)),
                SoundPadItem::new(5, "Tom Low").hotkey("Q").color(Color32::from_rgb(168, 85, 247)),
                SoundPadItem::new(6, "Tom High").hotkey("W").color(Color32::from_rgb(236, 72, 153)),
                SoundPadItem::new(7, "Crash").hotkey("E").color(Color32::from_rgb(6, 182, 212)),
                SoundPadItem::new(8, "Ride").hotkey("R").color(Color32::from_rgb(234, 179, 8)),
            ],

            mic_device: Some("mic_builtin".to_string()),
            mic_gain: 0.0,
            mic_muted: false,
            mic_active: true,
            mic_check: Some(MicCheckResult::Good),

            block_music_playing: false,
            block_music_time: 42.0,
            block_music_vol: 0.8,
            block_music_muted: false,
            block_music_rate: 1.0,
            block_music_loop: false,
            block_music_track: Some(0),
            block_music_show_spec: true,

            block_sound_vol: 0.85,
            block_sound_muted: false,
            block_sound_pads: [
                SoundPadItem::new(1, "Kick").hotkey("1").color(Color32::from_rgb(239, 68, 68)),
                SoundPadItem::new(2, "Snare").hotkey("2").color(Color32::from_rgb(59, 130, 246)),
                SoundPadItem::new(3, "Hi-Hat").hotkey("3").color(Color32::from_rgb(16, 185, 129)),
                SoundPadItem::new(4, "Clap").hotkey("4").color(Color32::from_rgb(245, 158, 11)),
                SoundPadItem::new(5, "Tom Low").hotkey("Q").color(Color32::from_rgb(168, 85, 247)),
                SoundPadItem::new(6, "Tom High").hotkey("W").color(Color32::from_rgb(236, 72, 153)),
                SoundPadItem::new(7, "Crash").hotkey("E").color(Color32::from_rgb(6, 182, 212)),
                SoundPadItem::new(8, "Ride").hotkey("R").color(Color32::from_rgb(234, 179, 8)),
            ],

            block_popover_dev: Some("sys_speakers".to_string()),
            block_popover_vol: 0.72,
            block_popover_muted: false,

            block_sys_orientation: Orientation::Vertical,
            block_sys_gain: [-3.0, -6.0, -12.0],
            block_sys_pan: [-0.2, 0.2, 0.0],
            block_sys_mute: [false, false, false],
            block_sys_solo: [false, false, false],
            block_sys_master_gain: -1.5,
            block_sys_master_mute: false,

            metallic_vol: 33.0,
            mini_gain: 0.0,
            mini_pan: 0.0,
            mini_mix: 65.0,

            eq_bands: [
                EqBand::new(1, FilterType::LowCut, 35.0, 0.0, 0.707).slope(FilterSlope::Db18),
                EqBand::new(2, FilterType::LowShelf, 120.0, 3.0, 0.707),
                EqBand::new(3, FilterType::Bell, 1250.0, -2.5, 1.41),
                EqBand::new(4, FilterType::HighShelf, 9000.0, 3.5, 0.707),
            ],
            selected_eq_band: Some(2),

            vu_reading: -4.0,
            gr_reading: -6.0,
            comp_threshold: -18.0,
            comp_ratio: 4.0,
            comp_attack: 15.0,
            comp_release: 120.0,
            comp_knee: 6.0,
            comp_makeup: 2.5,
            comp_sc_hz: 100.0,
            comp_sc_active: true,
            dynamics_tab: 0,
            limiter_ceiling: -0.5,
            limiter_release: 50.0,
            limiter_true_peak: true,
            gate_threshold: -36.0,
            gate_range: -40.0,
            gate_attack: 2.0,
            gate_hold: 40.0,
            gate_release: 140.0,
            gate_hysteresis: 3.0,

            sat_type: SaturationType::Tube,
            sat_drive: 3.2,
            sat_mix: 100.0,
            sat_tone: 0.0,
            sat_bias: 0.05,
            dist_drive: 4.8,
            dist_type: SaturationType::HardClip,
            dist_tone: 0.25,
            dist_output: -3.0,

            plugin_bypassed: false,
            plugin_ab: AbState::A,
            plugin_oversampling: Oversampling::X4,
            plugin_in_gain: 0.0,
            plugin_out_gain: -1.0,
            plugin_preset_idx: 0,
            plugin_presets: vec![
                Preset::new("Mastering", "Warm Analog Glue").author("Noda Master"),
                Preset::new("Vocals", "Air & Silk Opto").author("Noda Vox"),
                Preset::new("Drums", "Punchy Parallel Smash").author("Noda Beats"),
                Preset::new("Bass", "Sub Heavy Saturation").author("Noda Low"),
            ],
            rack_style: RackStyle::DarkAluminium,
            rack_section_eq: true,
            rack_section_comp: true,
            rack_section_sat: true,
        }
    }

    /// Swap the palette, keep the rest of the theme.
    fn set_light(&mut self, light: bool) {
        self.light = light;
        self.theme.palette = if light { Palette::light() } else { Palette::dark() };
        self.theme_stale = true;
    }

    fn header(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette.clone();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Noda Audio")
                    .font(FontId::proportional(self.theme.size.value_font() + 4.0))
                    .color(palette.primary),
            );
            ui.label(RichText::new("component showcase").color(palette.text_dim));
            ui.separator();
            for (name, size) in [("S", Size::Small), ("M", Size::Medium), ("L", Size::Large)] {
                if ui.selectable_label(self.theme.size == size, name).clicked() {
                    self.theme.size = size;
                    self.theme_stale = true;
                }
            }
            ui.separator();
            if ui.selectable_label(!self.light, "Dark").clicked() {
                self.set_light(false);
            }
            if ui.selectable_label(self.light, "Light").clicked() {
                self.set_light(true);
            }
            ui.separator();
            ui.checkbox(&mut self.live, "Live");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button("Blip")
                    .on_hover_text("Push the source over full scale for a quarter second")
                    .clicked()
                {
                    self.source.blip_until = self.source.t + BLIP_SECONDS;
                }
                ui.add(
                    Slider::new(&mut self.source.drive_db, -60.0..=0.0)
                        .text("Drive")
                        .suffix(" dB"),
                );
            });
        });
    }

    /// Meters, with every switch the widget exposes within reach.
    fn meters(&mut self, ui: &mut Ui) {
        let theme = Theme::of(ui);
        section(
            ui,
            "Level meters",
            "Peak, RMS marker, hold line, clip lamp, scale, readout.",
            |ui| {

            let pair = self.source.stereo(0, 0.0, 0.0);
            let four = [
                pair[0],
                pair[1],
                self.source.level(1, 0, -6.0),
                self.source.level(1, 1, -6.0),
            ];

            ui.horizontal(|ui| {
                let meter = LevelMeter::stereo(&pair)
                    .style(self.style)
                    .ballistics(self.ballistics)
                    .scale(self.show_scale)
                    .readout(self.show_readout)
                    .hold(self.show_hold)
                    .rms_marker(self.show_rms)
                    .clip(true)
                    .id(egui::Id::new("showcase-stereo"));
                at_height(ui, meter.desired_size(&theme), TALL, meter);

                let bank = LevelMeter::new(&four)
                    .style(self.style)
                    .ballistics(self.ballistics)
                    .thickness(14.0)
                    .channel_gap(4.0)
                    .clip(true)
                    .id(egui::Id::new("showcase-bank"));
                at_height(ui, bank.desired_size(&theme), TALL, bank);

                let meter = LevelMeter::stereo(&pair)
                    .style(MeterStyle::Segmented)
                    .ballistics(self.ballistics)
                    .segments(5.0, 3.0)
                    .scale(true)
                    .id(egui::Id::new("showcase-segments"));
                at_height(ui, meter.desired_size(&theme), TALL, meter);

                ui.separator();
                self.switches(ui);
            });

            ui.add_space(8.0);
            let sideways = self.source.stereo(1, 0.0, 0.0);
            let meter = LevelMeter::stereo(&sideways)
                .orientation(Orientation::Horizontal)
                .style(self.style)
                .ballistics(self.ballistics)
                .scale(true)
                .readout(true)
                .id(egui::Id::new("showcase-horizontal"));
            let width = ui.available_width();
            across(ui, meter.desired_size(&theme), width, meter);
        });
    }

    fn switches(&mut self, ui: &mut Ui) {
        let (label, dim) = self.caption();
        ui.vertical(|ui| {
            ui.label("Fill");
            ui.horizontal(|ui| {
                for (name, style) in [
                    ("Solid", MeterStyle::Solid),
                    ("Segments", MeterStyle::Segmented),
                    ("Gradient", MeterStyle::Gradient),
                ] {
                    if ui.selectable_label(self.style == style, name).clicked() {
                        self.style = style;
                    }
                }
            });
            ui.label("Ballistics");
            ui.horizontal(|ui| {
                for (name, constants) in [
                    ("Peak", Ballistics::PEAK),
                    ("VU", Ballistics::VU),
                    ("Instant", Ballistics::INSTANT),
                ] {
                    if ui.selectable_label(self.ballistics == constants, name).clicked() {
                        self.ballistics = constants;
                    }
                }
            });
            ui.checkbox(&mut self.show_scale, "Scale");
            ui.checkbox(&mut self.show_readout, "Readout");
            ui.checkbox(&mut self.show_hold, "Peak hold");
            ui.checkbox(&mut self.show_rms, "RMS marker");
            ui.label(
                RichText::new(
                    "Channel count, thickness and segment length are call-site choices: \
                     a bank of four and a stereo pair are the same widget.",
                )
                .size(label)
                .color(dim),
            );
        });
    }

    /// Three faders over one value, which is the only way to see what a
    /// taper does to the same number.
    fn faders(&mut self, ui: &mut Ui) {
        let theme = Theme::of(ui);
        section(
            ui,
            "Faders",
            "One value, three laws - the handle sits where its law puts it.",
            |ui| {

            let mut laws = [self.shared_db; 3];
            let no_detents: &[f32] = &[];
            ui.horizontal(|ui| {
                for (index, (name, taper)) in [
                    ("Console", Taper::AUDIO),
                    ("Half unity", Taper::Audio { unity_position: 0.5 }),
                    ("Linear", Taper::Linear),
                ]
                .into_iter()
                .enumerate()
                {
                    let fader = Fader::new(&mut laws[index])
                        .range(-60.0, 12.0)
                        .taper(taper)
                        .style(if self.iso { FaderStyle::Iso } else { FaderStyle::Plain })
                        .detents(if self.detents { &DEFAULT_DETENTS } else { no_detents })
                        .scale(true)
                        .value_readout(true)
                        .id(egui::Id::new(format!("showcase-fader-{name}")));
                    at_height(ui, fader.desired_size(&theme), TALL, fader);
                }

                // The three handles share one value, so whichever moved is
                // the new value.
                if let Some(moved) = laws.iter().copied().find(|value| *value != self.shared_db)
                {
                    self.shared_db = moved;
                }

                ui.separator();
                let (label, dim) = self.caption();
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.iso, "Iso fill");
                    ui.checkbox(&mut self.detents, "Detents");
                    ui.label(
                        RichText::new(
                            "Drag any handle: the other two follow to the same decibels at \
                             different heights. That gap is the taper.",
                        )
                        .size(label)
                        .color(dim),
                    );
                    ui.label(
                        RichText::new(
                            "Type into the readout, double-click to reset, arrows to nudge, \
                             Home and End for the ends.",
                        )
                        .size(label)
                        .color(dim),
                    );
                });
            });

            ui.add_space(10.0);
            let mut master = self.master_db;
            let fader = Fader::new(&mut master)
                .range(-60.0, 0.0)
                .taper(Taper::AUDIO)
                .orientation(Orientation::Horizontal)
                .drag(DragMode::Relative)
                .scale(true)
                .value_readout(true)
                .id(egui::Id::new("showcase-master"));
            let width = ui.available_width();
            if across(ui, fader.desired_size(&theme), width, fader).changed() {
                self.master_db = master;
            }
        });
    }

    /// Knobs, one per kind of value a plugin hands out.
    fn knobs(&mut self, ui: &mut Ui) {
        let theme = Theme::of(ui);
        section(
            ui,
            "Knobs",
            "A rotary control reads in its own units and types back the same way.",
            |ui| {

            ui.horizontal(|ui| {
                let knob = Knob::pan(&mut self.pan_only)
                    .label("Balance")
                    .reset(Some(0.0))
                    .id(egui::Id::new("showcase-pan"));
                let want = knob.desired_size(&theme);
                fill(ui, want, knob);

                let mut send = self.shared_db;
                let knob = Knob::new(&mut send)
                    .range(-60.0, 12.0)
                    .taper(Taper::AUDIO)
                    .reading(Reading::Db)
                    .label("Send")
                    .reset(Some(-18.0))
                    .id(egui::Id::new("showcase-send"));
                let want = knob.desired_size(&theme);
                if fill(ui, want, knob).changed() {
                    self.shared_db = send;
                }

                let knob = Knob::new(&mut self.freq_hz)
                    .range(20.0, 20_000.0)
                    .taper(Taper::Log)
                    .reading(Reading::Number { decimals: 1, unit: Some("Hz") })
                    .label("Freq")
                    .sweep_degrees(240.0)
                    .drag(DragMode::Grab)
                    .reset(Some(1000.0))
                    .id(egui::Id::new("showcase-freq"));
                let want = knob.desired_size(&theme);
                fill(ui, want, knob);

                let knob = Knob::new(&mut self.width_pct)
                    .range(0.0, 100.0)
                    .reading(Reading::Number { decimals: 0, unit: Some("%") })
                    .label("Width")
                    .scale(false)
                    .drag(DragMode::Relative)
                    .throw(90.0)
                    .id(egui::Id::new("showcase-width"));
                let want = knob.desired_size(&theme);
                fill(ui, want, knob);

                let mut compact = self.width_pct;
                let knob = Knob::new(&mut compact)
                    .range(0.0, 100.0)
                    .label("Compact")
                    .scale(false)
                    .value_readout(false)
                    .diameter(30.0)
                    .id(egui::Id::new("showcase-compact"));
                let want = knob.desired_size(&theme);
                if fill(ui, want, knob).changed() {
                    self.width_pct = compact;
                }

                ui.separator();
                let (label, dim) = self.caption();
                let (left, right) = equal_power(self.pan_only);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(
                            "Grab follows the pointer around the dial; Relative moves by \
                             pointer travel, so a long throw never jumps. The wheel trims \
                             and a double-click resets.",
                        )
                        .size(label)
                        .color(dim),
                    );
                    ui.label(format!(
                        "Balance {}  ·  L {left:.2}  R {right:.2}",
                        format_pan(self.pan_only)
                    ));
                });
            });
        });
    }

    /// High-fidelity metallic volume dial and mini aluminium knob controls.
    fn metallic_knobs(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Metallic Volume Dial & Metal Knobs",
            "Audiocn-inspired brushed aluminium anisotropic conic sheen, 100-tick radial scale with rotated numbers, and mini dense knobs.",
            |ui| {
                ui.horizontal_top(|ui| {
                    // 1. Full Hi-Fi Metallic Volume Dial (as in audiocn / reference image)
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Hi-Fi Master Volume Dial (0..100)").strong());
                        ui.add_space(6.0);
                        ui.add(
                            MetallicKnob::volume(&mut self.metallic_vol)
                                .diameter(168.0),
                        );
                    });

                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(20.0);

                    // 2. Mini Metal Knobs for dense channel strips
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Mini Metal Knobs (Dense Channel Strips & Plugins)").strong());
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label("Gain");
                                ui.add(MetallicKnob::mini(&mut self.mini_gain, -24.0, 24.0).diameter(54.0).bipolar(true));
                                ui.label(format!("{:.1} dB", self.mini_gain));
                            });
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.label("Pan");
                                ui.add(MetallicKnob::mini(&mut self.mini_pan, -1.0, 1.0).diameter(54.0).bipolar(true));
                                ui.label(format_pan(self.mini_pan));
                            });
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.label("Mix");
                                ui.add(MetallicKnob::mini(&mut self.mini_mix, 0.0, 100.0).diameter(54.0));
                                ui.label(format!("{:.0}%", self.mini_mix));
                            });
                        });
                        ui.add_space(12.0);
                        let (label, dim) = self.caption();
                        ui.label(
                            RichText::new(
                                "Anisotropic GPU triangle-fan conic sheen with lathe-turned \
                                 circular micro-grooves, dark bezel with specular light from above, \
                                 recessed dot indicator, and active bright white tick/label illumination.",
                            )
                            .size(label)
                            .color(dim),
                        );
                    });
                });
            },
        );
    }

    /// The ruler on its own, which is what a meter or fader hangs its bar on.
    fn scales(&mut self, ui: &mut Ui) {
        let theme = Theme::of(ui);
        section(
            ui,
            "dB scale",
            "The ruler a meter or fader measures against, drawn on its own.",
            |ui| {

            let dense: [f32; 7] = [-60.0, -48.0, -24.0, -12.0, -6.0, 0.0, 12.0];
            ui.horizontal(|ui| {
                for (name, scale) in [
                    ("Linear, default ticks", DbScale::vertical().taper(Taper::Linear)),
                    ("Console law", DbScale::vertical().taper(Taper::AUDIO).range(-60.0, 12.0)),
                    (
                        "Marks only, other edge",
                        DbScale::vertical()
                            .taper(Taper::AUDIO)
                            .range(-60.0, 12.0)
                            .labels(false)
                            .mark_edge(MarkEdge::Start),
                    ),
                    (
                        "Named ticks",
                        DbScale::vertical().range(-60.0, 12.0).ticks(&dense).mark_lengths(9.0, 4.0),
                    ),
                ] {
                    ui.vertical(|ui| {
                        let thickness = scale.desired_thickness(&theme);
                        let size = Vec2::new(thickness + 8.0, SCALE_HEIGHT);
                        let (rect, _) = ui.allocate_at_least(size, Sense::hover());
                        scale.paint(ui, rect);
                        ui.label(name);
                    });
                }
            });

            ui.add_space(6.0);
            let scale = DbScale::horizontal().range(-60.0, 12.0).taper(Taper::AUDIO);
            let thickness = scale.desired_thickness(&theme);
            let width = ui.available_width();
            let (rect, _) = ui.allocate_at_least(Vec2::new(width, thickness + 6.0), Sense::hover());
            scale.paint(ui, rect);
        });
    }

    /// The clip light and the latch that decides when it is on.
    fn clips(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Clip light",
            "A latch, not a blink: a one-sample hit is still an event.",
            |ui| {

            let now_ms = ui.input(|input| input.time) as f32 * 1000.0;
            let peak = self.source.level(0, 0, 0.0).peak_db;
            let policy = if self.latching { ClipPolicy::latching() } else { ClipPolicy::default() };
            let lit_now = self.latched.report(peak, now_ms, policy);
            let decays = self.decaying.report(peak, now_ms, ClipPolicy::default());

            let (label, dim) = self.caption();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Decaying");
                    if ClipIndicator::new().latch(&self.decaying).dot_size(12.0).ui(ui).clicked() {
                        self.decaying.reset();
                    }
                });
                ui.vertical(|ui| {
                    ui.label("Latching");
                    if ClipIndicator::new().latch(&self.latched).dot_size(12.0).ui(ui).clicked() {
                        self.latched.reset();
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.latching, "Latch until reset");
                    ui.label(RichText::new(if lit_now {
                        "Light is on: the peak is at or above -1 dBFS"
                    } else {
                        "Light is off: the peak is under the threshold"
                    })
                    .size(label)
                    .color(dim));
                    ui.label(
                        RichText::new(format!(
                            "Decaying light: {}. Click a dot to clear its count.",
                            if decays { "on" } else { "off" }
                        ))
                        .size(label)
                        .color(dim),
                    );
                });
            });
        });
    }

    /// The components as a mixer: two strips and a master, driven by one
    /// signal through a real gain and pan chain.
    fn mixer(&mut self, ui: &mut Ui) {
        let theme = Theme::of(ui);
        section(
            ui,
            "Composed: strips and a master",
            "Meters, fader, knob and lamp laid out as one console lane.",
            |ui| {

            let now_ms = ui.input(|input| input.time) as f32 * 1000.0;
            let any_solo = self.strips.iter().any(|strip| strip.solo);
            let active = |strip: &Strip| {
                if strip.mute || (any_solo && !strip.solo) { SILENCE_DB } else { strip.gain_db }
            };

            // Numbers for the whole row, taken before anything is drawn, so
            // what the meters show matches what the controls are set to.
            let inputs = [self.source.stereo(0, 0.0, 0.0), self.source.stereo(1, 0.0, 0.0)];
            let outputs = [
                self.source.stereo(0, active(&self.strips[0]), self.strips[0].balance),
                self.source.stereo(1, active(&self.strips[1]), self.strips[1].balance),
            ];
            let master = sum(outputs[0], outputs[1]);
            let muted = [self.strips[0].mute, self.strips[1].mute];

            // The latches are fed here rather than where they are drawn, so
            // the caption and the lamp agree on the same frame.
            let mut lit = [false; 2];
            for (index, strip) in self.strips.iter_mut().enumerate() {
                let peak = outputs[index][0].peak_db.max(outputs[index][1].peak_db);
                lit[index] = strip.clip.report(peak, now_ms, ClipPolicy::default());
            }

            ui.horizontal(|ui| {
                for index in 0..2 {
                    let name = format!("STRIP {}", index + 1);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            if lit[index] {
                                ui.colored_label(theme.palette.meter_clip, "CLIP");
                            }
                        });
                        ui.horizontal(|ui| {
                            let meter = LevelMeter::stereo(&inputs[index])
                                .style(self.style)
                                .thickness(10.0)
                                .id(egui::Id::new(format!("showcase-strip-{index}-in")));
                            at_height(ui, meter.desired_size(&theme), TALL, meter);

                            let strip = &mut self.strips[index];
                            let fader = Fader::new(&mut strip.gain_db)
                                .range(-60.0, 12.0)
                                .taper(Taper::AUDIO)
                                .reset_db(Some(0.0))
                                .scale(index == 0)
                                .value_readout(true)
                                .id(egui::Id::new(format!("showcase-strip-{index}-gain")));
                            at_height(ui, fader.desired_size(&theme), TALL, fader);

                            let knob = Knob::pan(&mut strip.balance)
                                .label("Pan")
                                .diameter(38.0)
                                .id(egui::Id::new(format!("showcase-strip-{index}-pan")));
                            let want = knob.desired_size(&theme);
                            fill(ui, want, knob);

                            let meter = LevelMeter::stereo(&outputs[index])
                                .style(self.style)
                                .thickness(10.0)
                                .hold(true)
                                .clip(true)
                                .id(egui::Id::new(format!("showcase-strip-{index}-out")));
                            at_height(ui, meter.desired_size(&theme), TALL, meter);
                        });
                        ui.horizontal(|ui| {
                            let strip = &mut self.strips[index];
                            let mute = ui.selectable_label(strip.mute, "Mute");
                            if mute.clicked() {
                                strip.mute = !strip.mute;
                            }
                            let solo = ui.selectable_label(strip.solo, "Solo");
                            if solo.clicked() {
                                strip.solo = !strip.solo;
                            }
                            ClipIndicator::new().latch(&strip.clip).dot_size(10.0).ui(ui);
                        });
                    });
                    if index == 0 {
                        ui.separator();
                    }
                }

                ui.separator();
                ui.vertical(|ui| {
                    ui.label("MASTER");
                    let meter = LevelMeter::stereo(&master)
                        .style(self.style)
                        .scale(true)
                        .readout(true)
                        .hold(true)
                        .rms_marker(true)
                        .clip(true)
                        .thickness(22.0)
                        .id(egui::Id::new("showcase-master-meter"));
                    at_height(ui, meter.desired_size(&theme), TALL, meter);
                    let (label, dim) = self.caption();
                    ui.label(
                        RichText::new(if muted.iter().any(|muted| *muted) {
                            "A muted strip is silent at the master: the widgets draw, but \
                             the signal chain stops."
                        } else {
                            "The master is the two strips' outputs summed, after gain and pan."
                        })
                        .size(label)
                        .color(dim),
                    );
                });
            });
        });
    }

    /// Parameter sliders, pan balance, volume control, and device picker.
    fn parameter_controls(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Parameter controls",
            "Horizontal sliders for non-volume parameters, stereo balance, media volume, and audio devices.",
            |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("ParameterSlider").strong());
                        ui.add(
                            ParameterSlider::new(&mut self.param_freq, 20.0, 20000.0)
                                .label("Cutoff")
                                .unit("Hz")
                                .taper(Taper::Log)
                                .decimals(0)
                                .width(220.0),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            ParameterSlider::new(&mut self.param_decay, 10.0, 5000.0)
                                .label("Decay")
                                .unit("ms")
                                .taper(Taper::Log)
                                .decimals(0)
                                .width(220.0),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            ParameterSlider::new(&mut self.param_ratio, 0.0, 100.0)
                                .label("Dry / Wet")
                                .unit("%")
                                .decimals(1)
                                .width(220.0),
                        );
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label(RichText::new("PanControl & VolumeControl").strong());
                        ui.add(
                            PanControl::new(&mut self.slider_pan)
                                .label("Pan Balance")
                                .width(220.0),
                        );
                        ui.add_space(8.0);
                        ui.add(
                            VolumeControl::new(&mut self.media_vol)
                                .mute(&mut self.media_mute)
                                .show_readout(true)
                                .width(220.0),
                        );
                        ui.add_space(8.0);
                        let devices = [
                            AudioDevice::new("built_in", "MacBook Pro Speakers").default_device(true),
                            AudioDevice::new("interface", "Focusrite Scarlett 4i4 USB"),
                            AudioDevice::new("airpods", "AirPods Pro (Bluetooth)").available(false),
                        ];
                        ui.add(AudioDeviceSelect::new(&mut self.selected_device, &devices).width(240.0));
                    });
                });
            },
        );
    }

    /// Full mixer container widget with solo exclusivity and master bus.
    fn full_mixer(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Mixer Container",
            "Audiocn-style full mixer container with solo exclusivity, master section, and mute dimming.",
            |ui| {
                let p1 = self.source.stereo(0, self.mixer_gain[0], self.mixer_pan[0]);
                let p2 = self.source.stereo(1, self.mixer_gain[1], self.mixer_pan[1]);
                let p3 = self.source.stereo(0, self.mixer_gain[2], self.mixer_pan[2]);
                let master_levels = sum(sum(p1, p2), p3);

                let [g0, g1, g2] = &mut self.mixer_gain;
                let [pan0, pan1, pan2] = &mut self.mixer_pan;
                let [m0, m1, m2] = &mut self.mixer_mute;
                let [s0, s1, s2] = &mut self.mixer_solo;

                let ch1 = MixerChannelData {
                    name: "Drums",
                    number: 1,
                    accent: Some(egui::Color32::from_rgb(239, 68, 68)),
                    gain_db: g0,
                    pan: pan0,
                    muted: m0,
                    solo: s0,
                    levels: &p1,
                };
                let ch2 = MixerChannelData {
                    name: "Bass",
                    number: 2,
                    accent: Some(egui::Color32::from_rgb(59, 130, 246)),
                    gain_db: g1,
                    pan: pan1,
                    muted: m1,
                    solo: s1,
                    levels: &p2,
                };
                let ch3 = MixerChannelData {
                    name: "Keys",
                    number: 3,
                    accent: Some(egui::Color32::from_rgb(16, 185, 129)),
                    gain_db: g2,
                    pan: pan2,
                    muted: m2,
                    solo: s2,
                    levels: &p3,
                };

                let master = MixerMasterData {
                    name: "Master Out",
                    gain_db: Some(&mut self.mixer_master_gain),
                    pan: None,
                    muted: Some(&mut self.mixer_master_mute),
                    levels: &master_levels,
                };

                Mixer::new()
                    .channel(ch1)
                    .channel(ch2)
                    .channel(ch3)
                    .master(master)
                    .ui(ui);
            },
        );
    }

    /// Group bus rows above the console's master output.
    fn bus_master_section(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Bus Strips & Master Output",
            "Two group buses fed from the channel gains, each with trim, mute/solo and a stereo meter, above a wide master output with fader, scaled meter and MUTE/DIM/MONO.",
            |ui| {
                let drums = self
                    .source
                    .stereo(0, self.mixer_gain[0] + self.bus_gain[0], self.mixer_pan[0]);
                let tonal = sum(
                    self.source
                        .stereo(1, self.mixer_gain[1] + self.bus_gain[1], self.mixer_pan[1]),
                    self.source
                        .stereo(2, self.mixer_gain[2] + self.bus_gain[1], self.mixer_pan[2]),
                );
                let master_levels = trim(sum(drums, tonal), self.master_out_gain);

                let width = ui.available_width().clamp(340.0, 720.0);

                ui.add(
                    BusStrip::new("DRUM BUS", &mut self.bus_gain[0])
                        .members(3)
                        .route("Master")
                        .accent(self.theme.palette.chart[0])
                        .mute(&mut self.bus_mute[0])
                        .solo(&mut self.bus_solo[0])
                        .levels(&drums)
                        .width(width),
                );
                ui.add_space(8.0);
                ui.add(
                    BusStrip::new("TONAL BUS", &mut self.bus_gain[1])
                        .members(2)
                        .route("Master")
                        .accent(self.theme.palette.chart[1])
                        .mute(&mut self.bus_mute[1])
                        .solo(&mut self.bus_solo[1])
                        .levels(&tonal)
                        .width(width),
                );
                ui.add_space(12.0);
                ui.add(
                    MasterSection::new(&mut self.master_out_gain)
                        .title("Master Output")
                        .subtitle("Main speakers · Stereo")
                        .mute(&mut self.master_out_mute)
                        .dim(&mut self.master_out_dim)
                        .mono(&mut self.master_out_mono)
                        .accent(self.theme.palette.primary)
                        .levels(&master_levels)
                        .width(width),
                );
            },
        );
    }

    /// Realtime spectrum, reactive bar visualizer, smooth wave/scope, and clip waveform.
    fn visualizers(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Visualizers",
            "Realtime spectrum analysis, reactive bar visualizers, fluid smooth waveforms, and seekable clips.",
            |ui| {
                let t = self.source.t;

                // Synthesize 32 frequency band levels from live source
                let bands: Vec<f32> = (0..32)
                    .map(|i| {
                        let fi = i as f32 / 32.0;
                        let s1 = (t * 3.5 + fi * 6.0).sin().abs() * 0.7;
                        let s2 = (t * 1.2 - fi * 3.0).cos().abs() * 0.3;
                        let s3 = (t * 8.0 + fi * 15.0).sin().abs() * 0.2;
                        let roll_off = (-fi * 1.5).exp();
                        ((s1 + s2 + s3) * roll_off * db_to_gain(self.source.drive_db + 14.0)).clamp(0.0, 1.0)
                    })
                    .collect();

                // Synthesize 64 time-domain samples
                let samples: Vec<f32> = (0..64)
                    .map(|i| {
                        let phase = i as f32 / 64.0 * 4.0 * std::f32::consts::PI;
                        let s = (phase + t * 4.0).sin() * 0.7 + (phase * 2.0 - t * 2.0).sin() * 0.3;
                        s * db_to_gain(self.source.drive_db + 14.0)
                    })
                    .collect();

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Spectrum Analyzer").strong());
                            for (name, var) in [
                                ("Area", SpectrumVariant::Area),
                                ("Line", SpectrumVariant::Line),
                                ("Bars", SpectrumVariant::Bars),
                            ] {
                                if ui.selectable_label(self.spectrum_variant == var, name).clicked() {
                                    self.spectrum_variant = var;
                                }
                            }
                        });

                        ui.add(
                            Spectrum::new(&bands)
                                .variant(self.spectrum_variant)
                                .peak_hold(&mut self.spectrum_peaks)
                                .width(340.0)
                                .height(110.0),
                        );
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Smooth Waveform").strong());
                            for (name, mode) in [
                                ("Wave", SmoothWaveformMode::Wave),
                                ("Scope", SmoothWaveformMode::Scope),
                            ] {
                                if ui.selectable_label(self.smooth_mode == mode, name).clicked() {
                                    self.smooth_mode = mode;
                                }
                            }
                            ui.checkbox(&mut self.vis_loading, "Loading pulse");
                        });

                        ui.add(
                            SmoothWaveform::new(&samples)
                                .mode(self.smooth_mode)
                                .loading(self.vis_loading)
                                .width(340.0)
                                .height(110.0),
                        );
                    });
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Bar Visualizer").strong());
                            for (name, idle) in [
                                ("Wave", BarIdle::Wave),
                                ("Pulse", BarIdle::Pulse),
                                ("Static", BarIdle::Static),
                            ] {
                                if ui.selectable_label(self.bar_idle == idle, name).clicked() {
                                    self.bar_idle = idle;
                                }
                            }
                        });

                        ui.add(
                            BarVisualizer::new(&bands)
                                .mirrored(true)
                                .align(BarAlign::Center)
                                .idle(self.bar_idle)
                                .loading(self.vis_loading)
                                .width(340.0)
                                .height(56.0),
                        );
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label(RichText::new("Live Streaming Waveform").strong());
                        ui.add(
                            LiveWaveform::new(&bands)
                                .variant(LiveWaveformVariant::Mirror)
                                .width(340.0)
                                .height(56.0),
                        );
                    });
                });

                ui.add_space(8.0);

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Audio Clip Waveform (Interactive seek, loop region, cue markers)").strong());
                        ui.label(format!("{:.1}%", self.waveform_progress * 100.0));
                    });

                    let static_peaks: Vec<f32> = (0..96)
                        .map(|i| {
                            let fi = i as f32 / 96.0;
                            let b1 = (fi * 12.0).sin().abs() * 0.7;
                            let b2 = (fi * 33.0).cos().abs() * 0.3;
                            let env = (fi * std::f32::consts::PI).sin().powf(0.6);
                            (b1 + b2) * env * 0.85 + 0.1
                        })
                        .collect();

                    let regions = [
                        WaveformRegion::new(15.0, 45.0)
                            .label("Loop 1")
                            .color(Color32::from_rgb(16, 185, 129)),
                    ];
                    let markers = [
                        WaveformMarker::new(8.0).label("Intro"),
                        WaveformMarker::new(48.0).label("Drop").color(Color32::from_rgb(239, 68, 68)),
                        WaveformMarker::new(85.0).label("Outro"),
                    ];

                    ui.add(
                        Waveform::new(&static_peaks, &mut self.waveform_progress)
                            .duration(110.0)
                            .variant(WaveformVariant::Mirror)
                            .regions(&regions)
                            .markers(&markers)
                            .width(ui.available_width())
                            .height(68.0),
                    );
                });
            },
        );
    }

    /// Electric plasma crackle bar visualizer and fractal lightning waveform.
    fn electric_visualizers(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Electric Visualizers",
            "Midpoint-displacement fractal lightning arcs and glowing plasma discharges.",
            |ui| {
                let t = self.source.t;
                let bands: Vec<f32> = (0..24)
                    .map(|i| {
                        let fi = i as f32 / 24.0;
                        ((t * 4.0 + fi * 8.0).sin().abs() * 0.7 + 0.1).clamp(0.0, 1.0)
                    })
                    .collect();

                let samples: Vec<f32> = (0..32)
                    .map(|i| {
                        let phase = i as f32 / 32.0 * 2.0 * std::f32::consts::PI;
                        (phase * 2.0 + t * 5.0).sin() * 0.6
                    })
                    .collect();

                ui.horizontal(|ui| {
                    ui.label("Intensity");
                    ui.add(Slider::new(&mut self.electric_intensity, 0.0..=1.0));
                });
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Electric Bar Visualizer").strong());
                        ui.add(
                            ElectricBarVisualizer::new(&bands)
                                .mirrored(true)
                                .intensity(self.electric_intensity)
                                .width(340.0)
                                .height(64.0),
                        );
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label(RichText::new("Electric Lightning Waveform").strong());
                        ui.add(
                            ElectricWaveform::new(&samples)
                                .intensity(self.electric_intensity)
                                .width(340.0)
                                .height(64.0),
                        );
                    });
                });
            },
        );
    }

    /// Vintage hardware backlit bezel LCD screen and dot-matrix level meter.
    fn lcd_components(&mut self, ui: &mut Ui) {
        section(
            ui,
            "LCD Screen & Meter",
            "Hardware bezel panel with scanlines, glass sheen, ghost segments, and authentic phosphor inks.",
            |ui| {
                let pair = self.source.stereo(0, 0.0, 0.0);
                let (variant, appearance) = (self.lcd_variant, self.lcd_appearance);

                ui.horizontal(|ui| {
                    ui.label("Ink Color");
                    for (name, var) in [
                        ("Amber", LcdVariant::Amber),
                        ("Green", LcdVariant::Green),
                        ("Cyan", LcdVariant::Cyan),
                        ("Red", LcdVariant::Red),
                        ("Primary", LcdVariant::Primary),
                    ] {
                        if ui.selectable_label(self.lcd_variant == var, name).clicked() {
                            self.lcd_variant = var;
                        }
                    }

                    ui.separator();

                    ui.label("Appearance");
                    for (name, app) in [
                        ("Auto", LcdAppearance::Auto),
                        ("Dark", LcdAppearance::Dark),
                        ("Light", LcdAppearance::Light),
                    ] {
                        if ui.selectable_label(self.lcd_appearance == app, name).clicked() {
                            self.lcd_appearance = app;
                        }
                    }
                });
                ui.add_space(8.0);

                let rows = [
                    LcdDisplayRow::new("RACK", "DSP-01"),
                    LcdDisplayRow::new("SAMPLE RATE", "48.0 kHz"),
                    LcdDisplayRow::rule(),
                    LcdDisplayRow::new("DEPTH", "24-BIT"),
                ];
                let display = LcdDisplay::new(&rows)
                    .variant(variant)
                    .appearance(appearance)
                    .width(460.0)
                    .status(LcdStatus::Busy);

                // A caller owns the metering clock: the LCD meter only draws
                // the holds it is handed, exactly as the React one is fed.
                let now = ui.input(|input| input.time) as f32 * 1000.0;
                let holds: [f32; 2] = std::array::from_fn(|lane| {
                    self.lcd_holds[lane]
                        .step(pair[lane].peak_db, self.ballistics, now)
                        .hold_db
                });

                ui.add(
                    LcdMeter::stereo(&pair)
                        .holds(&holds)
                        .range(-48.0, 6.0)
                        .rms(true)
                        .clipping(pair[0].peak_db > -1.0 || pair[1].peak_db > -1.0)
                        .on_display(&display),
                );
                ui.add_space(8.0);
                ui.label("The same meter on its own screen, with no rows above it:");
                ui.add(LcdMeter::stereo(&pair).variant(variant).appearance(appearance).range(-48.0, 6.0).holds(&holds));
            },
        );
    }

    /// Audio player transport, playlist queue, and MPC drum pads.
    fn media_components(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Sounds & Music",
            "Complete audio player transport, playlist track list, and interactive sound pad grid.",
            |ui| {
                ui.horizontal(|ui| {
                    // 1. Audio Player
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Audio Player Transport").strong());
                        ui.add(
                            AudioPlayer::new(&mut self.player_playing, &mut self.player_time, 184.0)
                                .title("Midnight Session")
                                .artist("Noda Audio")
                                .volume(&mut self.player_volume, &mut self.player_muted)
                                .rate(&mut self.player_rate)
                                .looping(&mut self.player_loop)
                                .width(340.0),
                        );
                    });

                    ui.separator();

                    // 2. Track List
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Playlist Track List").strong());
                        let tracks = [
                            TrackItem::new(1, "Neon Horizon", 195.0)
                                .artist("Synthetix")
                                .active(self.playlist_selected == Some(1))
                                .playing(self.playlist_selected == Some(1)),
                            TrackItem::new(2, "Velvet Sunset", 242.0)
                                .artist("Antigravity")
                                .active(self.playlist_selected == Some(2)),
                            TrackItem::new(3, "Subsurface Pulse", 188.0)
                                .artist("Deep Dive")
                                .active(self.playlist_selected == Some(3)),
                        ];
                        ui.add(
                            TrackList::new(&tracks, &mut self.playlist_selected)
                                .width(320.0)
                                .row_height(38.0),
                        );
                    });
                });

                ui.add_space(10.0);

                // 3. Sound Pad Grid
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Sound Pad Grid (Click to trigger)").strong());
                    });
                    ui.add_space(4.0);
                    ui.add(
                        SoundPadGrid::new(&mut self.sound_pads)
                            .columns(4)
                            .pad_size(72.0)
                            .gap(8.0),
                    );
                });
            },
        );
    }

    /// High-level composite audio UI blocks (Microphone Setup, Music Player, Soundboard, Quick Audio Popover, System Audio Mixer).
    fn composite_blocks(&mut self, ui: &mut Ui) {
        section(
            ui,
            "High-Level Composite Blocks",
            "Audiocn-inspired domain blocks assembling controls, meters, and visualizers into complete interfaces.",
            |ui| {
                let t = self.source.t;
                let pair = self.source.stereo(0, 0.0, 0.0);
                let bar_levels: Vec<f32> = (0..5)
                    .map(|i| {
                        let fi = i as f32 / 5.0;
                        ((t * 5.0 + fi * 6.0).sin().abs() * 0.7 + 0.15).clamp(0.0, 1.0)
                    })
                    .collect();

                let wave_samples: Vec<f32> = (0..48)
                    .map(|i| {
                        let p = i as f32 / 48.0 * 2.0 * std::f32::consts::PI;
                        (p * 3.0 + t * 6.0).sin() * 0.7
                    })
                    .collect();

                let static_peaks: Vec<f32> = (0..64)
                    .map(|i| {
                        let fi = i as f32 / 64.0;
                        let b1 = (fi * 10.0).sin().abs() * 0.6;
                        let b2 = (fi * 28.0).cos().abs() * 0.4;
                        (b1 + b2).clamp(0.05, 0.95)
                    })
                    .collect();

                let spec_bands: Vec<f32> = (0..24)
                    .map(|i| {
                        let fi = i as f32 / 24.0;
                        ((t * 4.0 + fi * 7.0).sin().abs() * 0.8 + 0.1).clamp(0.0, 1.0)
                    })
                    .collect();

                let devices = [
                    AudioDevice::new("mic_builtin", "MacBook Pro Microphone").default_device(true),
                    AudioDevice::new("mic_usb", "Elgato Wave:3 USB"),
                    AudioDevice::new("mic_scarlett", "Focusrite Scarlett 2i2").available(false),
                ];

                let out_devices = [
                    AudioDevice::new("sys_speakers", "MacBook Pro Speakers").default_device(true),
                    AudioDevice::new("sys_headphones", "External Headphones"),
                    AudioDevice::new("sys_airpods", "AirPods Pro").available(false),
                ];

                // Row 1: Quick Audio Popover Header + Mic Setup Block
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Quick Audio Popover").strong());
                        ui.add_space(4.0);
                        ui.add(QuickAudioPopover::new(
                            "showcase_quick_audio",
                            &out_devices,
                            &mut self.block_popover_dev,
                            &mut self.block_popover_vol,
                            &mut self.block_popover_muted,
                            pair,
                            &bar_levels,
                        ));
                        ui.add_space(12.0);
                        ui.label(RichText::new("Microphone Setup Block").strong());
                        ui.add_space(4.0);
                        ui.add(
                            MicSetupBlock::new(
                                &devices,
                                &mut self.mic_device,
                                &mut self.mic_gain,
                                &mut self.mic_muted,
                                &mut self.mic_active,
                                pair[0],
                                &wave_samples,
                                &mut self.mic_check,
                            )
                            .width(420.0),
                        );
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label(RichText::new("Live Soundboard Block").strong());
                        ui.add_space(4.0);
                        ui.add(
                            SoundboardBlock::new(
                                "Live Stream FX",
                                &mut self.block_sound_pads,
                                &mut self.block_sound_vol,
                                &mut self.block_sound_muted,
                            )
                            .columns(4)
                            .width(400.0),
                        );
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);

                // Row 2: Music Player Block
                ui.vertical(|ui| {
                    ui.label(RichText::new("Music Player Block").strong());
                    ui.add_space(4.0);
                    let tracks = [
                        TrackItem::new(1, "Neon Horizons", 214.0)
                            .artist("Synthwave Collective")
                            .active(self.block_music_track == Some(0))
                            .playing(self.block_music_playing && self.block_music_track == Some(0)),
                        TrackItem::new(2, "Midnight Drive", 185.0)
                            .artist("Kavinsky Tribute")
                            .active(self.block_music_track == Some(1)),
                        TrackItem::new(3, "Subsurface Pulse", 240.0)
                            .artist("Deep Dive Studio")
                            .active(self.block_music_track == Some(2)),
                    ];
                    let markers = [
                        WaveformMarker::new(32.0).label("Verse 1"),
                        WaveformMarker::new(88.0).label("Drop").color(Color32::from_rgb(239, 68, 68)),
                        WaveformMarker::new(160.0).label("Outro"),
                    ];
                    let regions = [
                        WaveformRegion::new(60.0, 110.0)
                            .label("Chorus Loop")
                            .color(Color32::from_rgb(16, 185, 129)),
                    ];

                    ui.add(
                        MusicPlayerBlock::new(
                            &tracks,
                            &mut self.block_music_track,
                            &mut self.block_music_playing,
                            &mut self.block_music_time,
                            &mut self.block_music_vol,
                            &mut self.block_music_muted,
                            &mut self.block_music_rate,
                            &mut self.block_music_loop,
                            &static_peaks,
                            &markers,
                            &regions,
                            Some(&spec_bands),
                            &mut self.block_music_show_spec,
                        )
                        .width(ui.available_width().min(640.0)),
                    );
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(12.0);

                // Row 3: System Audio Mixer Block
                ui.vertical(|ui| {
                    ui.label(RichText::new("System Audio Mixer Block").strong());
                    ui.add_space(4.0);

                    let p1 = self.source.stereo(0, self.block_sys_gain[0], self.block_sys_pan[0]);
                    let p2 = self.source.stereo(1, self.block_sys_gain[1], self.block_sys_pan[1]);
                    let p3 = self.source.stereo(0, self.block_sys_gain[2], self.block_sys_pan[2]);
                    let master_levels = sum(sum(p1, p2), p3);

                    let [g0, g1, g2] = &mut self.block_sys_gain;
                    let [pan0, pan1, pan2] = &mut self.block_sys_pan;
                    let [m0, m1, m2] = &mut self.block_sys_mute;
                    let [s0, s1, s2] = &mut self.block_sys_solo;

                    let channels = vec![
                        MixerChannelData {
                            name: "Desktop",
                            number: 1,
                            accent: Some(Color32::from_rgb(59, 130, 246)),
                            gain_db: g0,
                            pan: pan0,
                            muted: m0,
                            solo: s0,
                            levels: &p1,
                        },
                        MixerChannelData {
                            name: "Microphone",
                            number: 2,
                            accent: Some(Color32::from_rgb(16, 185, 129)),
                            gain_db: g1,
                            pan: pan1,
                            muted: m1,
                            solo: s1,
                            levels: &p2,
                        },
                        MixerChannelData {
                            name: "Spotify",
                            number: 3,
                            accent: Some(Color32::from_rgb(234, 179, 8)),
                            gain_db: g2,
                            pan: pan2,
                            muted: m2,
                            solo: s2,
                            levels: &p3,
                        },
                    ];

                    let master = Some(MixerMasterData {
                        name: "Main Mix",
                        gain_db: Some(&mut self.block_sys_master_gain),
                        pan: None,
                        muted: Some(&mut self.block_sys_master_mute),
                        levels: &master_levels,
                    });

                    ui.add(
                        SystemAudioMixerBlock::new(
                            "Broadcast Audio Console",
                            channels,
                            master,
                            &mut self.block_sys_orientation,
                        )
                        .width(ui.available_width().min(640.0)),
                    );
                });
            },
        );
    }

    fn eq_section(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Parametric EQ (FabFilter Pro-Q Style)",
            "Interactive frequency response curve, log grid (20Hz..20kHz), draggable handles, mouse-wheel Q, analytical biquad filter magnitude.",
            |ui| {
                ui.vertical(|ui| {
                    ui.add(
                        EqCurve::new(&mut self.eq_bands, &mut self.selected_eq_band)
                            .size(ui.available_width().max(420.0), 210.0),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Selected Band:").strong());
                        for i in 0..self.eq_bands.len() {
                            let is_sel = self.selected_eq_band == Some(i);
                            let btn_text = format!("Band {}", i + 1);
                            if ui.selectable_label(is_sel, btn_text).clicked() {
                                self.selected_eq_band = Some(i);
                            }
                        }
                    });

                    ui.add_space(6.0);
                    if let Some(idx) = self.selected_eq_band {
                        if idx < self.eq_bands.len() {
                            let band = &mut self.eq_bands[idx];
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("Filter Shape").strong());
                                    ui.add(FilterTypeSelector::new(&mut band.filter_type));
                                });

                                ui.add_space(16.0);
                                ui.vertical(|ui| {
                                    ui.add(
                                        ParameterSlider::new(&mut band.frequency, 20.0, 20000.0)
                                            .taper(Taper::Log)
                                            .label("Frequency")
                                            .unit("Hz")
                                            .decimals(1)
                                            .width(180.0),
                                    );
                                });

                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.add(
                                        ParameterSlider::new(&mut band.gain_db, -18.0, 18.0)
                                            .origin(0.0)
                                            .label("Gain")
                                            .unit("dB")
                                            .decimals(1)
                                            .width(180.0),
                                    );
                                });

                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.add(
                                        ParameterSlider::new(&mut band.q, 0.1, 10.0)
                                            .taper(Taper::Log)
                                            .label("Q Factor")
                                            .decimals(2)
                                            .width(160.0),
                                    );
                                });
                            });
                        }
                    }
                });
            },
        );
    }

    fn dynamics_section(&mut self, ui: &mut Ui) {
        if self.live {
            let level = self.source.level(0, 0, 0.0);
            self.vu_reading = level.peak_db;
            let over = (level.peak_db - self.comp_threshold).max(0.0);
            self.gr_reading = -(over * (1.0 - 1.0 / self.comp_ratio)).min(24.0);
        }

        section(
            ui,
            "Analog VU Meter & Pro Dynamics Suite",
            "ANSI moving-coil ballistics, downward Gain Reduction (GR) meter, and interactive compressor transfer curve.",
            |ui| {
                ui.horizontal_top(|ui| {
                    // Vintage & Modern VU Meters
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Vintage Analog VU Meter").strong());
                        ui.add_space(4.0);
                        ui.add(
                            VuMeter::from_dbfs(self.vu_reading)
                                .appearance(VuAppearance::Vintage)
                                .size(170.0, 110.0),
                        );
                        ui.add_space(6.0);
                        ui.label(RichText::new("Dark Modern VU").strong());
                        ui.add_space(4.0);
                        ui.add(
                            VuMeter::from_dbfs(self.vu_reading)
                                .appearance(VuAppearance::Dark)
                                .size(170.0, 105.0),
                        );
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Downward Gain Reduction Meter
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Gain Reduction (GR)").strong());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                GainReductionMeter::single(&self.gr_reading)
                                    .length(220.0),
                            );
                            ui.add_space(8.0);
                            let stereo_gr = [self.gr_reading, self.gr_reading * 0.85];
                            ui.add(
                                GainReductionMeter::new(&stereo_gr)
                                    .length(220.0),
                            );
                        });
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Dynamics Processor (Compressor / Limiter / Gate)
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for (tab, name) in ["Compressor", "Limiter", "Expander / Gate"].iter().enumerate() {
                                if ui.selectable_label(self.dynamics_tab == tab, *name).clicked() {
                                    self.dynamics_tab = tab;
                                }
                            }
                        });
                        ui.add_space(4.0);

                        match self.dynamics_tab {
                            0 => {
                                ui.horizontal_top(|ui| {
                                    let live_in = self.source.level(0, 0, 0.0).peak_db;
                                    let mut comp_curve = CompressorCurve::new(
                                        &mut self.comp_threshold,
                                        &mut self.comp_ratio,
                                    )
                                    .knee(self.comp_knee)
                                    .makeup_gain(self.comp_makeup)
                                    .size(200.0, 175.0);

                                    if self.live {
                                        comp_curve = comp_curve.live_input(live_in);
                                    }

                                    ui.add(comp_curve);
                                    ui.add_space(12.0);
                                    ui.add(
                                        CompressorControls::new(
                                            &mut self.comp_threshold,
                                            &mut self.comp_ratio,
                                            &mut self.comp_attack,
                                            &mut self.comp_release,
                                            &mut self.comp_knee,
                                            &mut self.comp_makeup,
                                        )
                                        .sidechain(&mut self.comp_sc_hz, &mut self.comp_sc_active)
                                        .width(220.0),
                                    );
                                });
                            }
                            1 => {
                                ui.horizontal_top(|ui| {
                                    let live_in = self.source.level(0, 0, 0.0).peak_db;
                                    let mut brickwall_ratio = 20.0;
                                    let mut comp_curve = CompressorCurve::new(
                                        &mut self.limiter_ceiling,
                                        &mut brickwall_ratio,
                                    )
                                    .knee(0.5)
                                    .size(200.0, 175.0);

                                    if self.live {
                                        comp_curve = comp_curve.live_input(live_in);
                                    }

                                    ui.add(comp_curve);
                                    ui.add_space(12.0);
                                    ui.add(
                                        LimiterControls::new(
                                            &mut self.limiter_ceiling,
                                            &mut self.limiter_release,
                                            &mut self.limiter_true_peak,
                                        )
                                        .width(220.0),
                                    );
                                });
                            }
                            _ => {
                                ui.horizontal_top(|ui| {
                                    let live_in = self.source.level(0, 0, 0.0).peak_db;
                                    let mut gate_ratio = 1.0;
                                    let mut comp_curve = CompressorCurve::new(
                                        &mut self.gate_threshold,
                                        &mut gate_ratio,
                                    )
                                    .size(200.0, 175.0);

                                    if self.live {
                                        comp_curve = comp_curve.live_input(live_in);
                                    }

                                    ui.add(comp_curve);
                                    ui.add_space(12.0);
                                    ui.add(
                                        GateControls::new(
                                            &mut self.gate_threshold,
                                            &mut self.gate_range,
                                            &mut self.gate_attack,
                                            &mut self.gate_hold,
                                            &mut self.gate_release,
                                            &mut self.gate_hysteresis,
                                        )
                                        .width(220.0),
                                    );
                                });
                            }
                        }
                    });
                });
            },
        );
    }

    fn saturation_section(&mut self, ui: &mut Ui) {
        let live_in = if self.live {
            Some((self.source.t * 3.5).sin() * 0.85)
        } else {
            None
        };

        let clip = live_in.map_or_else(
            || ClipIndicators::new(false, false),
            |peak| ClipIndicators::from_peak(self.sat_type, self.sat_drive, self.sat_bias, peak),
        );

        section(
            ui,
            "Saturation & Distortion Suite",
            "Transfer curve, shaper, Drive/Mix/Tone/Bias controls, a live harmonic spectrum with THD, and soft/hard clip lamps.",
            |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Transfer Curve f(x)").strong());
                        ui.add_space(4.0);
                        ui.add(
                            SaturationCurve::new(self.sat_type, self.sat_drive)
                                .mix(self.sat_mix / 100.0)
                                .bias(self.sat_bias)
                                .input_level(live_in)
                                .desired_size(Vec2::new(180.0, 180.0)),
                        );
                        ui.add_space(6.0);
                        ui.add(SaturationTypeSelect::new(&mut self.sat_type).width(180.0));
                    });

                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Saturator Unit").strong());
                        ui.add_space(4.0);
                        ui.add(
                            SaturatorControls::new(
                                &mut self.sat_drive,
                                &mut self.sat_mix,
                                &mut self.sat_tone,
                                &mut self.sat_bias,
                            )
                            .sat_type(&mut self.sat_type)
                            .width(135.0),
                        );
                        ui.add_space(8.0);
                        ui.add(clip);
                    });

                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Distortion Unit").strong());
                        ui.add_space(4.0);
                        ui.add(
                            DistortionControls::new(
                                &mut self.dist_drive,
                                &mut self.dist_type,
                                &mut self.dist_tone,
                                &mut self.dist_output,
                            )
                            .width(135.0),
                        );
                    });

                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Harmonic Spectrum").strong());
                        ui.add_space(4.0);
                        ui.add(
                            HarmonicsSpectrum::new(self.sat_type, self.sat_drive)
                                .bias(self.sat_bias)
                                .mix(self.sat_mix / 100.0)
                                .count(8)
                                .desired_size(Vec2::new(240.0, 180.0)),
                        );
                    });
                });
            },
        );
    }

    fn pro_plugin_section(&mut self, ui: &mut Ui) {
        section(
            ui,
            "Pro Audio Modular Rack Frame & Plugin Shell",
            "Hardware-inspired 19\" rack chassis with corner mounting screws, top toolbar (A/B, oversampling, I/O meters), preset browser, and collapsible sections.",
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Chassis Style:").strong());
                    for (name, style) in [
                        ("Dark Aluminium", RackStyle::DarkAluminium),
                        ("Vintage Faceplate", RackStyle::VintageBeige),
                        ("Studio Slate", RackStyle::SlateGrey),
                    ] {
                        if ui.selectable_label(self.rack_style == style, name).clicked() {
                            self.rack_style = style;
                        }
                    }
                });
                ui.add_space(8.0);

                let in_peaks = [-12.0, -9.5];
                let out_peaks = [-6.2, -6.0];

                RackFrame::new("NODA ANALOG MASTERING PROCESSOR")
                    .subtitle("OPTICAL BUS COMPRESSOR & PARAMETRIC CONSOLE EQ")
                    .style(self.rack_style)
                    .power_on(!self.plugin_bypassed)
                    .show(ui, |ui| {
                        // 1. Top Header Bar
                        ui.add(
                            PluginHeaderBar::new("Mastering Suite", &mut self.plugin_bypassed)
                                .ab_state(&mut self.plugin_ab)
                                .oversampling(&mut self.plugin_oversampling)
                                .in_gain(&mut self.plugin_in_gain, Some(in_peaks))
                                .out_gain(&mut self.plugin_out_gain, Some(out_peaks)),
                        );

                        ui.add_space(8.0);

                        // 2. Preset Browser
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Preset:").strong());
                            ui.add(PresetBrowser::new(&self.plugin_presets, &mut self.plugin_preset_idx));
                        });

                        ui.add_space(10.0);

                        // 3. Section Headers and Grouped Controls
                        ui.add(SectionHeader::new("Parametric Equalizer").enabled(&mut self.rack_section_eq));
                        if self.rack_section_eq {
                            ui.horizontal(|ui| {
                                for (i, band) in self.eq_bands.iter_mut().enumerate() {
                                    ui.group(|ui| {
                                        ui.label(format!("Band {}", i + 1));
                                        ui.add(
                                            ParameterSlider::new(&mut band.frequency, 20.0, 20000.0)
                                                .taper(Taper::Log)
                                                .label("Freq")
                                                .unit("Hz")
                                                .decimals(0)
                                                .width(110.0),
                                        );
                                        ui.add(
                                            ParameterSlider::new(&mut band.gain_db, -18.0, 18.0)
                                                .origin(0.0)
                                                .label("Gain")
                                                .unit("dB")
                                                .decimals(1)
                                                .width(110.0),
                                        );
                                    });
                                }
                            });
                        }

                        ui.add_space(6.0);
                        ui.add(SectionHeader::new("Mastering Dynamics").enabled(&mut self.rack_section_comp));
                        if self.rack_section_comp {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label("Threshold");
                                    let knob = ui.add(MetallicKnob::mini(&mut self.comp_threshold, -40.0, 0.0).diameter(50.0));
                                    show_drag_value(ui, &knob, &format!("{:.1} dB", self.comp_threshold));
                                    ui.label(format!("{:.1} dB", self.comp_threshold));
                                });
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.label("Ratio");
                                    let knob = ui.add(MetallicKnob::mini(&mut self.comp_ratio, 1.0, 20.0).diameter(50.0));
                                    show_drag_value(ui, &knob, &format!("{:.1}:1", self.comp_ratio));
                                    ui.label(format!("{:.1}:1", self.comp_ratio));
                                });
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.label("Makeup");
                                    let knob = ui.add(MetallicKnob::mini(&mut self.comp_makeup, 0.0, 18.0).diameter(50.0));
                                    show_drag_value(ui, &knob, &format!("{:.1} dB", self.comp_makeup));
                                    ui.label(format!("{:.1} dB", self.comp_makeup));
                                });
                            });
                        }

                        ui.add_space(6.0);
                        ui.add(SectionHeader::new("Analog Tape / Tube Saturation").enabled(&mut self.rack_section_sat));
                        if self.rack_section_sat {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label("Drive");
                                    let knob = ui.add(MetallicKnob::mini(&mut self.sat_drive, 1.0, 10.0).diameter(50.0));
                                    show_drag_value(ui, &knob, &format!("{:.1}x", self.sat_drive));
                                    ui.label(format!("{:.1}x", self.sat_drive));
                                });
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.label("Mix");
                                    let knob = ui.add(MetallicKnob::mini(&mut self.sat_mix, 0.0, 100.0).diameter(50.0));
                                    show_drag_value(ui, &knob, &format!("{:.0}%", self.sat_mix));
                                    ui.label(format!("{:.0}%", self.sat_mix));
                                });
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.label("Bias");
                                    let knob = ui.add(
                                        MetallicKnob::mini(&mut self.sat_bias, -0.8, 0.8)
                                            .diameter(50.0)
                                            .bipolar(true),
                                    );
                                    show_drag_value(ui, &knob, &format!("{:+.2}", self.sat_bias));
                                    ui.label(format!("{:+.2}", self.sat_bias));
                                });
                            });
                        }
                    });
            },
        );
    }

    /// Font size and colour for the prose next to a control.
    fn caption(&self) -> (f32, egui::Color32) {
        (self.theme.size.label_font(), self.theme.palette.text_dim)
    }
}

impl eframe::App for Showcase {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme_stale {
            self.theme.install(ctx);
            self.theme_stale = false;
        }
        self.source.t = ctx.input(|input| input.time) as f32;
        if self.live {
            // Meter movement is wall-clock driven, so the frame has to keep
            // coming round or the bars freeze mid-release.
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show_inside(ui, |ui| self.header(ui));
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                self.meters(ui);
                self.faders(ui);
                self.knobs(ui);
                self.metallic_knobs(ui);
                self.parameter_controls(ui);
                self.visualizers(ui);
                self.electric_visualizers(ui);
                self.lcd_components(ui);
                self.scales(ui);
                self.clips(ui);
                self.mixer(ui);
                self.full_mixer(ui);
                self.bus_master_section(ui);
                self.media_components(ui);
                self.composite_blocks(ui);
                self.eq_section(ui);
                self.dynamics_section(ui);
                self.saturation_section(ui);
                self.pro_plugin_section(ui);
            });
        });
    }
}

/// A titled panel: the showcase reads as a style guide, one component per
/// block, so each gets a name and the sentence that explains it.
fn section(ui: &mut Ui, title: &str, note: &str, body: impl FnOnce(&mut Ui)) {
    let theme = Theme::of(ui);
    let palette = &theme.palette;
    egui::Frame::group(ui.style())
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0_f32, palette.border))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(FontId::proportional(theme.size.value_font() + 3.0))
                        .color(palette.primary),
                );
                ui.label(RichText::new(note).color(palette.text_dim));
            });
            ui.add_space(8.0);
            body(ui);
        });
    ui.add_space(10.0);
}

/// Reserve `want` in the current row and hand the widget the whole rect:
/// a meter or a fader peels scale, lamp and readout off what it is given.
fn fill(ui: &mut Ui, want: Vec2, widget: impl Widget) -> Response {
    let (rect, _) = ui.allocate_at_least(want, Sense::hover());
    ui.put(rect, widget)
}

/// A control at the row's height, as wide as it asks to be.
fn at_height(ui: &mut Ui, want: Vec2, height: f32, widget: impl Widget) -> Response {
    fill(ui, Vec2::new(want.x, height.max(want.y)), widget)
}

/// A control at its own height, stretched over what is left of the row.
fn across(ui: &mut Ui, want: Vec2, width: f32, widget: impl Widget) -> Response {
    fill(ui, Vec2::new(width, want.y), widget)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gallery_paints() {
        let ctx = egui::Context::default();
        let mut app = Showcase::new(&ctx);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.meters(ui);
            app.faders(ui);
            app.knobs(ui);
            app.metallic_knobs(ui);
            app.parameter_controls(ui);
            app.visualizers(ui);
            app.electric_visualizers(ui);
            app.lcd_components(ui);
            app.scales(ui);
            app.clips(ui);
            app.mixer(ui);
            app.full_mixer(ui);
            app.bus_master_section(ui);
            app.media_components(ui);
            app.composite_blocks(ui);
            app.eq_section(ui);
            app.dynamics_section(ui);
            app.saturation_section(ui);
            app.pro_plugin_section(ui);
        });
        assert!(
            output.shapes.len() > 200,
            "six sections of controls should draw plenty, got {}",
            output.shapes.len()
        );
    }

    /// The row-height contract the gallery depends on: a control added with
    /// `at_height` gets the whole of the space the row reserved.
    #[test]
    fn a_control_fills_the_height_its_row_reserved() {
        let ctx = egui::Context::default();
        let levels = [ChannelLevel::silence(), ChannelLevel::silence()];
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let theme = Theme::of(ui);
                let meter = LevelMeter::stereo(&levels).id(egui::Id::new("row-height"));
                let response = at_height(ui, meter.desired_size(&theme), TALL, meter);
                assert_eq!(response.rect.height(), TALL, "the row keeps its height");
            });
        });
        assert!(!output.shapes.is_empty(), "the meter drew into the row");
    }

    #[test]
    fn the_light_theme_paints_too() {
        let ctx = egui::Context::default();
        let mut app = Showcase::new(&ctx);
        app.set_light(true);
        app.theme.install(&ctx);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.mixer(ui);
        });
        assert!(!output.shapes.is_empty(), "a strip under the light palette still draws");
    }
}
