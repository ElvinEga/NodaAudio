//! The LCD display: a bezelled screen whose glass, ink and dot grid come from
//! one colour, and everything sitting on it reads those pixels back.
//!
//! This is the port of audiocn's `lcd-display.tsx`. In the browser the screen
//! is a stack of CSS custom properties; here it is a [`LcdScreen`] a display
//! installs in the context, so a [`crate::ui::LcdMeter`] anywhere inside finds
//! the pixels its enclosing screen was built with.

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape, Stroke,
    Ui, Vec2, Widget,
};

use crate::oklab::{dim, oklch, pale, with_alpha};
use crate::ui::theme::Theme;

/// The dot grid in points: `--lcd-display-pitch`.
pub const PITCH: f32 = 2.0;

/// Per-character letter-spacing: `tracking-[0.12em]`.
pub const TRACKING: f32 = 0.12;

/// How far the bezel stands off the glass: `p-1.5`.
const BEZEL: f32 = 6.0;

/// The glass's own padding: `px-4 py-2`.
const GLASS_PAD_X: f32 = 16.0;
const GLASS_PAD_Y: f32 = 8.0;

/// The vertical padding of a row: `py-3`.
const ROW_PAD_Y: f32 = 12.0;

/// The status pixel: `size-1.5`, inset `right-2 bottom-2`.
const STATUS: f32 = 6.0;

/// `rgb(0 0 0 / a)`, the shadow and recess colours of both screens.
#[must_use]
pub fn black(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(0, 0, 0, alpha)
}

/// `rgb(255 255 255 / a)`, the sheen and the bezel's top highlight.
#[must_use]
pub fn white(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
}

/// The ink of a screen. `primary` follows the theme; the rest are the fixed
/// hardware colours from the `variant` column of `lcdDisplayVariants`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LcdVariant {
    /// The theme's `--primary`.
    #[default]
    Primary,
    /// `oklch(0.76 0.13 52)`.
    Amber,
    /// `oklch(0.85 0.19 142)`.
    Green,
    /// `oklch(0.84 0.12 205)`.
    Cyan,
    /// `oklch(0.68 0.2 27)`.
    Red,
}

impl LcdVariant {
    /// `--lcd-display-ink`.
    #[must_use]
    pub fn ink(&self, theme: &Theme) -> Color32 {
        match self {
            LcdVariant::Primary => theme.palette.primary,
            LcdVariant::Amber => oklch(0.76, 0.13, 52.0),
            LcdVariant::Green => oklch(0.85, 0.19, 142.0),
            LcdVariant::Cyan => oklch(0.84, 0.12, 205.0),
            LcdVariant::Red => oklch(0.68, 0.2, 27.0),
        }
    }
}

/// Which screen the ink sits behind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LcdAppearance {
    /// A backlit pale screen in the light theme, a glowing dark one in the
    /// dark theme - what the `auto` variant writes as two rule sets.
    #[default]
    Auto,
    /// Pale glass, dark pixels: a calculator or a pager.
    Light,
    /// Dark glass, glowing pixels: an oscilloscope.
    Dark,
}

impl LcdAppearance {
    /// `auto` resolves against the theme's own base.
    #[must_use]
    pub fn resolve(&self, theme: &Theme) -> Self {
        match self {
            LcdAppearance::Auto if theme.palette.is_dark() => LcdAppearance::Dark,
            LcdAppearance::Auto => LcdAppearance::Light,
            other => *other,
        }
    }
}

/// The text size of a screen, which its rows, rules and meter follow.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LcdSize {
    /// `text-sm`.
    Small,
    /// `text-lg`.
    #[default]
    Default,
    /// `text-2xl`.
    Large,
}

impl LcdSize {
    /// The display's point size.
    #[must_use]
    pub fn font_size(&self) -> f32 {
        match self {
            LcdSize::Small => 14.0,
            LcdSize::Default => 18.0,
            LcdSize::Large => 24.0,
        }
    }

    /// One meter row, in dots: 5, 7 and 10 times the pitch, so a track
    /// centres on a line of text with a whole number of dots above and below.
    #[must_use]
    pub fn meter_thickness(&self) -> f32 {
        PITCH * match self {
            LcdSize::Small => 5.0,
            LcdSize::Default => 7.0,
            LcdSize::Large => 10.0,
        }
    }
}

