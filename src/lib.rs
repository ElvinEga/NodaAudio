//! NodaAudio: audio UI components for [`egui`], plus a demo plugin showing
//! them wired to a real parameter and meter bridge.
//!
//! The component modules — [`ui`], [`db`], [`ballistics`], [`taper`],
//! [`zones`] — depend on egui alone. They draw the numbers they are handed
//! and open no device, so the same [`ui::LevelMeter`] serves a plugin
//! reading its `process` peak, a standalone app measuring its own buffer,
//! and a unit test feeding it a sine sweep.
//!
//! The demo plugin at the bottom of this file is the other half: it is the
//! reference for how a truce plugin should host the components. Levels
//! cross the audio→GUI boundary through `#[meter]` slots, and the fader
//! writes its parameter through the editor bridge so a DAW sees the
//! automation.
//!
//! ```text
//!  audio thread          GUI thread
//!  process()   ──set_meter──▶   LevelMeter::stereo(&levels)
//!             ◀──set_param──    Fader::new(&mut db)
//! ```

pub mod ballistics;
pub mod db;
pub mod electric;
pub mod oklab;
pub mod taper;
pub mod ui;
pub mod zones;

use std::f32::consts::FRAC_PI_4;
use std::sync::Arc;

use egui::{Align2, FontId, Rect, Response, Vec2, pos2};
use truce::prelude::*;
use truce_egui::{EditorUi, EguiEditor};

use crate::db::{clamp, db_to_gain, gain_to_db};
use crate::taper::Taper;
use crate::ui::theme::Theme;
use crate::ui::{
    ChannelLevel, ChannelStrip, ChannelStripVariant, Edge, Fader, Knob, LevelMeter, MeterStyle,
    peel,
};

/// Seconds for one round trip of the test tone's envelope.
const SWEEP_SECONDS: f32 = 8.0;

/// The frequency the input and output channels sweep to, chosen a third
/// apart so the stereo pair moves independently.
const TEST_HZ: [f32; 2] = [220.0, 277.0];

/// The demo's parameters and meter slots.
///
/// `#[param]` fields are host-visible; `#[meter]` fields are the one-way
/// 0..1 lanes the audio thread publishes levels on. The derive assigns ids
/// to both — parameters from the low end, meters from `METER_ID_BASE` — so
/// nothing has to be numbered by hand.
#[derive(Params)]
pub struct DemoParams {
    /// Output gain in dB. The range has headroom above unity so the fader
    /// shows its linear top segment.
    #[param(
        name = "Gain",
        range = "linear(-60, 12)",
        unit = "dB",
        default = 0.0,
        smooth = "exp(5)"
    )]
    pub gain: FloatParam,

    /// Stereo balance. Centre-detented, so a host recall and a knob drag
    /// both land exactly on the middle.
    #[param(
        name = "Pan",
        range = "sym_skewed(-1, 1, 2, 0)",
        unit = "pan",
        default = 0.0,
        smooth = "exp(5)"
    )]
    pub pan: FloatParam,

    /// Generate a swept test tone instead of passing audio through, so the
    /// meters move with no input device and no host. The demo ships with it
    /// on; a real plugin would default this off.
    #[param(name = "Test signal", default = true)]
    pub test_signal: BoolParam,

    /// Mute output channels.
    #[param(name = "Mute", default = false)]
    pub mute: BoolParam,

    #[meter]
    pub input_left: MeterSlot,
    #[meter]
    pub input_right: MeterSlot,
    #[meter]
    pub output_left: MeterSlot,
    #[meter]
    pub output_right: MeterSlot,
    #[meter]
    pub rms_left: MeterSlot,
    #[meter]
    pub rms_right: MeterSlot,
}

/// Gain stage plus the level measurement that feeds the editor.
///
/// The struct is its own DSP state. It carries the clock the test
/// oscillator free-runs on and the sample rate `reset` stamps, so the tone
/// keeps its pitch across blocks whatever the host's block size.
#[derive(Default)]
pub struct Demo {
    sample_rate: f32,
    clock: f32,
}

impl Demo {
    /// The signal the meters see while the test tone is on.
    ///
    /// The envelope runs from silence to past full scale and back, so one
    /// pass lights every metering zone and fires the clip latch without
    /// anyone touching the fader.
    fn test_sample(&self, seconds: f32, channel: usize) -> f32 {
        let hz = TEST_HZ[channel % TEST_HZ.len()];
        let phase = TAU * f64::from(hz * seconds);
        let sweep = (0.5 - 0.5 * (TAU * f64::from(seconds / SWEEP_SECONDS)).cos()) as f32;
        phase.sin() as f32 * (sweep * sweep) * 1.6
    }
}

