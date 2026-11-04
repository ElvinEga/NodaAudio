# NodaAudio

Audio UI components for [`egui`], written for [`truce`] plugins and the
standalone apps around them: level meters, faders, knobs, dB scales and clip
lights that speak in real engineering units instead of the 0..1 normalized
floats most widget kits hand you.

The components depend on egui alone. They draw the numbers they are handed
and open no device, so the same `LevelMeter` serves a plugin reading its
`process` peak, a standalone app measuring its own buffer, and a unit test
feeding it a sine sweep. The demo plugin in `src/lib.rs` is the other half: a
worked example of hosting them against a real parameter and meter bridge.

```text
 audio thread          GUI thread
 process()   ──set_meter──▶   LevelMeter::stereo(&levels)
             ◀──set_param──   Fader::new(&mut db)
```

## Running the demo

The demo is a complete truce plugin, so it runs as a standalone app, as a
CLAP or VST3 inside a host, and renders headless screenshots.

```sh
cargo truce run                 # build the .app bundle and open it
cargo truce run --debug         # skip the release build while iterating
cargo truce run -- --headless   # no window: device probe and transport only
cargo truce run -- --list-devices
cargo truce run -- --input-file in.wav --output-file out.wav --no-playback
cargo truce install             # install CLAP and VST3 into the system paths
cargo truce screenshot --out /tmp/noda.png --debug
```

`cargo truce run` needs an audio device unless you render a file: the flags
after `--` go to the standalone host (`--output`, `--input`, `--sample-rate`,
`--buffer`, `--bus-layout`, `--state`, `--preset`, `-v`). `cargo truce
screenshot` drives the same editor without a device — it runs one frame with
the transport stopped and writes a PNG, which is the fastest way to see a
layout change.

`cargo truce install` writes into the machine's plugin folders, so a host
rescan will find "Noda Audio Demo". Metadata comes from `truce.toml`.

## The showcase window

`examples/showcase.rs` is a plain eframe app — no truce, no audio device —
that gathers every component in one scrollable gallery and wires it to a
`Source` that fakes what a `process` call would measure: pulsing levels, peak
hold, rms, an over-load blip. It is the fastest way to compare the widgets
against each other and to watch theme, size, law and ballistics settings move
them all at once.

```sh
cargo run --example showcase          # build and open the gallery
cargo test --example showcase         # 3 headless paint tests
```

The faders, meters, knobs, dB scales and clip lamps each get a section, and
the last section composes them into two mixer strips feeding a master meter.
eframe is a dev-dependency for this file only; the library itself still
depends on egui alone.

## Using the components

Every widget is a builder over a value it was handed. Nothing in `src/ui/`
knows about truce.

```rust
use noda_audio::ui::{ChannelLevel, Fader, Knob, LevelMeter, theme::Theme};
use noda_audio::taper::Taper;

// A theme installed once per context makes egui's own widgets match.
theme.install(ui.ctx());

let levels = [
    ChannelLevel::from_linear(ctx.get_meter(ctx.input_left.id())),
    ChannelLevel::from_linear(ctx.get_meter(ctx.input_right.id())),
];
ui.add(LevelMeter::stereo(&levels).scale(true).clip(true));

// A fader over a dB range, on the console law, with detents at the marks.
let mut gain_db = ctx.gain.raw_target() as f32;
if ui.add(Fader::new(&mut gain_db).range(-60.0, 12.0).taper(Taper::AUDIO)).changed() {
    ctx.gain.set_value(gain_db as f64);
}

// A centre-detented balance knob that reads 30L / C / 50R.
let mut balance = ctx.pan.raw_target() as f32;
ui.add(Knob::pan(&mut balance).label("Balance"));
```

A drag is one gesture to a host, not sixty writes, so bracket it:

```rust
let response = ui.add(fader);
if response.changed() {
    if response.drag_started() { ctx.begin_edit(id); }
    ctx.set_param(id, ctx.gain.info.range.normalize(f64::from(gain_db)));
    if response.drag_stopped() { ctx.end_edit(id); }
}
```

A meter or a fader that owns its own strip — a scale beside a bar, a lamp
above a track — is laid out by peeling rects off the rect it was given, and
`Ui::put` hands a widget the whole rect to do it with:

```rust
let meter = LevelMeter::new(&levels).scale(true);
ui.put(track_column, meter);
```

## What is here