/// Every colour the screen is made of, resolved from an ink and an appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LcdScreen {
    /// The bezel's top edge.
    pub bezel: Color32,
    /// The bezel's bottom edge, where its gradient ends.
    pub bezel_edge: Color32,
    /// The hairline around the bezel.
    pub bezel_ring: Color32,
    /// The glass behind the pixels.
    pub glass: Color32,
    /// The lit pixels.
    pub lit: Color32,
    /// A label or rule drawn in the ink's shadow.
    pub ink_dim: Color32,
    /// An unlit pixel, still visible against the glass.
    pub ink_unlit: Color32,
    /// The halo around lit text.
    pub glow: Color32,
    /// The horizontal bands over the glass.
    pub scanline: Color32,
    /// The highlight down the top of the glass.
    pub sheen: Color32,
    /// The shadow recessed into the glass edge.
    pub recess: Color32,
}

impl LcdScreen {
    /// The screen an ink sits behind, straight off `LIGHT_SCREEN` and
    /// `DARK_SCREEN`.
    #[must_use]
    pub fn new(ink: Color32, appearance: LcdAppearance) -> Self {
        // `color-mix(in oklab, ink, black 62%)` keeps 38% of the ink; an
        // unlit screen glows the ink itself.
        let lit = match appearance {
            LcdAppearance::Light => dim(ink, 0.38),
            _ => ink,
        };
        match appearance {
            LcdAppearance::Light => Self {
                bezel: oklch(0.95, 0.003, 250.0),
                bezel_edge: oklch(0.85, 0.004, 250.0),
                bezel_ring: black(36),
                glass: pale(ink),
                glow: with_alpha(lit, 0.22),
                ink_dim: with_alpha(lit, 0.40),
                ink_unlit: with_alpha(lit, 0.07),
                lit,
                scanline: black(10),
                sheen: white(89),
                recess: black(72),
            },
            _ => Self {
                bezel: oklch(0.33, 0.004, 250.0),
                bezel_edge: oklch(0.24, 0.004, 250.0),
                bezel_ring: black(178),
                glass: oklch(0.15, 0.004, 200.0),
                glow: with_alpha(lit, 0.60),
                ink_dim: with_alpha(lit, 0.40),
                ink_unlit: with_alpha(lit, 0.07),
                lit,
                scanline: black(56),
                sheen: white(13),
                recess: black(230),
            },
        }
    }