/// Equal-power pan over a balance of -1..1.
///
/// The two sides trade amplitude along a quarter circle, so their squares
/// always sum to unity: hard left is one channel alone, centre is -3 dB on
/// each, and nothing in between changes the apparent width of the sound by
/// changing its level.
fn pan_gains(balance: f32) -> (f32, f32) {
    let theta = (clamp(balance, -1.0, 1.0) + 1.0) * FRAC_PI_4;
    (theta.cos(), theta.sin())
}

impl PluginLogic for Demo {
    type Params = DemoParams;
    type DspState = Self;

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        state.sample_rate = config.sample_rate as f32;
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        let is_muted = params.mute.value();
        let gain = if is_muted { 0.0 } else { f64::from(db_to_gain(params.gain.read())) };
        let pan = pan_gains(params.pan.read());
        let test = params.test_signal.value();
        let samples = buffer.num_samples();

        let rate = if state.sample_rate > 0.0 { state.sample_rate } else { 48000.0 };
        let block_start = state.clock;
        if test {
            state.clock = (block_start + samples as f32 / rate) % SWEEP_SECONDS;
        }

        for channel in 0..buffer.channels() {
            let (input, output) = buffer.io(channel);
            // Only the first two channels form the panned pair; a wider bus
            // passes the rest unbalanced rather than doubling them up.
            let stage = gain * f64::from(match channel {
                0 => pan.0,
                1 => pan.1,
                _ => 1.0,
            });
            let mut source_peak = 0.0f32;
            let mut out_peak = 0.0f64;
            let mut out_energy = 0.0f64;

            for (i, out) in output.iter_mut().enumerate().take(samples) {
                let at = block_start + i as f32 / rate;
                let source = if test { state.test_sample(at, channel) } else { input[i] };
                source_peak = source_peak.max(source.abs());

                let sample = f64::from(source) * stage;
                *out = sample as f32;
                out_peak = out_peak.max(sample.abs());
                out_energy += sample * sample;
            }

            // The editor shows a stereo pair; wider buses are still gain
            // staged, only the first two channels report levels.
            let [peak_in, peak_out, rms_out] = match channel {
                0 => [params.input_left.id(), params.output_left.id(), params.rms_left.id()],
                1 => [params.input_right.id(), params.output_right.id(), params.rms_right.id()],
                _ => continue,
            };
            // 0..1 linear, clamped: the slots are fixed-point lanes and the
            // widgets turn them back into dBFS.
            let rms = if samples > 0 { (out_energy / samples as f64).sqrt() } else { 0.0 };
            context.set_meter(peak_in, source_peak.min(1.0));
            context.set_meter(peak_out, (out_peak as f32).min(1.0));
            context.set_meter(rms_out, (rms as f32).min(1.0));
        }

        ProcessStatus::Normal
    }

    fn editor(params: Arc<DemoParams>) -> Box<dyn Editor> {
        Box::new(
            EguiEditor::with_ui(params, (520, 380), DemoEditor::new())
                .resizable(true)
                .min_size((400, 300))
                .max_size((1600, 1200)),
        )
    }
}

truce::plugin! {
    logic: Demo,
    params: DemoParams,
}

truce::enable_rt_paranoid!();

/// The demo editor: a stereo input meter, a console fader, a stereo output
/// meter.
///
/// Everything the widgets draw — palette, dB range, meter zones — comes
/// from [`Theme`] and the component modules, not from truce, so this `ui`
/// body would drop into a plain egui app unchanged.
struct DemoEditor {
    theme: Theme,
    installed: bool,
    dark: bool,
    segmented: bool,
    show_strip: bool,
}

impl DemoEditor {
    fn new() -> Self {
        Self {
            theme: Theme::dark(),
            installed: false,
            dark: true,
            segmented: false,
            show_strip: true,
        }
    }
}

impl EditorUi<DemoParams> for DemoEditor {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &PluginContext<DemoParams>) {
        if !self.installed {
            self.theme.install(ui.ctx());
            self.installed = true;
        }
        let theme = Theme::of(ui);
        let palette = theme.palette.clone();
        let size = theme.size;
        let gap = size.gap();

        let content = ui.max_rect().shrink(gap * 2.0);
        ui.painter().rect_filled(ui.max_rect(), 0.0, palette.background);