### Meters & Visualizers
| Module | Component | Description |
| --- | --- | --- |
| [`ui/meters/vu_meter.rs`](src/ui/meters/vu_meter.rs) | `VuMeter` | Classic analog moving-coil VU meter with ANSI C16.5 ballistics (300ms rise time), vintage parchment & modern dark skins, and needle shadow |
| [`ui/meters/gr_meter.rs`](src/ui/meters/gr_meter.rs) | `GainReductionMeter` | Downward attenuation gain reduction meter (0 to -24/-30 dB) with peak hold marker, mono/stereo layouts, and scale ticks |
| [`ui/meters/level_meter.rs`](src/ui/meters/level_meter.rs) | `LevelMeter` | Peak + RMS, broadcast zones, peak hold, clip indicator, stereo/multichannel, vertical or horizontal |
| [`ui/meters/lcd_meter.rs`](src/ui/meters/lcd_meter.rs) | `LcdMeter` | Vintage dot-matrix LCD level meter with ghost unlit segments and peak hold |
| [`ui/meters/db_scale.rs`](src/ui/meters/db_scale.rs) | `DbScale` | Engineering dB graduation strip with dynamic label thinning by print rank |
| [`ui/meters/db_readout.rs`](src/ui/meters/db_readout.rs) | `DbReadout` | Standalone dB text readout with color-by-zone, sign policies, and fixed monospace layout |
| [`ui/meters/clip_indicator.rs`](src/ui/meters/clip_indicator.rs) | `ClipIndicator` | Over-load detector with latching peak count and reset policy |
| [`ui/visualizers/spectrum.rs`](src/ui/visualizers/spectrum.rs) | `Spectrum` | Real-time FFT spectrum analyzer with log bands, frequency grid (100Hz/1kHz/10kHz), peak hold markers, Bars/Line/Area variants |
| [`ui/visualizers/bar_visualizer.rs`](src/ui/visualizers/bar_visualizer.rs) | `BarVisualizer` | Multi-bar visualizer with Center/Start/End alignment, center-mirroring, Static/Pulse/Wave idle animations, Gaussian sweep loading, and audiocn's 0.86/frame release |
| [`ui/visualizers/electric_bar_visualizer.rs`](src/ui/visualizers/electric_bar_visualizer.rs) | `ElectricBarVisualizer` | High-voltage filament scene with midpoint-displacement crackle, glow/sheath/core passes, inter-bar arcs, and rising-level sparks |
| [`ui/visualizers/electric_waveform.rs`](src/ui/visualizers/electric_waveform.rs) | `ElectricWaveform` | Crackling electric trace over the wave/scope line, with branched forks, sparks, and a soft blurred glow |
| [`ui/visualizers/smooth_waveform.rs`](src/ui/visualizers/smooth_waveform.rs) | `SmoothWaveform` | Harmonic wave synthesizer (with phase drift) and zero-crossing synchronized oscilloscope mode |
| [`ui/visualizers/live_waveform.rs`](src/ui/visualizers/live_waveform.rs) | `LiveWaveform` | Live buffer scrolling timeline visualizer with Bars, Mirror, and Line display modes |
| [`ui/visualizers/waveform.rs`](src/ui/visualizers/waveform.rs) | `Waveform` | Audio clip waveform with played/unplayed dual-pass split, scrubber seek bar, cue markers, loop regions, and hover badge |

The visualizers share [`bar_levels.rs`](src/ui/visualizers/bar_levels.rs) (release/idle/loading state), [`wave_line.rs`](src/ui/visualizers/wave_line.rs) (wave and scope lines), [`electric_scene.rs`](src/ui/visualizers/electric_scene.rs) (filament layout, arcs, sparks), and [`line_paint.rs`](src/ui/visualizers/line_paint.rs) (faded strokes and glow ribbons).

