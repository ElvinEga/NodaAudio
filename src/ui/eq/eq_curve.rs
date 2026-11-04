//! Interactive parametric EQ frequency response display and curve editor.
//!
//! Modelled on FabFilter Pro-Q and Logic's Channel EQ:
//! - The composite response is evaluated analytically across 20 Hz .. 20 kHz
//!   and summed in decibels, with the individual bands faintly behind it.
//! - A logarithmic frequency grid, a decibel grid and an illuminated fill
//!   between the curve and unity.
//! - Draggable band handles: drag moves frequency and gain, the wheel trims
//!   Q, double-click returns the band to neutral. With the display focused,
//!   the arrow keys move the selected band and shift makes the step finer.
//! - Optional live spectrum behind the curve, for a plugin that has FFT data.

use egui::{
    Align2, Color32, FontId, Key, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget,
};

use crate::db::clamp;
use crate::oklab::with_alpha;
use crate::ui::eq::eq_band::{EqBand, NEUTRAL_GAIN_DB, NEUTRAL_Q};
use crate::ui::theme::Theme;

/// Logarithmic frequency boundary constants.
pub const MIN_FREQ_HZ: f32 = 20.0;
pub const MAX_FREQ_HZ: f32 = 20_000.0;

/// Default decibel amplitude display range (+/- 18.0 dB).
pub const DEFAULT_EQ_RANGE_DB: f32 = 18.0;

/// Default sample rate used for frequency response plotting.
pub const DEFAULT_SAMPLE_RATE: f32 = 48_000.0;

/// How many points the composite curve is evaluated at.
const EVAL_POINTS: usize = 256;

/// How far from a handle's centre a pointer still grabs it.
const GRAB_RADIUS: f32 = 12.0;

/// The drawn radius of a handle's dot.
const HANDLE_DOT: f32 = 7.5;

/// The labels along the bottom of the display.
const FREQ_TICKS: [(f32, &str); 10] = [
    (20.0, "20"),
    (50.0, "50"),
    (100.0, "100"),
    (200.0, "200"),
    (500.0, "500"),
    (1_000.0, "1k"),
    (2_000.0, "2k"),
    (5_000.0, "5k"),
    (10_000.0, "10k"),
    (20_000.0, "20k"),
];

/// An interactive parametric EQ curve display widget.
#[must_use = "an EqCurve draws nothing until it is added to a Ui"]
pub struct EqCurve<'a> {
    bands: &'a mut [EqBand],
    selected_band: &'a mut Option<usize>,
    range_db: f32,
    sample_rate: f32,
    spectrum: Option<&'a [f32]>,
    width: Option<f32>,
    height: Option<f32>,
    show_individual_curves: bool,
    show_handles: bool,
}

impl<'a> EqCurve<'a> {
    /// Create a new interactive EQ curve editor over a mutable slice of bands
    /// and the caller-owned active selection.
    pub fn new(bands: &'a mut [EqBand], selected_band: &'a mut Option<usize>) -> Self {
        Self {
            bands,
            selected_band,
            range_db: DEFAULT_EQ_RANGE_DB,
            sample_rate: DEFAULT_SAMPLE_RATE,
            spectrum: None,
            width: None,
            height: None,
            show_individual_curves: true,
            show_handles: true,
        }
    }

    /// Set vertical decibel display range (+/- range_db, default 18.0 dB).
    #[must_use]
    pub fn range_db(mut self, range_db: f32) -> Self {
        self.range_db = range_db.abs().max(6.0);
        self
    }

    /// Set audio sample rate for digital biquad response evaluation (default
    /// 48000.0).
    #[must_use]
    pub fn sample_rate(mut self, sample_rate: f32) -> Self {
        self.sample_rate = sample_rate.max(8_000.0);
        self
    }

    /// Optional live FFT background spectrum bands (0..1 normalized
    /// amplitudes, low to high frequency).
    #[must_use]
    pub fn spectrum(mut self, spectrum: &'a [f32]) -> Self {
        self.spectrum = Some(spectrum);
        self
    }

    /// Set explicit dimensions.
    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Toggle rendering of faint individual band curves (default true).
    #[must_use]
    pub fn individual_curves(mut self, show: bool) -> Self {
        self.show_individual_curves = show;
        self
    }

