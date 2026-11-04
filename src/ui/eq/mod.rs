//! Professional parametric equalizer components (interactive frequency response, biquad bands, handles, and filter types).

pub mod eq_band;
pub mod eq_curve;
pub mod filter_selector;
pub mod filter_type;

pub use eq_band::EqBand;
pub use eq_curve::{DEFAULT_EQ_RANGE_DB, DEFAULT_SAMPLE_RATE, EqCurve, MAX_FREQ_HZ, MIN_FREQ_HZ};
pub use filter_selector::FilterTypeSelector;
pub use filter_type::{FilterSlope, FilterType};