### Pro Plugin & EQ Components
| Module | Component | Description |
| --- | --- | --- |
| [`ui/eq/eq_curve.rs`](src/ui/eq/eq_curve.rs) | `EqCurve` | Interactive FabFilter Pro-Q style frequency response display with log grid (20Hz..20kHz), draggable band handles, mouse-wheel Q, and live spectrum backdrop |
| [`ui/eq/eq_band.rs`](src/ui/eq/eq_band.rs) | `EqBand` | Parametric EQ band with exact closed-form biquad magnitude transfer function calculation |
| [`ui/eq/filter_selector.rs`](src/ui/eq/filter_selector.rs) | `FilterTypeSelector` | Segmented filter type selector with Bell, LowCut, HighCut, LowShelf, HighShelf, Notch, BandPass, and TiltShelf shapes |
| [`ui/dynamics/compressor_curve.rs`](src/ui/dynamics/compressor_curve.rs) | `CompressorCurve` | Interactive compressor transfer curve (input vs output dBFS), soft-knee quadratic interpolation, unity diagonal, GR fill, and live tracking dot |
| [`ui/dynamics/dynamics_controls.rs`](src/ui/dynamics/dynamics_controls.rs) | `CompressorControls` | Parameter control banks for compressors (Threshold, Ratio, Attack, Release, Knee, Makeup, Sidechain), limiters (Ceiling, Release, True Peak), and noise gates |
| [`ui/saturation/curve.rs`](src/ui/saturation/curve.rs) | `SaturationCurve` | Waveshaping transfer function visualizer $y=f(x)$ supporting Tape, Tube, Soft Tanh, Hard Clip, Foldback wavefolding, and Fuzz, with live input marker |
| [`ui/saturation/type_select.rs`](src/ui/saturation/type_select.rs) | `SaturationTypeSelect` | Grid selector for the waveshaper character, with short or full names |
| [`ui/saturation/controls.rs`](src/ui/saturation/controls.rs) | `SaturatorControls`, `DistortionControls` | Drive/Mix/Tone/Bias saturator bank, and Drive/Type/Tone/Output distortion bank |
| [`ui/saturation/harmonics.rs`](src/ui/saturation/harmonics.rs) | `HarmonicsSpectrum` | Harmonic bar spectrum computed from the real waveshaper with THD and per-partial dB labels |
| [`ui/saturation/indicators.rs`](src/ui/saturation/indicators.rs) | `ClipIndicators` | Soft and hard clip lamps driven by the selected shaper, drive and bias |
| [`ui/plugin/rack_frame.rs`](src/ui/plugin/rack_frame.rs) | `RackFrame` | Hardware 19" rack mount chassis with brushed metal finish, 4 corner crosshead mounting screws, status/power LED, and title badge |
| [`ui/plugin/header_bar.rs`](src/ui/plugin/header_bar.rs) | `PluginHeaderBar` | Standard pro audio plugin top toolbar with A/B comparison, oversampling (1x..8x), input/output gains with stereo mini peak meters, and global bypass |
| [`ui/plugin/preset_browser.rs`](src/ui/plugin/preset_browser.rs) | `PresetBrowser` | Preset manager with category badges, preset name display, cycling `<` `>` stepping arrows, and save callback |
| [`ui/plugin/section_header.rs`](src/ui/plugin/section_header.rs) | `SectionHeader` | Collapsible parameter group divider with title badge, on/off enable switch, and horizontal accent line |
| [`ui/plugin/value_popup.rs`](src/ui/plugin/value_popup.rs) | `ValuePopup` | Floating value bubble above a dragged control, plus the `show_drag_value` helper |

### Controls
| Module | Component | Description |
| --- | --- | --- |
| [`ui/controls/metallic_knob.rs`](src/ui/controls/metallic_knob.rs) | `MetallicKnob` | Hi-Fi brushed aluminium volume dial and mini metal knob with anisotropic conic sheen, 100-tick scale with rotated numbers, and active lighting |
| [`ui/controls/fader.rs`](src/ui/controls/fader.rs) | `Fader` | Professional dB vertical fader with audio/linear/log tapers, magnetic detents, Iso/Plain styles, and keyboard nav |
| [`ui/controls/knob.rs`](src/ui/controls/knob.rs) | `Knob` | 270° rotary knob with bipolar/unipolar fills, tick graduations, magnetic center detents, and direct text editing |
| [`ui/controls/pan_control.rs`](src/ui/controls/pan_control.rs) | `PanControl` | Horizontal bipolar slider with center snap, magnetic detents, and balance readout |
| [`ui/controls/parameter_slider.rs`](src/ui/controls/parameter_slider.rs) | `ParameterSlider` | DAW parameter slider with label, unit suffix, linear/log tapers, and inline click-to-edit |
| [`ui/controls/volume_control.rs`](src/ui/controls/volume_control.rs) | `VolumeControl` | Normalized 0..1 volume slider with integrated mute button and dynamic speaker icon |
| [`ui/controls/channel_toggle.rs`](src/ui/controls/channel_toggle.rs) | `ChannelToggle` | Mute, Solo, Monitor, and Neutral channel switches with distinct active tone highlights |
| [`ui/controls/lcd_display.rs`](src/ui/controls/lcd_display.rs) | `LcdDisplay` | Hardware-bezel LCD screen container with scanlines, glass sheen, dot matrix, and Primary/Amber/Green/Cyan/Red inks |
| [`ui/controls/audio_device_select.rs`](src/ui/controls/audio_device_select.rs) | `AudioDeviceSelect` | Audio input/output device selector with default badge and availability states |

### Mixer
| Module | Component | Description |
| --- | --- | --- |
| [`ui/mixer/channel_strip.rs`](src/ui/mixer/channel_strip.rs) | `ChannelStrip` | Mixing console channel strip or track row composing meter, fader, pan, mute/solo toggles, track badge, and dB readout |
| [`ui/mixer/mixer.rs`](src/ui/mixer/mixer.rs) | `Mixer` | Multi-channel mixer container with horizontal/vertical layouts, exclusive solo logic, mute dimming, and dedicated master bus section |
| [`ui/mixer/bus_strip.rs`](src/ui/mixer/bus_strip.rs) | `BusStrip` | Group bus row with member count, routing caption, stereo meter, audio-taper gain, mute/solo and an accent edge bar |
| [`ui/mixer/master_section.rs`](src/ui/mixer/master_section.rs) | `MasterSection` | Wide master output with large gain readout, scaled stereo meter with clip lamps, fader, and MUTE/DIM/MONO switches |