        let (header, body) = peel(content, Edge::Top, size.value_font() + gap * 2.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(header), |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new("Noda Audio")
                        .font(FontId::proportional(size.value_font()))
                        .color(palette.primary),
                );
                ui.separator();
                if ui.checkbox(&mut self.dark, "Dark").changed() {
                    self.theme = if self.dark { Theme::dark() } else { Theme::light() };
                    self.theme.install(ui.ctx());
                }
                ui.checkbox(&mut self.segmented, "Segments");
                ui.checkbox(&mut self.show_strip, "ChannelStrip");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let on = ctx.test_signal.value();
                    if ui
                        .selectable_label(on, "Test signal")
                        .on_hover_text("Drive the meters with a swept internal tone")
                        .clicked()
                    {
                        ctx.test_signal.set_value(!on);
                    }
                });
            });
        });

        // Meter smoothing is wall-clock driven, so the bars must keep being
        // redrawn between gestures or they freeze mid-release.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

        let style = if self.segmented { MeterStyle::Segmented } else { MeterStyle::Solid };
        let levels = levels_of(ctx);

        let mut gain_db = ctx.gain.raw_target() as f32;
        let mut balance = ctx.pan.raw_target() as f32;
        let input_meter = LevelMeter::new(&levels[0..2])
            .style(style)
            .scale(true)
            .clip(true)
            .id(egui::Id::new("demo-input-meter"));
        if self.show_strip {
            let mut muted = ctx.mute.value();
            let mut solo = false;
            let initial_muted = muted;
            let initial_gain = gain_db;
            let initial_pan = balance;

            let input_meter = LevelMeter::new(&levels[0..2])
                .style(style)
                .scale(true)
                .clip(true)
                .id(egui::Id::new("demo-input-meter"));
            let output_meter = LevelMeter::new(&levels[2..4])
                .style(style)
                .scale(true)
                .readout(true)
                .clip(true)
                .id(egui::Id::new("demo-output-meter"));

            let input_w = input_meter.desired_size(&theme).x;
            let output_w = output_meter.desired_size(&theme).x;
            let strip_w = 110.0;
            let gutter = gap * 4.0;
            let total_w = input_w + strip_w + output_w + gutter * 2.0;
            let start_x = (body.center().x - total_w * 0.5).max(body.left());

            // 1. Input meter
            let in_rect = Rect::from_min_size(
                pos2(start_x, body.top() + size.label_font() + gap),
                Vec2::new(input_w, body.height() - (size.label_font() + gap)),
            );
            ui.painter().text(
                pos2(in_rect.center().x, body.top() + size.label_font() * 0.5),
                Align2::CENTER_CENTER,
                "INPUT",
                FontId::proportional(size.label_font()),
                palette.text_dim,
            );
            ui.put(in_rect, input_meter);

            // 2. ChannelStrip
            let strip_x = start_x + input_w + gutter;
            let strip_rect = Rect::from_min_size(
                pos2(strip_x, body.top()),
                Vec2::new(strip_w, body.height()),
            );
            let strip = ChannelStrip::new("Track 1")
                .number(1)
                .accent(palette.meter_ok)
                .variant(ChannelStripVariant::Card)
                .meter_style(style)
                .gain(&mut gain_db)
                .pan(&mut balance)
                .mute(&mut muted)
                .solo(&mut solo)
                .levels(&levels[2..4]);
            let strip_resp = ui.put(strip_rect, strip);

            if muted != initial_muted {
                ctx.mute.set_value(muted);
            }
            if (gain_db - initial_gain).abs() > 1e-4 {
                publish(ctx, &ctx.gain, gain_db.clamp(-60.0, 12.0), &strip_resp);
            }
            if (balance - initial_pan).abs() > 1e-4 {
                publish(ctx, &ctx.pan, balance.clamp(-1.0, 1.0), &strip_resp);
            }

            // 3. Output Master meter
            let out_x = strip_x + strip_w + gutter;
            let out_rect = Rect::from_min_size(
                pos2(out_x, body.top() + size.label_font() + gap),
                Vec2::new(output_w, body.height() - (size.label_font() + gap)),
            );
            ui.painter().text(
                pos2(out_rect.center().x, body.top() + size.label_font() * 0.5),
                Align2::CENTER_CENTER,
                "MASTER",
                FontId::proportional(size.label_font()),
                palette.text_dim,
            );
            ui.put(out_rect, output_meter);
        } else {
            let output_meter = LevelMeter::new(&levels[2..4])
                .style(style)
                .scale(true)
                .readout(true)
                .clip(true)
                .id(egui::Id::new("demo-output-meter"));
            let fader = Fader::new(&mut gain_db)
                .range(-60.0, 12.0)
                .taper(Taper::AUDIO)
                .id(egui::Id::new("demo-gain"));
            let knob = Knob::pan(&mut balance).label("Balance").id(egui::Id::new("demo-pan"));

            // One column per control, each as wide as its widget asks for and
            // the group centered, so the editor reads as one lane of a console.
            let caption = size.label_font() + gap;
            let widths = [
                input_meter.desired_size(&theme).x,
                fader.desired_size(&theme).x,
                knob.desired_size(&theme).x,
                output_meter.desired_size(&theme).x,
            ];
            let gutter = gap * 3.0;
            let group = widths.iter().sum::<f32>() + gutter * (widths.len() as f32 - 1.0);
            let mut columns = Vec::with_capacity(widths.len());
            let mut x = body.center().x - group / 2.0;
            for width in widths {
                let right = x + width;
                columns.push(Rect::from_min_max(pos2(x, body.top()), pos2(right, body.bottom())));
                x = right + gutter;
            }

            for (name, column) in ["INPUT", "GAIN", "PAN", "OUTPUT"].into_iter().zip(&columns) {
                ui.painter().text(
                    pos2(column.center().x, column.top() + size.label_font() * 0.5),
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(size.label_font()),
                    palette.text_dim,
                );
            }

            // `Ui::put` gives a widget the whole rect, which is what a meter or
            // a fader wants: their own layout then peels scale, lamp and readout
            // off it.
            let track = |column: &Rect| column.with_min_y(column.top() + caption);
            ui.put(track(&columns[0]), input_meter);
            let gain = ui.put(track(&columns[1]), fader);
            let pan = ui.put(track(&columns[2]), knob);
            ui.put(track(&columns[3]), output_meter);

            publish(ctx, &ctx.gain, gain_db.clamp(-60.0, 12.0), &gain);
            publish(ctx, &ctx.pan, balance.clamp(-1.0, 1.0), &pan);
        }
    }
}

