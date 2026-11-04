//! egui widgets for audio: meters, faders, knobs and their supporting
//! parts.
//!
//! Every widget here is **data-driven**. It draws what it is handed and
//! never opens a device, reads a clock, or owns signal. A plugin supplies
//! levels from its `process` call; a test or a demo app can supply them
//! from a sine wave. The same widget works either way.

pub mod blocks;
pub mod controls;
pub mod dynamics;
pub mod eq;
pub mod media;
pub mod meters;
pub mod mixer;
pub mod plugin;
pub mod saturation;
pub mod theme;
pub mod visualizers;

pub use blocks::{
    MicCheckResult, MicSetupBlock, MusicPlayerBlock, QuickAudioPopover, SoundboardBlock,
    SystemAudioMixerBlock,
};

pub use controls::{
    AudioDevice, AudioDeviceSelect, ChannelToggle, ChannelToggleTone, DragMode, Fader,
    FaderStyle, Knob, LcdAppearance, LcdContext, LcdDisplay, LcdDisplayRow, LcdScreen, LcdSize,
    LcdStatus, LcdVariant, MetallicKnob, MetallicKnobVariant, PanControl, ParameterSlider, Reading,
    VolumeControl, VolumeCurve, VolumeLevel, format_pan, installed, paint_separator, parse_pan,
};
pub use dynamics::{CompressorControls, CompressorCurve, GateControls, LimiterControls};
pub use eq::{
    DEFAULT_EQ_RANGE_DB, DEFAULT_SAMPLE_RATE, EqBand, EqCurve, FilterSlope, FilterType,
    FilterTypeSelector, MAX_FREQ_HZ, MIN_FREQ_HZ,
};
pub use media::{
    AudioPlayer, PLAYBACK_RATES, SoundPadGrid, SoundPadItem, SoundPadMode, TrackItem,
    TrackList,
};
pub use meters::{
    ChannelLevel, ClipIndicator, ClipLatch, ClipPolicy, DEFAULT_GR_MAX_DB, DEFAULT_TICKS,
    DEFAULT_VU_HEADROOM_DB, DbReadout, DbScale, GainReductionMeter, LcdMeter, LevelMeter,
    MarkEdge, MeterStyle, VuAppearance, VuMeter, print_magnitude,
};
pub use mixer::{
    BusStrip, ChannelStrip, ChannelStripVariant, MasterSection, Mixer, MixerChannelData,
    MixerMasterData,
};
pub use plugin::{
    AbState, Oversampling, POPUP_GAP, PluginHeaderBar, Preset, PresetBrowser, RackFrame, RackStyle,
    SectionHeader, ValuePopup, draw_value_popup, popup_rect, show_drag_value,
    show_drag_value_labeled,
};
pub use saturation::{
    ClipIndicators, DistortionControls, HarmonicsSpectrum, MAX_HARMONICS, MIN_HARMONIC_DB,
    SaturationCurve, SaturationType, SaturationTypeSelect, SaturatorControls, harmonic_levels,
    thd_percent,
};
pub use theme::{Palette, Size, Theme};
pub use visualizers::{
    BarAlign, BarIdle, BarVisualizer, ElectricBarVisualizer, ElectricWaveform,
    LiveWaveform, LiveWaveformMode, LiveWaveformVariant, SmoothWaveform,
    SmoothWaveformMode, Spectrum, SpectrumVariant, Waveform, WaveformMarker,
    WaveformRegion, WaveformVariant, bands_from_spectrum, format_time,
    fractal_displace, log_band_edges, pseudo_random, resample_levels, trigger_index,
};

/// Which way a widget runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Orientation {
    /// Bottom to top, as on a console.
    Vertical,
    /// Left to right, as on a track.
    Horizontal,
}

impl Orientation {
    /// Along the widget's travel.
    #[must_use]
    pub fn length(self, rect: egui::Rect) -> f32 {
        match self {
            Orientation::Vertical => rect.height(),
            Orientation::Horizontal => rect.width(),
        }
    }

    /// Across the widget.
    #[must_use]
    pub fn thickness(self, rect: egui::Rect) -> f32 {
        match self {
            Orientation::Vertical => rect.width(),
            Orientation::Horizontal => rect.height(),
        }
    }

