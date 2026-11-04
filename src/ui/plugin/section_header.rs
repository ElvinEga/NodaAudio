//! Section header and parameter grouping dividers for audio plugins.
//!
//! Provides clean visual separation for complex plugins (e.g., EQ, dynamics,
//! filter, modulation, or output sections).

use egui::{Color32, CornerRadius, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};

use crate::oklab::with_alpha;
use crate::ui::theme::Theme;

/// Black or white, whichever stays readable on `background`.
fn readable_on(background: Color32) -> Color32 {
    let luma = 0.299 * f32::from(background.r())
        + 0.587 * f32::from(background.g())
        + 0.114 * f32::from(background.b());
    if luma > 140.0 {
        Color32::from_black_alpha(190)
    } else {
        Color32::WHITE
    }
}

/// A section header with title, optional enable switch, and horizontal accent divider.
///
/// ```ignore
/// ui.add(SectionHeader::new("EQ").enabled(&mut eq_on));
/// ```
pub struct SectionHeader<'a> {
    title: &'a str,
    enabled: Option<&'a mut bool>,
    accent_color: Option<Color32>,
}

impl<'a> SectionHeader<'a> {
    /// Create a new section header with title.
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            enabled: None,
            accent_color: None,
        }
    }

    /// Add an active/enable toggle for this section.
    #[must_use]
    pub fn enabled(mut self, enabled: &'a mut bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Set a custom accent color for the title or divider.
    #[must_use]
    pub fn accent(mut self, color: Color32) -> Self {
        self.accent_color = Some(color);
        self
    }
}

impl Widget for SectionHeader<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let palette = &theme.palette;
        let accent = self.accent_color.unwrap_or(palette.primary);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Optional section on/off toggle.
            if let Some(on) = self.enabled {
                let check_rect = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::click()).0;
                let is_checked = *on;
                let painter = ui.painter_at(check_rect);

                let bg = if is_checked { accent } else { palette.track };
                painter.rect_filled(check_rect, CornerRadius::same(3), bg);
                painter.rect_stroke(
                    check_rect,
                    CornerRadius::same(3),
                    Stroke::new(1.0_f32, if is_checked { accent } else { palette.border }),
                    egui::StrokeKind::Inside,
                );
                if is_checked {
                    let tick = readable_on(accent);
                    let p1 = check_rect.min + Vec2::new(3.5, 7.0);
                    let p2 = check_rect.min + Vec2::new(6.0, 10.5);
                    let p3 = check_rect.min + Vec2::new(10.5, 4.0);
                    painter.line_segment([p1, p2], Stroke::new(1.8_f32, tick));
                    painter.line_segment([p2, p3], Stroke::new(1.8_f32, tick));
                }

                let hit = ui.interact(check_rect, ui.id().with("chk"), Sense::click());
                if hit.clicked() {
                    *on = !*on;
                }
                if hit.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    painter.rect_stroke(
                        check_rect,
                        CornerRadius::same(3),
                        Stroke::new(1.0_f32, with_alpha(palette.foreground, 0.35)),
                        egui::StrokeKind::Outside,
                    );
                }
            }

            // Section title.
            ui.label(
                RichText::new(self.title.to_uppercase())
                    .font(egui::FontId::proportional(11.0))
                    .strong()
                    .color(palette.foreground),
            );

            // Divider, tinted by the accent so a group reads at a glance.
            let available_width = ui.available_width();
            if available_width > 12.0 {
                let (line_rect, _) =
                    ui.allocate_exact_size(Vec2::new(available_width, 1.0), Sense::hover());
                ui.painter().line_segment(
                    [line_rect.left_center(), line_rect.right_center()],
                    Stroke::new(1.0_f32, with_alpha(accent, 0.30)),
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
    fn test_section_header_creation() {
        let mut enabled = true;
        let header = SectionHeader::new("Filter").enabled(&mut enabled);
        assert_eq!(header.title, "Filter");
        assert_eq!(header.enabled, Some(&mut true));
    }

    #[test]
    fn section_header_renders_headlessly() {
        let mut enabled = false;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(SectionHeader::new("Dynamics").enabled(&mut enabled));
        });
        assert!(!output.shapes.is_empty(), "section header should emit shapes");
    }
}
