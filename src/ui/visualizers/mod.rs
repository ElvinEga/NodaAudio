//! The visualizer family: widgets that draw a signal's shape or spectrum.

pub mod bar_levels;
pub mod bar_visualizer;
pub mod electric_bar_visualizer;
pub mod electric_scene;
pub mod electric_waveform;
pub mod line_paint;
pub mod live_waveform;
pub mod smooth_waveform;
pub mod spectrum;
pub mod wave_line;
pub mod waveform;

pub use bar_levels::{
    BarIdle, BarLevelsFrame, BarLevelsOptions, BarLevelsState, idle_level, sweep_level,
};
pub use bar_visualizer::{BarAlign, BarVisualizer};
pub use electric_scene::{
    ElectricArc, ElectricLayout, ElectricLayoutOptions, ElectricSceneOptions, ElectricSceneState,
    layout_electric_bars,
};
pub use wave_line::{WaveLineMode, WaveLineOptions, WaveLineState};
pub use electric_bar_visualizer::{ElectricBarVisualizer, pseudo_random};
pub use electric_waveform::{ElectricWaveform, fractal_displace};
pub use live_waveform::{LiveWaveform, LiveWaveformMode, LiveWaveformVariant};
pub use smooth_waveform::{SmoothWaveform, SmoothWaveformMode, trigger_index};
pub use spectrum::{
    Spectrum, SpectrumVariant, bands_from_spectrum, log_band_edges, resample_levels,
};
pub use waveform::{
    Waveform, WaveformMarker, WaveformRegion, WaveformVariant, format_time,
};
