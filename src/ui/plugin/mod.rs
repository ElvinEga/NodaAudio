//! Professional audio plugin shell and utility components.
//!
//! Includes modular rack frames, preset browsers, header bars with A/B and oversampling,
//! parameter group section headers, and drag value popups.

pub mod header_bar;
pub mod preset_browser;
pub mod rack_frame;
pub mod section_header;
pub mod value_popup;

pub use header_bar::{AbState, Oversampling, PluginHeaderBar};
pub use preset_browser::{Preset, PresetBrowser};
pub use rack_frame::{RackFrame, RackStyle};
pub use section_header::SectionHeader;
pub use value_popup::{
    POPUP_GAP, ValuePopup, draw as draw_value_popup, popup_rect, show_drag_value,
    show_drag_value_labeled,
};
