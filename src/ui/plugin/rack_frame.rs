//! Modular rack chassis and frame for pro audio plugins and hardware emulations.
//!
//! Provides a 19-inch rack unit appearance with authentic corner mounting screws,
//! brushed metal finish, top header badge, power indicator LED, and inset shadow styling.

use egui::{
    Color32, CornerRadius, Pos2, Response, Sense, Stroke, Ui, Vec2,
};

use crate::ui::theme::Theme;

/// Style of the rack chassis panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RackStyle {
    /// Dark anodized brushed aluminium / modern rack (1U / 2U).
    DarkAluminium,
    /// Vintage parchment / warm analog faceplate.
    VintageBeige,
    /// Classic studio slate grey.
    SlateGrey,
}

/// A container widget that surrounds plugin controls in a pro audio rack chassis.
pub struct RackFrame<'a> {
    title: &'a str,
    subtitle: Option<&'a str>,
    style: RackStyle,
    power_on: bool,
    show_screws: bool,
    height: Option<f32>,
    width: Option<f32>,
}

impl<'a> RackFrame<'a> {
    /// Create a new rack frame with a title (e.g. "NODA COMPRESSOR").
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
            style: RackStyle::DarkAluminium,
            power_on: true,
            show_screws: true,
            height: None,
            width: None,
        }
    }

    /// Set an optional subtitle (e.g., "OPTICAL MASTERING LIMITER").
    #[must_use]
    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    /// Choose rack appearance style.
    #[must_use]
    pub fn style(mut self, style: RackStyle) -> Self {
        self.style = style;
        self
    }

    /// Set whether the power/status LED is illuminated.
    #[must_use]
    pub fn power_on(mut self, power_on: bool) -> Self {
        self.power_on = power_on;
        self
    }

    /// Set whether corner mounting screws are drawn.
    #[must_use]
    pub fn show_screws(mut self, show: bool) -> Self {
        self.show_screws = show;
        self
    }

    /// Set fixed explicit width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set fixed explicit height.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Render the rack frame and provide an inner `Ui` closure for the module content.
    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> Response {
        let theme = Theme::of(ui);
        // A dark chassis under light-theme controls reads as a rendering
        // fault rather than a design choice, so the default finish follows
        // the theme; an explicit grey or beige finish is kept either way.
        let style = if !theme.palette.is_dark() && self.style == RackStyle::DarkAluminium {
            RackStyle::VintageBeige
        } else {
            self.style
        };

        // Frame colors based on style
        let (chassis_bg, border_color, text_color, screw_head_color) = match style {
            RackStyle::DarkAluminium => (
                Color32::from_rgb(20, 22, 26),
                Color32::from_rgb(45, 50, 60),
                Color32::from_rgb(220, 225, 235),
                Color32::from_rgb(75, 80, 90),
            ),
            RackStyle::VintageBeige => (
                Color32::from_rgb(238, 233, 222),
                Color32::from_rgb(190, 182, 168),
                Color32::from_rgb(40, 36, 32),
                Color32::from_rgb(140, 135, 125),
            ),
            RackStyle::SlateGrey => (
                Color32::from_rgb(34, 38, 44),
                Color32::from_rgb(60, 66, 76),
                Color32::from_rgb(215, 220, 230),
                Color32::from_rgb(90, 95, 105),
            ),
        };

        let frame = egui::Frame::NONE
            .fill(chassis_bg)
            .stroke(Stroke::new(1.5_f32, border_color))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(16, 12));

        let response = frame.show(ui, |ui| {
            ui.vertical(|ui| {
                if let Some(w) = self.width {
                    ui.set_min_width(w);
                    ui.set_max_width(w);
                }
                if let Some(h) = self.height {
                    ui.set_min_height(h);
                }

                // Top Header: Title, Subtitle, and Power Indicator LED
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // Power / Status LED
                    let (led_rect, _) =
                        ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                    let led_color = if self.power_on {
                        Color32::from_rgb(80, 240, 120) // Active glowing green
                    } else {
                        Color32::from_rgb(60, 65, 70) // Dark off LED
                    };
                    let painter = ui.painter();
                    if self.power_on {
                        // Halo glow
                        painter.circle_filled(
                            led_rect.center(),
                            7.5_f32,
                            Color32::from_rgba_unmultiplied(80, 240, 120, 45),
                        );
                    }
                    painter.circle_filled(led_rect.center(), 4.0_f32, led_color);
                    painter.circle_stroke(
                        led_rect.center(),
                        4.5_f32,
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 100)),
                    );

                    // Title badge
                    ui.label(
                        egui::RichText::new(self.title)
                            .font(egui::FontId::proportional(12.5))
                            .strong()
                            .color(text_color),
                    );

                    if let Some(sub) = self.subtitle {
                        ui.label(
                            egui::RichText::new(sub)
                                .font(egui::FontId::proportional(10.0))
                                .color(if style == RackStyle::VintageBeige {
                                    Color32::from_rgb(110, 100, 90)
                                } else {
                                    Color32::from_rgb(140, 150, 165)
                                }),
                        );
                    }
                });

                ui.add_space(8.0);

                // Inner module contents
                add_contents(ui)
            })
            .inner
        });

        // Overlay corner rack screws
        if self.show_screws {
            let painter = ui.painter_at(response.response.rect);
            let r = response.response.rect;
            let screw_positions = [
                r.min + Vec2::new(8.0, 8.0),
                Pos2::new(r.max.x - 8.0, r.min.y + 8.0),
                Pos2::new(r.min.x + 8.0, r.max.y - 8.0),
                r.max - Vec2::new(8.0, 8.0),
            ];

            for pos in screw_positions {
                draw_rack_screw(&painter, pos, screw_head_color);
            }
        }

        response.response
    }
}

/// Draw an authentic crosshead rack chassis screw.
fn draw_rack_screw(painter: &egui::Painter, center: Pos2, color: Color32) {
    let radius = 3.8_f32;
    // Outer screw rim
    painter.circle_filled(center, radius, color);
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 120)),
    );

    // Phillips cross slot
    let slot_stroke = Stroke::new(
        1.0_f32,
        Color32::from_rgba_unmultiplied(20, 20, 20, 180),
    );
    let d = 2.0_f32;
    painter.line_segment([center + Vec2::new(-d, 0.0), center + Vec2::new(d, 0.0)], slot_stroke);
    painter.line_segment([center + Vec2::new(0.0, -d), center + Vec2::new(0.0, d)], slot_stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rack_frame_instantiation() {
        let frame = RackFrame::new("Pro Compressor")
            .subtitle("Mastering Grade")
            .style(RackStyle::DarkAluminium)
            .power_on(true)
            .show_screws(true);
        assert_eq!(frame.title, "Pro Compressor");
        assert_eq!(frame.subtitle, Some("Mastering Grade"));
        assert!(frame.power_on);
        assert!(frame.show_screws);
    }

    #[test]
    fn rack_contents_stack_vertically_inside_a_horizontal_parent() {
        let ctx = egui::Context::default();
        let mut height = 0.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| {
                let response = RackFrame::new("MODULE").width(200.0).show(ui, |ui| {
                    for _ in 0..3 {
                        ui.allocate_exact_size(Vec2::new(120.0, 40.0), Sense::hover());
                    }
                });
                height = response.rect.height();
            });
        });
        assert!(
            height > 140.0,
            "three 40-point modules must stack below the title, got {height}"
        );
    }
}