    /// Toggle the draggable handles and, with them, editing. A read-only
    /// display is `EqCurve::new(...).handles(false)`.
    #[must_use]
    pub fn handles(mut self, show: bool) -> Self {
        self.show_handles = show;
        self
    }

    /// The composite response at one frequency, including solo state.
    #[must_use]
    pub fn response_db(&self, frequency: f32) -> f32 {
        let soloing = self.bands.iter().any(|band| band.enabled && band.solo);
        self.bands
            .iter()
            .filter(|band| band.enabled && (!soloing || band.solo))
            .map(|band| band.magnitude_db(frequency, self.sample_rate))
            .sum()
    }

    /// The screen position of a band's handle.
    #[must_use]
    pub fn handle_position(&self, band: &EqBand, rect: Rect) -> Pos2 {
        Pos2::new(
            freq_to_screen_x(band.frequency, rect),
            db_to_screen_y(band.gain_db, rect, self.range_db),
        )
    }

    /// The band nearest a point, if one is inside its grab radius.
    #[must_use]
    pub fn band_at(&self, pos: Pos2, rect: Rect) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for band in self.bands.iter() {
            let distance = pos.distance(self.handle_position(band, rect));
            if distance <= GRAB_RADIUS && best.map_or(true, |(_, d)| distance < d) {
                best = Some((band.id, distance));
            }
        }
        best.map(|(id, _)| id)
    }
}

#[inline]
fn freq_to_norm_x(f: f32) -> f32 {
    let f = f.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ);
    (f / MIN_FREQ_HZ).log10() / (MAX_FREQ_HZ / MIN_FREQ_HZ).log10()
}

#[inline]
fn norm_x_to_freq(nx: f32) -> f32 {
    let nx = nx.clamp(0.0, 1.0);
    MIN_FREQ_HZ * (MAX_FREQ_HZ / MIN_FREQ_HZ).powf(nx)
}

#[inline]
fn freq_to_screen_x(f: f32, rect: Rect) -> f32 {
    rect.min.x + freq_to_norm_x(f) * rect.width()
}

#[inline]
fn screen_x_to_freq(x: f32, rect: Rect) -> f32 {
    norm_x_to_freq((x - rect.min.x) / rect.width())
}

#[inline]
fn db_to_screen_y(db: f32, rect: Rect, range_db: f32) -> f32 {
    let norm = (db / range_db).clamp(-1.0, 1.0);
    rect.center().y - norm * (rect.height() * 0.46)
}

#[inline]
fn screen_y_to_db(y: f32, rect: Rect, range_db: f32) -> f32 {
    let norm = (rect.center().y - y) / (rect.height() * 0.46);
    clamp(norm * range_db, -range_db * 1.5, range_db * 1.5)
}

