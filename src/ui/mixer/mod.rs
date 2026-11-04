//! The mixing desk: a strip that composes meters and controls, and a
//! container that arranges strips.

pub mod bus_strip;
pub mod channel_strip;
pub mod master_section;
pub mod mixer;

pub use bus_strip::BusStrip;
pub use channel_strip::{ChannelStrip, ChannelStripVariant};
pub use master_section::MasterSection;
pub use mixer::{Mixer, MixerChannelData, MixerMasterData};
