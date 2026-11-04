//! The control family: widgets a user takes a parameter out of.

pub mod audio_device_select;
pub mod channel_toggle;
pub mod fader;
pub mod knob;
pub mod lcd_display;
pub mod metallic_knob;
pub mod pan_control;
pub mod parameter_slider;
pub mod volume_control;

pub use audio_device_select::{AudioDevice, AudioDeviceSelect};
pub use channel_toggle::{ChannelToggle, ChannelToggleTone};
pub use fader::{DragMode, Fader, FaderStyle};
pub use knob::{Knob, Reading, format_pan, parse_pan};
pub use lcd_display::{
    LcdAppearance, LcdContext, LcdDisplay, LcdDisplayRow, LcdScreen, LcdSize, LcdStatus,
    LcdVariant, installed, paint_separator,
};
pub use metallic_knob::{MetallicKnob, MetallicKnobVariant};
pub use pan_control::PanControl;
pub use parameter_slider::ParameterSlider;
pub use volume_control::{VolumeControl, VolumeCurve, VolumeLevel};
