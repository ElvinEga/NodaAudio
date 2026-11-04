//! A segmented selector for saturation and distortion characters.

use egui::{Align2, Color32, FontId, Key, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

use crate::oklab::with_alpha;
use crate::ui::saturation::SaturationType;
use crate::ui::theme::Theme;

/// The padding either side of a segment's label.
const SEGMENT_PADDING: f32 = 14.0;

/// A segmented selector for the six [`SaturationType`] characters: Tape,
/// Tube, Tanh, Clip, Fold and Fuzz.
///
/// Click a segment to select it; with the selector focused, the left and
/// right arrow keys move the selection.
///
/// ```ignore
/// ui.add(SaturationTypeSelect::new(&mut sat_type));
/// ```
#[must_use = "a SaturationTypeSelect draws nothing until it is added to a Ui"]
pub struct SaturationTypeSelect<'a> {
    selected: &'a mut SaturationType,
    accent: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    full_names: bool,
}

impl<'a> SaturationTypeSelect<'a> {
    /// Bind the selector to an active saturation type.
    pub fn new(selected: &'a mut SaturationType) -> Self {
        Self {
            selected,
            accent: None,
            width: None,
            height: None,
            full_names: false,
        }
    }

    /// Color accent for the active segment. Default the theme's primary.
    #[must_use]
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    /// A fixed width; otherwise each segment takes the width of its label.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(80.0));
        self
    }

    /// A fixed height. Default 24 points.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(16.0));
        self
    }

    /// Print `Hard Clip` instead of `Clip`. Default false.
    #[must_use]
    pub fn full_names(mut self, full: bool) -> Self {
        self.full_names = full;
        self
    }

    /// The segments' labels, in [`SaturationType::ALL`] order.
    #[must_use]
    pub fn labels(&self) -> Vec<&'static str> {
        SaturationType::ALL
            .iter()
            .map(|kind| {
                if self.full_names {
                    kind.name()
                } else {
                    kind.short_name()
                }
            })
            .collect()
    }
}

impl Widget for SaturationTypeSelect<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let accent = self.accent.unwrap_or(theme.palette.primary);
        let height = self.height.unwrap_or(24.0);
        let font = FontId::proportional((theme.size.label_font() - 1.0).max(8.0));
        let labels = self.labels();

        let widths: Vec<f32> = if self.width.is_some() {
            vec![0.0; labels.len()]
        } else {
            labels
                .iter()
                .map(|label| {
                    let galley = ui.painter().layout_no_wrap(
                        (*label).to_owned(),
                        font.clone(),
                        Color32::PLACEHOLDER,
                    );
                    (galley.size().x + SEGMENT_PADDING).max(44.0)
                })
                .collect()
        };
        let natural: f32 = if widths.iter().all(|w| *w == 0.0) {
            self.width.unwrap_or(80.0)
        } else {
            widths.iter().sum()
        };
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(natural, height), Sense::click());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        ui.painter()
            .rect_filled(rect, theme.radius.sm(), theme.palette.track);
        ui.painter().rect_stroke(
            rect,
            theme.radius.sm(),
            Stroke::new(1.0_f32, theme.palette.border),
            egui::StrokeKind::Inside,
        );

        let segment_width = rect.width() / labels.len() as f32;
        let mut clicked = None;

        for (index, (kind, label)) in SaturationType::ALL.iter().zip(labels.iter()).enumerate() {
            let left = if self.width.is_some() {
                rect.min.x + index as f32 * segment_width
            } else {
                rect.min.x + widths[..index].iter().sum::<f32>()
            };
            let width = if self.width.is_some() { segment_width } else { widths[index] };
            let item = Rect::from_min_size(
                egui::pos2(left, rect.min.y),
                Vec2::new(width, rect.height()),
            );
            let hit = ui.interact(item, ui.id().with(kind), Sense::click());
            let selected = *self.selected == *kind;

            if hit.clicked() {
                clicked = Some(*kind);
            }
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if selected {
                let body = item.shrink(1.5);
                ui.painter()
                    .rect_filled(body, theme.radius.sm(), with_alpha(accent, 0.16));
                ui.painter().rect_stroke(
                    body,
                    theme.radius.sm(),
                    Stroke::new(1.0_f32, accent),
                    egui::StrokeKind::Inside,
                );
            } else if hit.hovered() {
                ui.painter().rect_filled(
                    item.shrink(2.0),
                    theme.radius.sm(),
                    with_alpha(theme.palette.foreground, 0.06),
                );
            }

            let color = if selected {
                accent
            } else if hit.hovered() {
                theme.palette.foreground
            } else {
                theme.palette.text_dim
            };
            ui.painter()
                .text(item.center(), Align2::CENTER_CENTER, *label, font.clone(), color);
            if !self.full_names {
                hit.on_hover_text(kind.description());
            }
        }

        // Left/right move the selection while the group has focus.
        if let Some(kind) = clicked {
            *self.selected = kind;
            response.request_focus();
            response.mark_changed();
        }
        if response.has_focus() {
            let step = ui.input(|i| {
                if i.key_pressed(Key::ArrowLeft) {
                    Some(-1isize)
                } else if i.key_pressed(Key::ArrowRight) {
                    Some(1isize)
                } else {
                    None
                }
            });
            if let Some(step) = step {
                let current = SaturationType::ALL
                    .iter()
                    .position(|kind| *kind == *self.selected)
                    .unwrap_or(0) as isize;
                let next = (current + step).rem_euclid(SaturationType::ALL.len() as isize) as usize;
                *self.selected = SaturationType::ALL[next];
                response.mark_changed();
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturation_selector_renders_headlessly() {
        let mut kind = SaturationType::Tube;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(SaturationTypeSelect::new(&mut kind));
        });
        assert!(!output.shapes.is_empty(), "saturation selector should emit shapes");
    }

    #[test]
    fn labels_can_be_short_or_full() {
        let mut kind = SaturationType::Tube;
        let selector = SaturationTypeSelect::new(&mut kind);
        assert_eq!(selector.labels().len(), SaturationType::ALL.len());
        assert_eq!(selector.labels()[2], "Tanh");
        assert_eq!(selector.full_names(true).labels()[3], "Hard Clip");
    }
}
