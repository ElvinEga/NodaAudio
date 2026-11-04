//! Floating value readouts shown while a control is dragged.
//!
//! A knob or slider is small; the number it is changing should not be. The
//! popup is painted on the tooltip layer, above every panel, so it survives
//! clipping and follows the control it belongs to.
//!
//! ```ignore
//! let response = ui.add(Knob::new(&mut gain).range(-24.0, 24.0));
//! show_drag_value(ui, &response, &format!("{gain:+.1} dB"));
//! ```

use std::borrow::Cow;

use egui::{
    Color32, Context, CornerRadius, FontId, Id, LayerId, Order, Pos2, Rect, Response, Stroke,
    StrokeKind, Ui, Vec2,
};

use crate::oklab::with_alpha;
use crate::ui::theme::Theme;

/// Distance from the anchor to the popup's bottom edge.
pub const POPUP_GAP: f32 = 4.0;

/// The rect a popup of `size` takes when its bottom-centre is anchored at
/// `anchor`.
#[must_use]
pub fn popup_rect(anchor: Pos2, size: Vec2) -> Rect {
    Rect::from_min_size(
        egui::pos2(anchor.x - size.x / 2.0, anchor.y - size.y - POPUP_GAP),
        size,
    )
}

/// A value bubble with an optional caption and accent stripe.
#[must_use = "a ValuePopup paints nothing until it is painted"]
pub struct ValuePopup<'a> {
    anchor: Pos2,
    value: Cow<'a, str>,
    label: Option<Cow<'a, str>>,
    accent: Option<Color32>,
}

impl<'a> ValuePopup<'a> {
    /// A popup whose bottom-centre sits at `anchor`.
    pub fn new(anchor: Pos2, value: impl Into<Cow<'a, str>>) -> Self {
        Self {
            anchor,
            value: value.into(),
            label: None,
            accent: None,
        }
    }

    /// A small caption above the value, e.g. the parameter name.
    #[must_use]
    pub fn label(mut self, label: impl Into<Cow<'a, str>>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A stripe on the left edge, usually the plugin's accent.
    #[must_use]
    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Paint the bubble on the tooltip layer and return its rect.
    pub fn paint(self, ctx: &Context) -> Rect {
        draw(ctx, self.anchor, &self.value, self.label.as_deref(), self.accent)
    }
}

/// Paint a value bubble whose bottom-centre sits at `anchor`.
///
/// `label` is optional; `accent` defaults to the theme's primary color.
pub fn draw(
    ctx: &Context,
    anchor: Pos2,
    value: &str,
    label: Option<&str>,
    accent: Option<Color32>,
) -> Rect {
    let theme = Theme::from_ctx(ctx);
    let palette = &theme.palette;
    let painter =
        ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("noda-audio::value-popup")));

    let value_galley = painter.layout_no_wrap(
        value.to_owned(),
        FontId::proportional(11.5),
        palette.foreground,
    );
    let label_galley = label.map(|label| {
        painter.layout_no_wrap(label.to_owned(), FontId::proportional(8.5), palette.text_dim)
    });

    let text_width = label_galley
        .as_ref()
        .map_or(value_galley.size().x, |galley| galley.size().x.max(value_galley.size().x));
    let text_height = value_galley.size().y
        + label_galley.as_ref().map_or(0.0, |galley| galley.size().y + 1.0);
    let size = Vec2::new(text_width.max(26.0) + 16.0, text_height + 10.0);
    let rect = popup_rect(anchor, size);

    // A faint shadow separates the bubble from a light panel behind it.
    painter.rect_filled(
        rect.expand(1.0),
        CornerRadius::same(theme.radius.sm() as u8),
        with_alpha(Color32::BLACK, if palette.is_dark() { 0.35 } else { 0.12 }),
    );
    painter.rect_filled(
        rect,
        CornerRadius::same(theme.radius.sm() as u8),
        palette.popover,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(theme.radius.sm() as u8),
        Stroke::new(1.0_f32, palette.border),
        StrokeKind::Inside,
    );

    let accent = accent.unwrap_or(palette.primary);
    painter.line_segment(
        [
            Pos2::new(rect.left() + 1.5, rect.top() + 5.0),
            Pos2::new(rect.left() + 1.5, rect.bottom() - 5.0),
        ],
        Stroke::new(2.0_f32, accent),
    );

    let mut y = rect.top() + 5.0;
    if let Some(galley) = label_galley {
        painter.galley(
            Pos2::new(rect.center().x - galley.size().x / 2.0, y),
            galley.clone(),
            palette.text_dim,
        );
        y += galley.size().y + 1.0;
    }
    painter.galley(
        Pos2::new(rect.center().x - value_galley.size().x / 2.0, y),
        value_galley,
        palette.foreground,
    );

    rect
}

/// Show the value bubble above `response` while it is being dragged.
///
/// Returns true when a bubble was painted. Call it right after adding a
/// control; a control that is not being dragged costs one branch.
pub fn show_drag_value(ui: &Ui, response: &Response, value: &str) -> bool {
    if !response.dragged() {
        return false;
    }
    let anchor = Pos2::new(response.rect.center().x, response.rect.top());
    draw(ui.ctx(), anchor, value, None, None);
    true
}

/// [`show_drag_value`] with a caption and a custom accent.
pub fn show_drag_value_labeled(
    ui: &Ui,
    response: &Response,
    label: &str,
    value: &str,
    accent: Color32,
) -> bool {
    if !response.dragged() {
        return false;
    }
    let anchor = Pos2::new(response.rect.center().x, response.rect.top());
    draw(ui.ctx(), anchor, value, Some(label), Some(accent));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_popup_sits_above_its_anchor_and_centres_on_it() {
        let anchor = Pos2::new(100.0, 80.0);
        let rect = popup_rect(anchor, Vec2::new(60.0, 24.0));
        assert_eq!(rect.center().x, anchor.x);
        assert_eq!(rect.bottom(), anchor.y - POPUP_GAP);
        assert_eq!(rect.width(), 60.0);
    }

    #[test]
    fn a_value_popup_paints_without_a_ui() {
        let ctx = Context::default();
        let mut wide = Rect::NOTHING;
        let mut narrow = Rect::NOTHING;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            wide = draw(ui.ctx(), Pos2::new(40.0, 40.0), "-6.0 dB", Some("Gain"), None);
            narrow = draw(ui.ctx(), Pos2::new(40.0, 40.0), "1", None, None);
        });
        assert!(wide.width() > 26.0);
        assert!(wide.height() > 10.0);
        assert!(narrow.width() >= 42.0);
    }
}
