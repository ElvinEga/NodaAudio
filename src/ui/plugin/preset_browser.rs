//! Preset browser for audio plugins and synth patches.
//!
//! Category badge, preset name with a dropdown list, `<` `>` cycling arrows,
//! a position counter, and an optional Save action.

use egui::{CornerRadius, Response, RichText, Stroke, Ui, Vec2, Widget};

use crate::ui::theme::Theme;

/// A preset descriptor containing category, name, and optional author/description.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// Category or sound bank (e.g. "Mastering", "Vocal", "Drums").
    pub category: String,
    /// Name of the preset (e.g. "Warm Tube Glue", "Punchy Snare 1176").
    pub name: String,
    /// Optional author or sound designer.
    pub author: Option<String>,
}

impl Preset {
    /// Create a new preset with category and name.
    pub fn new(category: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            name: name.into(),
            author: None,
        }
    }

    /// Add an author name.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }
}

/// An interactive preset navigation bar for plugins.
///
/// ```ignore
/// let mut index = 0;
/// ui.add(PresetBrowser::new(&presets, &mut index).on_save(|| { /* store */ }));
/// ```
#[must_use = "a PresetBrowser draws nothing until it is added to a Ui"]
pub struct PresetBrowser<'a> {
    presets: &'a [Preset],
    current_index: &'a mut usize,
    on_save: Option<Box<dyn FnMut() + 'a>>,
    width: Option<f32>,
}

impl<'a> PresetBrowser<'a> {
    /// Create a new preset browser referencing a preset list and active index.
    pub fn new(presets: &'a [Preset], current_index: &'a mut usize) -> Self {
        Self {
            presets,
            current_index,
            on_save: None,
            width: None,
        }
    }

    /// Callback triggered when the Save button is clicked.
    #[must_use]
    pub fn on_save(mut self, callback: impl FnMut() + 'a) -> Self {
        self.on_save = Some(Box::new(callback));
        self
    }

    /// Width of the open list. Default 200 points.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(120.0));
        self
    }
}

impl Widget for PresetBrowser<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let total = self.presets.len();

        let frame = egui::Frame::NONE
            .fill(palette.surface)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 4));

        frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    let current = if total > 0 {
                        self.presets.get(*self.current_index)
                    } else {
                        None
                    };

                    // Category pill.
                    egui::Frame::NONE
                        .fill(palette.track)
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(egui::Margin::symmetric(5, 2))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(
                                    current.map_or("Default", |preset| preset.category.as_str()),
                                )
                                .font(egui::FontId::proportional(10.0))
                                .color(palette.text_dim),
                            );
                        });

                    // Previous preset.
                    let prev = ui
                        .add_enabled(
                            total > 1,
                            egui::Button::new(
                                RichText::new("◀").font(egui::FontId::proportional(9.0)),
                            )
                            .min_size(Vec2::new(18.0, 18.0)),
                        )
                        .clicked();
                    if prev && total > 0 {
                        *self.current_index =
                            self.current_index.checked_sub(1).unwrap_or(total - 1);
                    }

                    // Preset name, with a dropdown of the whole bank.
                    let name = current.map_or("Init Preset", |preset| preset.name.as_str());
                    if total == 0 {
                        ui.label(
                            RichText::new(name)
                                .font(egui::FontId::proportional(11.5))
                                .strong()
                                .color(palette.foreground),
                        );
                    } else {
                        let mut selected = *self.current_index;
                        egui::ComboBox::from_id_salt(ui.id().with("preset_list"))
                            .selected_text(
                                RichText::new(name)
                                    .font(egui::FontId::proportional(11.5))
                                    .strong()
                                    .color(palette.foreground),
                            )
                            .width(self.width.unwrap_or(200.0))
                            .show_ui(ui, |ui| {
                                for (index, preset) in self.presets.iter().enumerate() {
                                    let item = ui.selectable_value(
                                        &mut selected,
                                        index,
                                        format!("{}  ·  {}", preset.name, preset.category),
                                    );
                                    if let Some(author) = &preset.author {
                                        item.on_hover_text(author);
                                    }
                                }
                            });
                        *self.current_index = selected.min(total - 1);
                    }

                    // Next preset.
                    let next = ui
                        .add_enabled(
                            total > 1,
                            egui::Button::new(
                                RichText::new("▶").font(egui::FontId::proportional(9.0)),
                            )
                            .min_size(Vec2::new(18.0, 18.0)),
                        )
                        .clicked();
                    if next && total > 0 {
                        *self.current_index = (*self.current_index + 1) % total;
                    }

                    // Position counter.
                    if total > 0 {
                        ui.label(
                            RichText::new(format!("{}/{}", *self.current_index + 1, total))
                                .font(egui::FontId::monospace(9.5))
                                .color(palette.text_dim),
                        );
                    }

                    // Save action.
                    if let Some(mut save) = self.on_save.take() {
                        ui.add_space(4.0);
                        let save_button = ui.add(
                            egui::Button::new(
                                RichText::new("Save")
                                    .font(egui::FontId::proportional(10.0))
                                    .color(palette.primary_foreground),
                            )
                            .fill(palette.primary),
                        );
                        if save_button.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            save();
                        }
                    }
                });

                // Author of the current preset, when there is one.
                let author = if total > 0 {
                    self.presets
                        .get(*self.current_index)
                        .and_then(|preset| preset.author.clone())
                } else {
                    None
                };
                if let Some(author) = author {
                    ui.label(
                        RichText::new(author)
                            .font(egui::FontId::proportional(8.5))
                            .color(palette.text_dim),
                    );
                }
            })
            .response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_browser_creation() {
        let presets = vec![
            Preset::new("Dynamics", "Warm Glue").author("Noda"),
            Preset::new("Mastering", "Gentle Ceiling"),
        ];
        let mut idx = 0;
        let browser = PresetBrowser::new(&presets, &mut idx);
        assert_eq!(browser.presets.len(), 2);
    }

    #[test]
    fn preset_browser_renders_headlessly() {
        let presets = vec![
            Preset::new("Dynamics", "Warm Glue").author("Noda"),
            Preset::new("Mastering", "Gentle Ceiling"),
        ];
        let mut idx = 1;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(PresetBrowser::new(&presets, &mut idx));
        });
        assert!(!output.shapes.is_empty(), "preset browser should emit shapes");
    }
}