/// Hand a widget's value back to the host.
///
/// The edit bracket matters: a drag is one gesture to a DAW's automation
/// lane, not sixty separate writes, so `begin_edit` opens it and
/// `end_edit` closes it. The plain value is normalized through the range
/// the derive built, which is where a skewed or logarithmic law gets its
/// taper back.
fn publish(
    ctx: &PluginContext<DemoParams>,
    param: &FloatParam,
    plain: f32,
    response: &Response,
) {
    if !response.changed() {
        return;
    }
    let id = param.id();
    if response.drag_started() {
        ctx.begin_edit(id);
    }
    ctx.set_param(id, param.info.range.normalize(f64::from(plain)));
    if response.drag_stopped() {
        ctx.end_edit(id);
    }
}

/// The four channels the editor shows: input pair, then output pair.
fn levels_of(ctx: &PluginContext<DemoParams>) -> [ChannelLevel; 4] {
    let input = |slot: &MeterSlot| ChannelLevel::from_linear(ctx.get_meter(slot.id()));
    let output = |peak: &MeterSlot, rms: &MeterSlot| {
        ChannelLevel::from_linear(ctx.get_meter(peak.id()))
            .rms(gain_to_db(ctx.get_meter(rms.id())))
    };
    [
        input(&ctx.input_left),
        input(&ctx.input_right),
        output(&ctx.output_left, &ctx.rms_left),
        output(&ctx.output_right, &ctx.rms_right),
    ]
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const FRAMES: usize = 64;

    /// One block through the demo, with the meter lane captured.
    fn run(
        mut state: Demo,
        gain_db: f64,
        balance: f64,
        test_signal: bool,
        input: [&[f32]; 2],
    ) -> (Vec<(u32, f32)>, [Vec<f32>; 2]) {
        let params = DemoParams::default();
        // Target and smoothed value both set: the test wants the gain the
        // name says, not wherever the 5 ms smoother has got to by block 64.
        params.gain.set_value(gain_db);
        params.gain.smoother.snap(gain_db);
        params.pan.set_value(balance);
        params.pan.smoother.snap(balance);
        params.test_signal.set_value(test_signal);

        let mut output = [vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]];
        let mut outs: Vec<&mut [f32]> = output.iter_mut().map(|ch| ch.as_mut_slice()).collect();
        let mut buffer = AudioBuffer::from_slices_checked(&input[..], &mut outs, FRAMES);

        let transport = TransportInfo::for_screenshot();
        let mut sent = EventList::default();
        let incoming = EventList::default();
        let meters = RefCell::new(Vec::new());
        {
            let observe = |id: u32, value: f32| {
                meters.borrow_mut().push((id, value));
            };
            let mut context = ProcessContext::new(&transport, 48000.0, FRAMES, &mut sent)
                .with_meters(&observe);
            Demo::process(&mut state, &params, &mut buffer, &incoming, &mut context);
        }
        (meters.into_inner(), output)
    }

    /// Every frame of a channel at `expected`, to within a float's rounding.
    fn assert_close(channel: &[f32], expected: f32) {
        assert!(
            channel.iter().all(|sample| (sample - expected).abs() < 1e-6),
            "expected {expected} throughout, got {channel:?}"
        );
    }

    #[test]
    fn unity_gain_at_centre_splits_the_signal_equal_power() {
        let left = [0.5f32; FRAMES];
        let right = [-0.25f32; FRAMES];
        // Hard left would mute the right channel, so the pan law has to be
        // undone before the signal can be expected to arrive unchanged.
        let (left_gain, right_gain) = pan_gains(0.0);
        let (_, output) = run(Demo::default(), 0.0, 0.0, false, [&left, &right]);
        assert_close(&output[0], 0.5 * left_gain);
        assert_close(&output[1], -0.25 * right_gain);
    }

    #[test]
    fn the_gain_param_is_read_in_decibels() {
        let half = db_to_gain(-6.0) * pan_gains(0.0).0;
        let left = [1.0f32; FRAMES];
        let (_, output) = run(Demo::default(), -6.0, 0.0, false, [&left, &left]);
        assert!((output[0][0] - half).abs() < 1e-6, "got {}", output[0][0]);
    }

    #[test]
    fn the_pan_law_is_a_quarter_turn_and_holds_its_power() {
        let (left, right) = pan_gains(0.0);
        assert!((left - right).abs() < 1e-6, "centre is symmetric");
        assert!((left.mul_add(left, right * right) - 1.0).abs() < 1e-6, "constant power");
        let (full_left, _) = pan_gains(-1.0);
        assert!((full_left - 1.0).abs() < 1e-6);
        let (_, quiet_right) = pan_gains(-1.0);
        assert!(quiet_right.abs() < 1e-6, "hard left leaves nothing on the right");
    }

    #[test]
    fn every_channel_reports_onto_its_own_slots() {
        let left = [0.5f32; FRAMES];
        let right = [-0.125f32; FRAMES];
        // Hard left: the left channel keeps its level and the right is
        // silenced, so each lane can be read on its own slots.
        let (meters, _) = run(Demo::default(), 0.0, -1.0, false, [&left, &right]);
        let params = DemoParams::default();
        let read = |slot: &MeterSlot| {
            meters.iter().find(|(id, _)| *id == slot.id()).map(|(_, value)| *value)
        };
        assert_eq!(read(&params.input_left), Some(0.5));
        assert_eq!(read(&params.output_left), Some(0.5));
        assert_eq!(read(&params.input_right), Some(0.125), "the source is measured before the pan");
        assert_eq!(read(&params.output_right), Some(0.0));
        assert_eq!(read(&params.rms_right), Some(0.0));
        assert_eq!(meters.len(), 6, "one peak pair, one output pair, one rms pair");
    }

    #[test]
    fn a_hot_channel_reads_full_scale() {
        let loud = [2.0f32; FRAMES];
        let (meters, output) = run(Demo::default(), 0.0, -1.0, false, [&loud, &loud]);
        assert_eq!(output[0][0], 2.0, "the plugin stays honest about clipping");
        let params = DemoParams::default();
        let peak = meters.iter().find(|(id, _)| *id == params.output_left.id()).unwrap().1;
        assert_eq!(peak, 1.0, "levels publish clamped to the 0..1 lane");
    }

    #[test]
    fn the_test_tone_sweeps_up_from_silence() {
        let quiet = [0.0f32; FRAMES];
        let (_meters, output) = run(Demo::default(), 0.0, 0.0, true, [&quiet, &quiet]);
        assert!(
            output[0].iter().all(|s| s.abs() < 1e-3),
            "the sweep starts at rest, so the first block must be near silent"
        );

        // A quarter of the way round the envelope the tone is well up.
        let mid = Demo { clock: SWEEP_SECONDS / 4.0, ..Demo::default() };
        let (meters, output) = run(mid, 0.0, -1.0, true, [&quiet, &quiet]);
        let params = DemoParams::default();
        let peak = meters.iter().find(|(id, _)| *id == params.input_left.id()).unwrap().1;
        assert!(output[0].iter().any(|s| *s > 0.1), "the tone must reach audible level");
        assert!(peak > 0.1, "and report it on the meter lane");
    }
}