    /// The display's mono face.
    #[must_use]
    pub fn font(&self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Monospace)
    }

    /// Height of one line of the display's text, whose root sets `leading-none`.
    #[must_use]
    pub fn line_height(&self, ui: &Ui, size: f32) -> f32 {
        ui.painter()
            .layout_no_wrap("0".to_owned(), self.font(size), Color32::WHITE)
            .size()
            .y
    }

    /// Width one character cell occupies: a mono glyph plus the 0.12em the
    /// display puts after it.
    #[must_use]
    pub fn ch(&self, ui: &Ui, size: f32) -> f32 {
        let glyph = ui
            .painter()
            .layout_no_wrap("0".to_owned(), self.font(size), Color32::WHITE)
            .size()
            .x;
        glyph * (1.0 + TRACKING)
    }

    /// The box a string of character cells occupies.
    #[must_use]
    pub fn measure(&self, ui: &Ui, text: &str, size: f32) -> Vec2 {
        Vec2::new(
            self.ch(ui, size) * text.chars().count() as f32,
            self.line_height(ui, size),
        )
    }

    /// Draw text one character cell at a time, returning the box it covered -
    /// what a dot mask or a glow is punched against.
    pub fn text(
        &self,
        ui: &Ui,
        at: Pos2,
        align: Align2,
        text: &str,
        size: f32,
        color: Color32,
    ) -> Rect {
        let ch = self.ch(ui, size);
        let box_size = self.measure(ui, text, size);
        let left = match align {
            Align2::LEFT_CENTER | Align2::LEFT_TOP | Align2::LEFT_BOTTOM => at.x,
            Align2::CENTER_CENTER | Align2::CENTER_TOP | Align2::CENTER_BOTTOM => {
                at.x - box_size.x * 0.5
            }
            _ => at.x - box_size.x,
        };
        let top = match align {
            Align2::LEFT_TOP | Align2::CENTER_TOP | Align2::RIGHT_TOP => at.y,
            Align2::LEFT_CENTER | Align2::CENTER_CENTER | Align2::RIGHT_CENTER => {
                at.y - box_size.y * 0.5
            }
            _ => at.y - box_size.y,
        };
        for (index, letter) in text.chars().enumerate() {
            let name = letter.to_string();
            ui.painter().text(
                Pos2::new(left + index as f32 * ch, top),
                Align2::LEFT_TOP,
                name,
                self.font(size),
                color,
            );
        }
        Rect::from_min_size(Pos2::new(left, top), box_size)
    }

    /// Lit text with the screen's halo: the string offset a pixel either side
    /// in the glow colour, standing in for the CSS drop shadow, then solid on
    /// top and cut back into dots.
    pub fn glow_text(
        &self,
        ui: &Ui,
        at: Pos2,
        align: Align2,
        text: &str,
        size: f32,
        matrix: bool,
    ) {
        for offset in [-1.0, 1.0] {
            let _ = self.text(ui, Pos2::new(at.x + offset, at.y), align, text, size, self.glow);
        }
        let box_rect = self.text(ui, at, align, text, size, self.lit);
        if matrix {
            punch_grid(ui, box_rect, self.glass);
        }
    }

    /// `--lcd-meter-unlit`: the ghost a meter's track is drawn in. The lit ink
    /// at 14%, twice the 7% of an unlit pixel, since a segment has to read as
    /// a segment and not as the glass.
    #[must_use]
    pub fn meter_unlit(&self) -> Color32 {
        with_alpha(self.lit, 0.14)
    }
}

/// Cut a rect back into dots: glass-coloured bars along every grid line, so
/// text under it reads as lit pixels rather than solid strokes.
pub fn punch_grid(ui: &Ui, rect: Rect, glass: Color32) {
    if !rect.is_positive() {
        return;
    }
    let line = PITCH * 0.5;
    let mut x = (rect.min.x / PITCH).ceil() * PITCH;
    while x < rect.max.x {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(line, rect.height())),
            0.0,
            glass,
        );
        x += PITCH;
    }
    let mut y = (rect.min.y / PITCH).ceil() * PITCH;
    while y < rect.max.y {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(rect.width(), line)),
            0.0,
            glass,
        );
        y += PITCH;
    }
}

/// The blinking pixel in the corner of a screen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LcdStatus {
    /// Nothing lit.
    Off,
    /// A slow blink: CSS's default `animate-pulse`.
    #[default]
    Idle,
    /// A fast blink: `animation-duration: 0.6s`.
    Busy,
}

impl LcdStatus {
    /// How long one blink lasts, in seconds.
    #[must_use]
    pub fn period(self) -> f64 {
        match self {
            LcdStatus::Off => 0.0,
            LcdStatus::Idle => 2.0,
            LcdStatus::Busy => 0.6,
        }
    }

    /// Whether the pixel is lit at `time`, the clock the blink is read from.
    #[must_use]
    pub fn is_lit_at(self, time: f64) -> bool {
        match self {
            LcdStatus::Off => false,
            state => (time % state.period()) < state.period() * 0.5,
        }
    }
}

/// One line of a screen: a dim label left, a bright value right.
#[derive(Clone, Copy, Debug)]
pub struct LcdDisplayRow<'a> {
    /// The name of what the value measures.
    pub label: &'a str,
    /// The value, already formatted.
    pub value: &'a str,
    /// A dotted rule in place of text.
    pub separator: bool,
}

impl<'a> LcdDisplayRow<'a> {
    /// A row of text.
    #[must_use]
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self { label, value, separator: false }
    }

    /// A dotted rule between rows, `LcdDisplaySeparator`.
    #[must_use]
    pub fn rule() -> Self {
        Self { label: "", value: "", separator: true }
    }
}