    /// A strip `size` points deep, taken from the given rect along the
    /// axis the widget grows.
    #[must_use]
    pub fn strip(self, rect: egui::Rect, size: f32) -> egui::Rect {
        match self {
            Orientation::Vertical => {
                egui::Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top()),
                    egui::pos2(rect.left() + size, rect.bottom()),
                )
            }
            Orientation::Horizontal => {
                egui::Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top()),
                    egui::pos2(rect.right(), rect.top() + size),
                )
            }
        }
    }

    /// The coordinate along the travel, low to high from the widget's
    /// origin: bottom for a vertical bar, left for a horizontal one.
    #[must_use]
    pub fn along(self, rect: egui::Rect, position: f32) -> f32 {
        match self {
            Orientation::Vertical => rect.bottom() - position * rect.height(),
            Orientation::Horizontal => rect.left() + position * rect.width(),
        }
    }

    /// The axis a pointer must travel along to change the value.
    #[must_use]
    pub fn axis_delta(self, delta: egui::Vec2) -> f32 {
        match self {
            Orientation::Vertical => -delta.y,
            Orientation::Horizontal => delta.x,
        }
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Vertical
    }
}

/// A position along a control's travel, always 0..1 with 1 at the top or
/// the right.
pub type Position = f32;

/// Which edge of a rect a composite widget takes a strip from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Take a `size`-deep strip off one edge, returning the strip and what is
/// left of the rect. Meters and faders peel their scale, lamp and readout
/// off the same rect this way.
#[must_use]
pub(crate) fn peel(rect: egui::Rect, side: Edge, size: f32) -> (egui::Rect, egui::Rect) {
    // The cut line: where the strip ends and what is left of the rect begins.
    let (cut_top, cut_bottom) = (rect.top() + size, rect.bottom() - size);
    let (cut_left, cut_right) = (rect.left() + size, rect.right() - size);
    let strip = match side {
        Edge::Top => egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), cut_top)),
        Edge::Bottom => egui::Rect::from_min_max(egui::pos2(rect.left(), cut_bottom), rect.max),
        Edge::Left => egui::Rect::from_min_max(rect.min, egui::pos2(cut_left, rect.bottom())),
        Edge::Right => egui::Rect::from_min_max(egui::pos2(cut_right, rect.top()), rect.max),
    };
    let rest = match side {
        Edge::Top => egui::Rect::from_min_max(egui::pos2(rect.left(), cut_top), rect.max),
        Edge::Bottom => egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), cut_bottom)),
        Edge::Left => egui::Rect::from_min_max(egui::pos2(cut_left, rect.top()), rect.max),
        Edge::Right => egui::Rect::from_min_max(rect.min, egui::pos2(cut_right, rect.bottom())),
    };
    (strip, rest)
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};

    use super::*;

    const TALL: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 200.0));
    const WIDE: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 10.0));

    #[test]
    fn length_and_thickness_follow_the_axis() {
        assert_eq!(Orientation::Vertical.length(TALL), 200.0);
        assert_eq!(Orientation::Vertical.thickness(TALL), 10.0);
        assert_eq!(Orientation::Horizontal.length(WIDE), 200.0);
        assert_eq!(Orientation::Horizontal.thickness(WIDE), 10.0);
    }

    #[test]
    fn positions_run_from_the_quiet_end() {
        assert_eq!(Orientation::Vertical.along(TALL, 0.0), 200.0);
        assert_eq!(Orientation::Vertical.along(TALL, 1.0), 0.0);
        assert_eq!(Orientation::Horizontal.along(WIDE, 0.5), 100.0);
    }

    #[test]
    fn dragging_up_increases_a_vertical_value() {
        let up = egui::vec2(0.0, -20.0);
        assert_eq!(Orientation::Vertical.axis_delta(up), 20.0);
        assert_eq!(Orientation::Horizontal.axis_delta(egui::vec2(20.0, 0.0)), 20.0);
    }

    #[test]
    fn peeling_a_strip_leaves_the_rest_contiguous() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let (strip, rest) = peel(rect, Edge::Top, 12.0);
        assert_eq!(strip.height(), 12.0);
        assert_eq!(rest.top(), 12.0);
        assert_eq!(rest.bottom(), 100.0);
        let (strip, rest) = peel(rect, Edge::Right, 20.0);
        assert_eq!(strip.left(), 80.0);
        assert_eq!(rest.right(), 80.0);
    }

    #[test]
    fn a_strip_is_taken_from_the_start() {
        let strip = Orientation::Vertical.strip(TALL, 28.0);
        assert_eq!(strip.width(), 28.0);
        assert_eq!(strip.height(), 200.0);
        let strip = Orientation::Horizontal.strip(WIDE, 16.0);
        assert_eq!(strip.width(), 200.0);
        assert_eq!(strip.height(), 16.0);
    }
}
