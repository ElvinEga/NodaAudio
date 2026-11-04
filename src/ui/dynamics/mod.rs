//! Dynamics processor UI: compressor transfer curves and parameter banks.
//!
//! Saturation and distortion live in [`crate::ui::saturation`].

pub mod compressor_curve;
pub mod dynamics_controls;

pub use compressor_curve::CompressorCurve;
pub use dynamics_controls::{CompressorControls, GateControls, LimiterControls};
