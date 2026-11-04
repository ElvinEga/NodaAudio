//! Audio device selector dropdown for audio inputs and outputs.
//!
//! Inspired by audiocn's `audio-device-select.tsx`. Pure data-driven UI:
//! accepts a list of available devices and binds to a selected device ID.

use egui::{
    ComboBox, FontId, Response, RichText, Ui, Widget,
};

use crate::ui::theme::{Size, Theme};

/// Description of an audio device (microphone, interface, speaker, headphone).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioDevice {
    /// Unique device ID.
    pub id: String,
    /// Display name (e.g. "MacBook Pro Microphone", "Scarlett 2i2 USB").
    pub label: String,
    /// Whether this is the system default device.
    pub is_default: bool,
    /// Whether the device is currently connected and available.
    pub is_available: bool,
}

impl AudioDevice {
    /// Create a new available audio device entry.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            is_default: false,
            is_available: true,
        }
    }

    /// Mark as system default device.
    pub fn default_device(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Mark availability status.
    pub fn available(mut self, is_available: bool) -> Self {
        self.is_available = is_available;
        self
    }
}

/// An audio device picker widget.
///
/// ```ignore
/// let devices = vec![
///     AudioDevice::new("built-in", "MacBook Pro Speakers").default_device(true),
///     AudioDevice::new("scarlett", "Focusrite Scarlett 2i2"),
/// ];
/// let mut selected = Some("built-in".to_string());
/// ui.add(AudioDeviceSelect::new(&mut selected, &devices));
/// ```
#[must_use = "an AudioDeviceSelect draws nothing until it is added to a Ui"]
pub struct AudioDeviceSelect<'a> {
    selected_id: &'a mut Option<String>,
    devices: &'a [AudioDevice],
    label: Option<&'a str>,
    placeholder: &'a str,
    allow_none: bool,
    size: Option<Size>,
    enabled: bool,
    width: Option<f32>,
}

impl<'a> AudioDeviceSelect<'a> {
    /// Create a device selector binding to `selected_id`.
    pub fn new(selected_id: &'a mut Option<String>, devices: &'a [AudioDevice]) -> Self {
        Self {
            selected_id,
            devices,
            label: None,
            placeholder: "Select audio device...",
            allow_none: false,
            size: None,
            enabled: true,
            width: None,
        }
    }

    /// Optional label preceding the dropdown.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Placeholder text when no device is selected.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Whether "None" is a selectable option.
    pub fn allow_none(mut self, allow: bool) -> Self {
        self.allow_none = allow;
        self
    }

    /// Size ladder.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Enable or disable interaction.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Explicit width in points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Add this selector to `ui`.
    pub fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);

        let current_label = match self.selected_id.as_deref() {
            Some(id) => self
                .devices
                .iter()
                .find(|d| d.id == id)
                .map_or_else(|| format!("{id} (disconnected)"), |d| d.label.clone()),
            None => self.placeholder.to_string(),
        };

        let width = self.width.unwrap_or(match size {
            Size::Small => 140.0,
            Size::Medium => 200.0,
            Size::Large => 260.0,
        });

        let mut changed = false;

        let combo_id = ui.make_persistent_id(self.label.unwrap_or("audio_device_select"));
        let response = ui.horizontal(|ui| {
            if let Some(lbl) = self.label {
                ui.label(
                    RichText::new(lbl)
                        .font(FontId::proportional(theme.size.label_font()))
                        .color(theme.palette.text_dim),
                );
            }

            let mut combo = ComboBox::from_id_salt(combo_id)
                .width(width)
                .selected_text(RichText::new(&current_label).font(FontId::proportional(theme.size.value_font())));

            if !self.enabled {
                combo = combo.wrap_mode(egui::TextWrapMode::Truncate);
            }

            combo.show_ui(ui, |ui| {
                if self.allow_none {
                    let is_selected = self.selected_id.is_none();
                    if ui.selectable_label(is_selected, "None").clicked() {
                        *self.selected_id = None;
                        changed = true;
                    }
                }

                for device in self.devices {
                    let is_selected = self
                        .selected_id
                        .as_deref()
                        .map_or(false, |id| id == device.id);

                    let item_text = if device.is_default {
                        format!("{} (Default)", device.label)
                    } else {
                        device.label.clone()
                    };

                    let color = if !device.is_available {
                        theme.palette.text_dim
                    } else {
                        theme.palette.foreground
                    };

                    let resp = ui.selectable_label(
                        is_selected,
                        RichText::new(item_text).color(color),
                    );

                    if resp.clicked() {
                        *self.selected_id = Some(device.id.clone());
                        changed = true;
                    }
                }
            });
        }).response;

        if changed {
            ui.ctx().request_repaint();
        }

        response
    }
}

impl<'a> Widget for AudioDeviceSelect<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_device_select_renders() {
        let devices = vec![
            AudioDevice::new("dev1", "Microphone 1").default_device(true),
            AudioDevice::new("dev2", "USB Interface"),
        ];
        let mut selected = Some("dev1".to_string());

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(AudioDeviceSelect::new(&mut selected, &devices).label("Input"));
        });
        assert_eq!(selected, Some("dev1".to_string()));
    }
}