### Media & Music
| Module | Component | Description |
| --- | --- | --- |
| [`ui/media/audio_player.rs`](src/ui/media/audio_player.rs) | `AudioPlayer` | Full transport toolbar with play/pause, seek scrubber, time elapsed/remaining, speed multipliers (0.5x..2x), volume, and loop |
| [`ui/media/track_list.rs`](src/ui/media/track_list.rs) | `TrackList` | Multi-track playlist queue with active/playing state indicators, duration readouts, and track selection |
| [`ui/media/sound_pad.rs`](src/ui/media/sound_pad.rs) | `SoundPadGrid` | MPC / Launchpad trigger pad matrix with OneShot, Toggle, Hold, and Loop modes, hotkey badges, and trigger glow halos |

### Composite Blocks
| Module | Component | Description |
| --- | --- | --- |
| [`ui/blocks/mic_setup.rs`](src/ui/blocks/mic_setup.rs) | `MicSetupBlock` | Microphone setup card combining device selector, live waveform preview, horizontal meter, input gain slider, and automated diagnostic level check |
| [`ui/blocks/music_player.rs`](src/ui/blocks/music_player.rs) | `MusicPlayerBlock` | Complete audio workstation player card integrating transport controls, interactive waveform with cue markers and loop regions, collapsible FFT spectrum analyzer, and playlist queue |
| [`ui/blocks/soundboard.rs`](src/ui/blocks/soundboard.rs) | `SoundboardBlock` | Live stream soundboard card with Panic Stop All, master volume control, customizable pad grid, and active trigger status |
| [`ui/blocks/quick_audio_popover.rs`](src/ui/blocks/quick_audio_popover.rs) | `QuickAudioPopover` | Compact system tray popover with dynamic speaker icon and 5-band mini visualizer, expanding into output device selector, volume slider, and level meter |
| [`ui/blocks/system_audio_mixer.rs`](src/ui/blocks/system_audio_mixer.rs) | `SystemAudioMixerBlock` | Multi-source broadcast audio desk card with layout orientation toggle (Console vs Horizontal), Reset Unity action, and multi-channel mixing desk |

### Foundation & DSP
| Module | Description |
| --- | --- |
| [`ui/theme.rs`](src/ui/theme.rs) | `Palette`, `Size`, `Theme`, `Radius`, `Preset` (Standard, Ocean, Mono, Amber, Violet) with full dark and light mode support |
| [`oklab.rs`](src/oklab.rs) | Perceptual OKLab & OKLCH color maths matching CSS `color-mix` and hardware LCD screen inks |
| [`db.rs`](src/db.rs) | dBFS conversions, typographic minus (`\u{2212}`), digital silence preservation, and formatting |
| [`taper.rs`](src/taper.rs) | Transfer curves: linear, logarithmic, and broadcast audio console law |
| [`ballistics.rs`](src/ballistics.rs) | Attack / release envelope smoothing, peak hold decay, and idle gating |
| [`zones.rs`](src/zones.rs) | EBU / ITU broadcast metering zones and threshold color dispatch |

## Design rules the code keeps to

- **Units are decibels.** A widget takes dBFS and a range; the conversions to
  the 0..1 lanes a host or a meter slot uses happen at the boundary, in the
  call site, where they can be read.
- **Digital silence is a real value.** `-inf` survives the conversions and
  prints as `-inf`; `NaN` prints as `--` so an unmeasured meter never claims
  to be silent.
- **Components never touch audio.** No device, no clock, no state that only
  makes sense inside a `process` call. Ballistics are stepped with a time the
  caller supplies.
- **Everything is keyboard reachable where the widget takes input**: arrows
  nudge, Page keys jump, Home and End go to the stops, a double-click or a
  secondary click returns to the reset value, and the readout takes typed
  text.
- **Theme is data, not CSS.** Colors and sizes come from `Theme`; a plugin
  that wants another look sets a palette and installs it.

## Tests

```sh
cargo test --lib                # 228 unit tests: units, laws, ballistics, layout, rendering
cargo test --doc                # 24 documentation tests
cargo test --example showcase   # gallery paint tests (3 passing)
cargo check --all-targets       # 0 warnings policy
```

The rendering tests drive a real `egui::Context` headlessly and inspect the
shapes it emits, so a zone color or an arc that stopped being drawn fails a
test rather than a screenshot review.

[`egui`]: https://github.com/emilk/egui
[`truce`]: https://crates.io/crates/truce