/// The settings a screen publishes to the widgets drawn on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LcdContext {
    /// The palette.
    pub screen: LcdScreen,
    /// The text size, which a meter takes its row height from.
    pub size: LcdSize,
    /// Whether the glass is broken into dots.
    pub matrix: bool,
}

/// An LCD screen in a bezel, drawing its rows.
///
/// [`LcdDisplay::paint_frame`] hands the glass to the caller instead, which is
/// how [`crate::ui::LcdMeter`] puts a meter on a display.
#[derive(Clone, Copy, Debug)]
#[must_use = "an LcdDisplay draws nothing until it is added to a Ui"]
pub struct LcdDisplay<'a> {
    rows: &'a [LcdDisplayRow<'a>],
    variant: LcdVariant,
    appearance: LcdAppearance,
    size: LcdSize,
    matrix: bool,
    scanlines: bool,
    width: Option<f32>,
    status: LcdStatus,
}

impl Default for LcdDisplay<'_> {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl<'a> LcdDisplay<'a> {
    /// A screen of rows.
    #[must_use]
    pub fn new(rows: &'a [LcdDisplayRow<'a>]) -> Self {
        Self {
            rows,
            variant: LcdVariant::default(),
            appearance: LcdAppearance::default(),
            size: LcdSize::default(),
            matrix: true,
            scanlines: true,
            width: None,
            status: LcdStatus::Idle,
        }
    }

    /// The ink. Default [`LcdVariant::Primary`].
    #[must_use]
    pub fn variant(mut self, variant: LcdVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Pale glass or dark. Default [`LcdAppearance::Auto`].
    #[must_use]
    pub fn appearance(mut self, appearance: LcdAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// The text size the rows inherit. Default [`LcdSize::Default`].
    #[must_use]
    pub fn size(mut self, size: LcdSize) -> Self {
        self.size = size;
        self
    }

    /// Break the glass and its text into dots. Default true.
    #[must_use]
    pub fn matrix(mut self, matrix: bool) -> Self {
        self.matrix = matrix;
        self
    }

    /// Draw the horizontal bands over the glass. Default true.
    #[must_use]
    pub fn scanlines(mut self, scanlines: bool) -> Self {
        self.scanlines = scanlines;
        self
    }

    /// A fixed width in points; otherwise the screen takes the widest row.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The corner pixel's state. Default a slow idle blink.
    #[must_use]
    pub fn status(mut self, status: LcdStatus) -> Self {
        self.status = status;
        self
    }

    /// The palette this display paints with.
    #[must_use]
    pub fn screen(&self, theme: &Theme) -> LcdScreen {
        LcdScreen::new(self.variant.ink(theme), self.appearance.resolve(theme))
    }

    /// The glass inside a bezel of this outer rect.
    #[must_use]
    pub fn glass_rect(rect: Rect) -> Rect {
        rect.shrink(BEZEL)
    }

    /// The area a display hands to whatever sits on its glass.
    #[must_use]
    pub fn content_rect(rect: Rect) -> Rect {
        Self::glass_rect(rect).shrink2(Vec2::new(GLASS_PAD_X, GLASS_PAD_Y))
    }

    /// The height one row of this screen occupies.
    #[must_use]
    pub fn row_height(&self, ui: &Ui, screen: &LcdScreen) -> f32 {
        screen.line_height(ui, self.size.font_size()) + ROW_PAD_Y * 2.0
    }

    /// The size a row set wants.
    #[must_use]
    pub fn desired(&self, ui: &Ui, screen: &LcdScreen) -> Vec2 {
        let text = self.size.font_size();
        let widest = self
            .rows
            .iter()
            .map(|line| {
                screen.measure(ui, line.label, text).x + screen.measure(ui, line.value, text).x
            })
            .fold(0.0f32, f32::max);
        Vec2::new(
            self.width.unwrap_or(widest + GLASS_PAD_X * 2.0 + 32.0),
            BEZEL * 2.0 + GLASS_PAD_Y * 2.0 + self.row_height(ui, screen) * self.rows.len() as f32,
        )
    }

    /// Draw the bezel and glass and publish the screen for children, returning
    /// the content rect to draw rows or a meter into.
    pub fn paint_frame(&self, ui: &mut Ui, rect: Rect, theme: &Theme) -> Rect {
        let screen = self.screen(theme);
        install(ui, &screen, self.size, self.matrix);
        let radius = theme.radius.lg().round() as u8;
        ui.painter().add(Shape::rect_filled(rect, radius, screen.bezel));
        let lower = Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y), rect.right_bottom());
        ui.painter().add(Shape::rect_filled(
            lower,
            CornerRadius { nw: 0, ne: 0, sw: radius, se: radius },
            screen.bezel_edge,
        ));
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, screen.bezel_ring),
            egui::StrokeKind::Inside,
        );

        let glass = Self::glass_rect(rect);
        let md = theme.radius.md();
        ui.painter().rect_filled(glass, md, screen.glass);
        if self.matrix {
            // The unlit dots fill 64% of each cell at 7% alpha. Averaged over
            // a whole screen that is a flat wash of the same product, and a
            // flat wash costs one quad instead of twenty thousand.
            ui.painter().rect_filled(glass, md, with_alpha(screen.lit, 0.045));
        }
        if self.scanlines {
            let mut y = glass.min.y + PITCH;
            while y < glass.max.y {
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(glass.min.x, y), Vec2::new(glass.width(), 1.0)),
                    0.0,
                    screen.scanline,
                );
                y += PITCH + 1.0;
            }
        }
        let sheen = glass.height() * 0.45;
        for step in 0..6 {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(glass.min.x, glass.min.y + step as f32 * sheen / 6.0),
                    Vec2::new(glass.width(), sheen / 6.0 + 0.5),
                ),
                0.0,
                with_alpha(screen.sheen, (1.0 - step as f32 / 6.0) / 6.0 * 2.2),
            );
        }
        ui.painter().rect_stroke(
            glass,
            md,
            Stroke::new(1.0_f32, screen.recess),
            egui::StrokeKind::Inside,
        );
        if self.status != LcdStatus::Off {
            let content = Self::content_rect(rect);
            let lit = self.status.is_lit_at(ui.input(|input| input.time));
            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(content.right() - STATUS, content.bottom() - STATUS),
                    Vec2::splat(STATUS),
                ),
                0.0,
                if lit { screen.lit } else { screen.ink_unlit },
            );
        }
        Self::content_rect(rect)
    }

    /// The display's own settings, which a child on its glass inherits the way
    /// a React meter reads them off `useLcdDisplay()`.
    #[must_use]
    pub fn inherited(&self) -> (LcdSize, bool) {
        (self.size, self.matrix)
    }

    /// Draw a screen of rows into a rect the caller owns.
    pub fn show(&self, ui: &mut Ui, rect: Rect, theme: &Theme) {
        let screen = self.screen(theme);
        let content = self.paint_frame(ui, rect, theme);
        self.show_rows(ui, content, &screen);
    }

    /// Lay the rows on a glass the caller has already drawn, so a meter can
    /// take the band underneath them.
    pub fn show_rows(&self, ui: &mut Ui, content: Rect, screen: &LcdScreen) {
        let text = self.size.font_size();
        let row = self.row_height(ui, screen);
        let matrix = self.matrix;
        ui.set_clip_rect(content);
        for (index, line) in self.rows.iter().enumerate() {
            let band = Rect::from_min_size(
                Pos2::new(content.min.x, content.min.y + index as f32 * row),
                Vec2::new(content.width(), row),
            );
            if line.separator {
                paint_separator(ui, band, screen);
                continue;
            }
            let at = band.center().y;
            let _ = screen.text(ui, Pos2::new(band.min.x, at), Align2::LEFT_CENTER, line.label, text, screen.ink_dim);
            screen.glow_text(ui, Pos2::new(band.max.x, at), Align2::RIGHT_CENTER, line.value, text, matrix);
        }
    }
}

