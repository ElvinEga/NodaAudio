//! The metering family: the parts that show a level rather than take one.

pub mod clip_indicator;
pub mod db_readout;
pub mod db_scale;
pub mod gr_meter;
pub mod lcd_meter;
pub mod level_meter;
pub mod vu_meter;

pub use clip_indicator::{ClipIndicator, ClipLatch, ClipPolicy};
pub use db_readout::DbReadout;
pub use db_scale::{DbScale, MarkEdge};
pub use gr_meter::{DEFAULT_GR_MAX_DB, GainReductionMeter};
pub use lcd_meter::{DEFAULT_TICKS, LcdMeter, print_magnitude};
pub use level_meter::{ChannelLevel, LevelMeter, MeterStyle};
pub use vu_meter::{DEFAULT_VU_HEADROOM_DB, VuAppearance, VuMeter};