/// Format a frequency the way an EQ band's readout does.
#[must_use]
pub fn format_frequency(hz: f32) -> String {
    if hz >= 10_000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// Format a gain as a signed decibel value.
#[must_use]
pub fn format_gain(db: f32) -> String {
    if db > 0.0 {
        format!("+{db:.1} dB")
    } else {
        format!("{db:.1} dB")
    }
}

/// A filled strip between a curve and a horizontal baseline. Triangles are
/// used instead of a polygon because a response may cross the baseline
/// several times and a convex hull would be wrong.
fn fill_under(painter: &egui::Painter, curve: &[Pos2], baseline_y: f32, color: Color32) {
    if curve.len() < 2 || color.a() == 0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for point in curve {
        mesh.colored_vertex(*point, color);
        mesh.colored_vertex(Pos2::new(point.x, baseline_y), color);
    }
    for i in 0..curve.len() - 1 {
        let top = i as u32 * 2;
        let bottom = top + 1;
        mesh.add_triangle(top, bottom, top + 2);
        mesh.add_triangle(bottom, bottom + 2, top + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl Widget for EqCurve<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::of(ui);
        let width = self.width.unwrap_or_else(|| ui.available_width().max(320.0));
        let height = self.height.unwrap_or(220.0);
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let range_db = self.range_db;
        let sample_rate = self.sample_rate;
        let pointer_pos = ui.ctx().pointer_interact_pos();
        let hovered_band = pointer_pos.and_then(|pos| self.band_at(pos, rect));

        // 1. Editing: drag a handle, wheel its Q, double-click to neutral.
        if self.show_handles {
            if response.drag_started() || response.clicked() {
                response.request_focus();
            }
            if response.drag_started() {
                if let Some(id) = hovered_band {
                    *self.selected_band = Some(id);
                }
            }
            if response.dragged() {
                // Fall back to the selected band so a drag that began on a
                // handle keeps editing even when the pointer outruns it.
                let id = hovered_band.or(*self.selected_band);
                if let Some(band) = id.and_then(|id| self.bands.iter_mut().find(|b| b.id == id)) {
                    if let Some(pos) = pointer_pos {
                        band.frequency = clamp(screen_x_to_freq(pos.x, rect), MIN_FREQ_HZ, MAX_FREQ_HZ);
                        if band.filter_type.has_gain() {
                            band.gain_db =
                                clamp(screen_y_to_db(pos.y, rect, range_db), -30.0, 30.0);
                        }
                        response.mark_changed();
                    }
                }
            }
            if response.clicked() {
                *self.selected_band = hovered_band;
            }
            if response.double_clicked() {
                if let Some(band) = hovered_band.and_then(|id| {
                    self.bands.iter_mut().find(|b| b.id == id)
                }) {
                    band.gain_db = NEUTRAL_GAIN_DB;
                    band.q = NEUTRAL_Q;
                    response.mark_changed();
                }
            }
            if let Some(id) = hovered_band {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    let fine = ui.input(|i| i.modifiers.shift);
                    if let Some(band) = self.bands.iter_mut().find(|b| b.id == id) {
                        let step = if fine { 0.015 } else { 0.06 };
                        band.q = clamp(band.q * (1.0 + scroll.signum() * step), 0.1, 40.0);
                        response.mark_changed();
                    }
                }
            }
        }

        // 2. Keyboard editing of the selected band.
        if response.has_focus() {
            if let Some(id) = *self.selected_band {
                let (left, right, up, down, shift) = ui.input(|i| {
                    (
                        i.key_pressed(Key::ArrowLeft),
                        i.key_pressed(Key::ArrowRight),
                        i.key_pressed(Key::ArrowUp),
                        i.key_pressed(Key::ArrowDown),
                        i.modifiers.shift,
                    )
                });
                if let Some(band) = self.bands.iter_mut().find(|b| b.id == id) {
                    let mut changed = false;
                    if left || right {
                        let semitones = if shift { 1.0 / 48.0 } else { 1.0 / 12.0 };
                        let factor = 2.0f32.powf(if left { -semitones } else { semitones });
                        band.frequency = clamp(band.frequency * factor, MIN_FREQ_HZ, MAX_FREQ_HZ);
                        changed = true;
                    }
                    if up || down {
                        let sign = if up { 1.0 } else { -1.0 };
                        if band.filter_type.has_gain() {
                            band.gain_db =
                                clamp(band.gain_db + sign * if shift { 0.1 } else { 0.5 }, -30.0, 30.0);
                        } else {
                            let step = if shift { 1.015 } else { 1.06 };
                            band.q = clamp(band.q * if up { step } else { 1.0 / step }, 0.1, 40.0);
                        }
                        changed = true;
                    }
                    if changed {
                        response.mark_changed();
                    }
                }
            }
        }

        if hovered_band.is_some() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }

        // 3. Chassis and grid.
        let painter = ui.painter();
        painter.rect_filled(rect, theme.radius.md(), theme.palette.surface);
        painter.rect_stroke(
            rect,
            theme.radius.md(),
            Stroke::new(1.0_f32, theme.palette.border),
            egui::StrokeKind::Inside,
        );

        let faint_grid = with_alpha(theme.palette.text_dim, 0.12);
        let solid_grid = with_alpha(theme.palette.text_dim, 0.34);
        let label_font = FontId::proportional((theme.size.label_font() - 2.0).max(7.0));

        let db_ticks = [
            range_db,
            range_db * (2.0 / 3.0),
            range_db / 3.0,
            0.0,
            -range_db / 3.0,
            -range_db * (2.0 / 3.0),
            -range_db,
        ];
        for &db in &db_ticks {
            let y = db_to_screen_y(db, rect, range_db);
            let zero = db.abs() < 0.01;
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(if zero { 1.1_f32 } else { 0.6_f32 }, if zero { solid_grid } else { faint_grid }),
            );
            painter.text(
                Pos2::new(rect.min.x + 6.0, y - 2.0),
                Align2::LEFT_BOTTOM,
                if zero { "0 dB".to_owned() } else { format!("{db:+.0}") },
                label_font.clone(),
                theme.palette.text_dim,
            );
        }

        for (f, label) in FREQ_TICKS {
            let x = freq_to_screen_x(f, rect);
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(0.6_f32, faint_grid),
            );
            painter.text(
                Pos2::new(x, rect.max.y - 4.0),
                Align2::CENTER_BOTTOM,
                label,
                label_font.clone(),
                theme.palette.text_dim,
            );
        }

        // 4. Optional background spectrum.
        if let Some(spectrum) = self.spectrum {
            if spectrum.len() >= 2 {
                let last = (spectrum.len() - 1) as f32;
                let points: Vec<Pos2> = spectrum
                    .iter()
                    .enumerate()
                    .map(|(i, amp)| {
                        Pos2::new(
                            rect.min.x + (i as f32 / last) * rect.width(),
                            rect.max.y - amp.clamp(0.0, 1.0) * rect.height() * 0.85,
                        )
                    })
                    .collect();
                fill_under(
                    painter,
                    &points,
                    rect.max.y,
                    with_alpha(theme.palette.primary, 0.07),
                );
            }
        }

        // 5. Evaluate the bands.
        let soloing = self.bands.iter().any(|band| band.enabled && band.solo);
        let contributes = |band: &EqBand| band.enabled && (!soloing || band.solo);

        let mut composite = Vec::with_capacity(EVAL_POINTS);
        let mut individual: Vec<Vec<Pos2>> = if self.show_individual_curves {
            vec![Vec::with_capacity(EVAL_POINTS); self.bands.len()]
        } else {
            Vec::new()
        };

        for i in 0..EVAL_POINTS {
            let t = i as f32 / (EVAL_POINTS - 1) as f32;
            let freq = norm_x_to_freq(t);
            let x = rect.min.x + t * rect.width();
            let mut total = 0.0;
            for (index, band) in self.bands.iter().enumerate() {
                let db = if contributes(band) {
                    band.magnitude_db(freq, sample_rate)
                } else {
                    0.0
                };
                total += db;
                if self.show_individual_curves {
                    individual[index].push(Pos2::new(x, db_to_screen_y(db, rect, range_db)));
                }
            }
            composite.push(Pos2::new(x, db_to_screen_y(total, rect, range_db)));
        }

        if self.show_individual_curves {
            for (index, band) in self.bands.iter().enumerate() {
                if !contributes(band) {
                    continue;
                }
                painter.add(egui::Shape::line(
                    std::mem::take(&mut individual[index]),
                    Stroke::new(1.0_f32, with_alpha(band.color, 0.34)),
                ));
            }
        }

        let zero_y = db_to_screen_y(0.0, rect, range_db);
        fill_under(painter, &composite, zero_y, with_alpha(theme.palette.primary, 0.10));
        painter.add(egui::Shape::line(
            composite,
            Stroke::new(2.2_f32, theme.palette.primary),
        ));

        // 6. Handles and the readout of the band under the pointer.
        if self.show_handles {
            for band in self.bands.iter() {
                if !band.enabled {
                    continue;
                }
                let centre = self.handle_position(band, rect);
                let hovered = hovered_band == Some(band.id);
                let selected = *self.selected_band == Some(band.id);
                let dim = !contributes(band);
                let ink = if dim { with_alpha(band.color, 0.45) } else { band.color };

                if hovered || selected {
                    let halo = if selected { 13.0 } else { 11.0 };
                    painter.circle_filled(centre, halo, with_alpha(ink, 0.20));
                    painter.circle_stroke(centre, halo, Stroke::new(1.0_f32, ink));
                }
                painter.circle_filled(centre, HANDLE_DOT, ink);
                painter.circle_stroke(
                    centre,
                    HANDLE_DOT,
                    Stroke::new(1.2_f32, with_alpha(theme.palette.background, 0.75)),
                );
                painter.text(
                    centre,
                    Align2::CENTER_CENTER,
                    band.id.to_string(),
                    FontId::proportional(8.0),
                    theme.palette.primary_foreground,
                );
            }

            if let Some(band) = hovered_band
                .and_then(|id| self.bands.iter().find(|b| b.id == id))
                .filter(|band| band.enabled)
            {
                let centre = self.handle_position(band, rect);
                let gain = if band.filter_type.has_gain() {
                    format!("  {}", format_gain(band.gain_db))
                } else {
                    String::new()
                };
                let text = format!(
                    "B{} · {}{} · Q {:.2}",
                    band.id,
                    format_frequency(band.frequency),
                    gain,
                    band.q
                );
                let font = FontId::proportional((theme.size.label_font() + 0.5).max(9.0));
                let galley = painter.layout_no_wrap(text, font, theme.palette.foreground);
                let badge = Rect::from_min_size(
                    Pos2::new(
                        (centre.x - galley.size().x * 0.5 - 7.0)
                            .clamp(rect.min.x + 4.0, rect.max.x - galley.size().x - 18.0),
                        (centre.y - galley.size().y - 18.0).max(rect.min.y + 4.0),
                    ),
                    galley.size() + Vec2::new(14.0, 8.0),
                );
                painter.rect_filled(badge, theme.radius.sm(), theme.palette.popover);
                painter.rect_stroke(
                    badge,
                    theme.radius.sm(),
                    Stroke::new(1.0_f32, with_alpha(band.color, 0.65)),
                    egui::StrokeKind::Inside,
                );
                painter.galley(
                    badge.min + Vec2::new(7.0, 4.0),
                    galley,
                    theme.palette.foreground,
                );
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::eq::filter_type::FilterType;

    fn sample_bands() -> [EqBand; 4] {
        [
            EqBand::new(1, FilterType::LowCut, 40.0, 0.0, 0.707),
            EqBand::new(2, FilterType::LowShelf, 120.0, 3.5, 0.71),
            EqBand::new(3, FilterType::Bell, 1200.0, -4.0, 1.41),
            EqBand::new(4, FilterType::HighShelf, 8000.0, 2.5, 0.71),
        ]
    }

    #[test]
    fn freq_norm_round_trip() {
        for f in [20.0, 100.0, 1000.0, 5000.0, 20_000.0] {
            let nx = freq_to_norm_x(f);
            let back = norm_x_to_freq(nx);
            assert!((f - back).abs() < 0.1, "freq {f} should round trip, got {back}");
        }
    }

    #[test]
    fn the_log_axis_is_even_in_octaves() {
        // Halfway along 20 Hz .. 20 kHz is the geometric mean, 632 Hz.
        assert!((freq_to_norm_x(632.4555) - 0.5).abs() < 1e-4);
        // 2 kHz is two of the three decades between the ends.
        assert!((freq_to_norm_x(2000.0) - 2.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn solo_mutes_the_other_bands() {
        let mut bands = sample_bands();
        let mut selected = None;
        bands[2].solo = true;
        let curve = EqCurve::new(&mut bands, &mut selected);
        assert!(curve.response_db(120.0).abs() < 0.5, "the shelf is muted by the solo");
        assert!(curve.response_db(1200.0) < -3.5);
    }

    #[test]
    fn handle_round_trips_through_screen_space() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let band = EqBand::bell(1, 1000.0, 6.0, 1.0);
        let mut bands = [band.clone()];
        let mut selected = None;
        let curve = EqCurve::new(&mut bands, &mut selected);
        let at = curve.handle_position(&band, rect);
        assert!((screen_x_to_freq(at.x, rect) - 1000.0).abs() < 1.0);
        assert!((screen_y_to_db(at.y, rect, DEFAULT_EQ_RANGE_DB) - 6.0).abs() < 0.1);
        assert_eq!(curve.band_at(at, rect), Some(1));
    }

    #[test]
    fn readouts_use_engineering_units() {
        assert_eq!(format_frequency(440.0), "440 Hz");
        assert_eq!(format_frequency(1000.0), "1.00 kHz");
        assert_eq!(format_frequency(12_500.0), "12.5 kHz");
        assert_eq!(format_gain(3.0), "+3.0 dB");
        assert_eq!(format_gain(-3.0), "-3.0 dB");
    }

    #[test]
    fn eq_curve_renders_headlessly() {
        let mut bands = sample_bands();
        let mut selected = Some(3);
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = ui.add(EqCurve::new(&mut bands, &mut selected).size(420.0, 240.0));
        });
        assert!(!output.shapes.is_empty(), "EQ curve should emit shapes");
    }
}