/// The dotted rule between rows: `ink-dim` dots on a 2 x 1 pitch.
pub fn paint_separator(ui: &Ui, rect: Rect, screen: &LcdScreen) {
    let y = rect.center().y - PITCH * 0.5;
    let mut x = rect.min.x;
    while x < rect.max.x {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), Vec2::splat(PITCH)),
            0.0,
            screen.ink_dim,
        );
        x += PITCH * 2.0;
    }
}

impl Widget for LcdDisplay<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let screen = self.screen(&theme);
        let size = self.desired(ui, &screen);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.show(ui, rect, &theme);
        }
        response
    }
}

fn key() -> egui::Id {
    egui::Id::new("noda_lcd_context")
}

/// Publish a screen so widgets inside it read the same pixels back.
pub fn install(ui: &mut Ui, screen: &LcdScreen, size: LcdSize, matrix: bool) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(key(), LcdContext { screen: *screen, size, matrix });
    });
}

/// The enclosing display's settings, or `None` outside one.
#[must_use]
pub fn installed(ui: &Ui) -> Option<LcdContext> {
    ui.ctx().data(|data| data.get_temp::<LcdContext>(key()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_screens_are_the_two_kinds_of_hardware() {
        let ink = Color32::from_rgb(125, 236, 115);
        let dark = LcdScreen::new(ink, LcdAppearance::Dark);
        let light = LcdScreen::new(ink, LcdAppearance::Light);
        assert_eq!(dark.lit, ink, "an unlit screen glows the ink itself");
        assert!(light.lit.r() < ink.r() && light.lit.g() < ink.g(), "backlit pixels are dark: {:?}", light.lit);
        assert!(light.glass.r() > 200 && light.glass.g() > 200, "and the glass is pale: {:?}", light.glass);
        assert!(dark.glass.r() < 30 && dark.glass.g() < 30, "the dark glass is nearly black: {:?}", dark.glass);
    }

    #[test]
    fn auto_follows_the_theme_it_is_asked_from() {
        assert_eq!(LcdAppearance::Auto.resolve(&Theme::dark()), LcdAppearance::Dark);
        assert_eq!(LcdAppearance::Auto.resolve(&Theme::light()), LcdAppearance::Light);
    }

    #[test]
    fn the_meter_rows_are_a_whole_number_of_dots() {
        assert_eq!(LcdSize::Small.meter_thickness(), 10.0);
        assert_eq!(LcdSize::Default.meter_thickness(), 14.0);
        assert_eq!(LcdSize::Large.meter_thickness(), 20.0);
    }

    #[test]
    fn only_the_corner_pixel_blinks_and_only_when_told() {
        assert!(!LcdStatus::Off.is_lit_at(0.0));
        assert!(LcdStatus::Idle.is_lit_at(0.1));
        assert!(!LcdStatus::Idle.is_lit_at(1.1));
        assert!(LcdStatus::Busy.is_lit_at(0.1));
        assert!(!LcdStatus::Busy.is_lit_at(0.4));
    }

    #[test]
    fn tracking_widens_every_character_by_the_same_amount() {
        let _ = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            let screen = LcdScreen::new(Color32::from_rgb(125, 236, 115), LcdAppearance::Dark);
            let one = screen.ch(ui, 18.0);
            let eight = screen.measure(ui, "00000000", 18.0).x;
            assert!((eight - one * 8.0).abs() < 0.01, "{eight} against {one} per cell");
            let glyph = one / (1.0 + TRACKING);
            assert!(glyph > 8.0, "a monospace digit at 18pt is wider than 8pt: {glyph}");
        });
    }

    #[test]
    fn a_display_installs_its_screen_for_children() {
        let rows = [LcdDisplayRow::new("PEAK", "-12.3")];
        let _ = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            let theme = Theme::dark();
            let display = LcdDisplay::new(&rows).variant(LcdVariant::Green);
            assert!(installed(ui).is_none(), "nothing is installed before a screen is drawn");
            let screen = display.screen(&theme);
            display.paint_frame(ui, Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 80.0)), &theme);
            let context = installed(ui).expect("the display publishes its screen");
            assert_eq!(context.screen, screen);
            assert_eq!(context.screen.lit, Color32::from_rgb(125, 236, 115));
            assert_eq!(context.size, LcdSize::Default);
            assert!(context.matrix, "the dot grid is on until a caller turns it off");
        });
    }
}
