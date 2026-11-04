//! Saturation and distortion UI: character selection, parameter banks,
//! the static transfer curve, harmonic content and clip lamps.

pub mod controls;
pub mod curve;
pub mod harmonics;
pub mod indicators;
pub mod type_select;

pub use controls::{DistortionControls, SaturatorControls};
pub use curve::{SaturationCurve, SaturationType};
pub use harmonics::{
    HarmonicsSpectrum, MAX_HARMONICS, MIN_HARMONIC_DB, harmonic_levels, thd_percent,
};
pub use indicators::ClipIndicators;
pub use type_select::SaturationTypeSelect;
