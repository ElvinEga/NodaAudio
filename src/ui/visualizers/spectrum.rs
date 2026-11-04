//! A realtime frequency spectrum visualizer with bars, line, and area modes.
//!
//! Visual language and behavior directly follow audiocn's `spectrum.tsx` and `bands.ts`.
//! Accepts FFT bin magnitudes in dB or pre-computed logarithmic bands, supports
//! peak hold, and draws optional frequency/decibel grid reference lines.

use egui::{
    FontFamily, FontId, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::taper::Taper;
use crate::ui::theme::{Size, Theme};

/// Default minimum frequency in Hz.
pub const DEFAULT_MIN_HZ: f32 = 20.0;
/// Default maximum frequency in Hz.
pub const DEFAULT_MAX_HZ: f32 = 20000.0;
/// Default minimum decibel floor.
pub const DEFAULT_MIN_DB: f32 = -90.0;
/// Default maximum decibel ceiling.
pub const DEFAULT_MAX_DB: f32 = 0.0;

/// Default frequency grid lines in Hz.
pub const DEFAULT_FREQUENCY_TICKS: &[f32] = &[100.0, 1000.0, 10000.0];

/// Visual style of the spectrum display.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpectrumVariant {
    /// Discrete vertical bars for each frequency band.
    #[default]
    Bars,
    /// Continuous line tracing the spectrum envelope.
    Line,
    /// Filled area below the spectrum envelope with translucent wash.
    Area,
}

/// Compute `count + 1` band boundary edges in Hz, spaced logarithmically.
#[must_use]
pub fn log_band_edges(count: usize, min_hz: f32, max_hz: f32) -> Vec<f32> {
    if count == 0 {
        return vec![min_hz];
    }
    let ratio = max_hz / min_hz.max(1.0);
    (0..=count)
        .map(|i| min_hz * ratio.powf(i as f32 / count as f32))
        .collect()
}

/// Resample/reduce FFT spectrum dB bins into 0.0..=1.0 band energies across `edges`.
#[must_use]
pub fn bands_from_spectrum(
    spectrum_db: &[f32],
    sample_rate: f32,
    edges: &[f32],
    min_db: f32,
    max_db: f32,
) -> Vec<f32> {
    if spectrum_db.is_empty() || edges.len() < 2 {
        return Vec::new();
    }
    let bin_count = spectrum_db.len();
    let hz_per_bin = (sample_rate * 0.5) / bin_count as f32;
    let band_count = edges.len() - 1;
    let mut bands = Vec::with_capacity(band_count);
    let span = (max_db - min_db).max(1.0);

    for band in 0..band_count {
        let low_bin = ((edges[band] / hz_per_bin).floor() as usize).min(bin_count.saturating_sub(1));
        let high_bin = ((edges[band + 1] / hz_per_bin).ceil() as usize).clamp(low_bin + 1, bin_count);

        let mut power_sum = 0.0f32;
        let count = (high_bin - low_bin) as f32;
        for bin in low_bin..high_bin {
            let db = spectrum_db[bin].clamp(min_db, max_db);
            // Power decibels: 10^(dB/10)
            power_sum += 10.0f32.powf(db / 10.0);
        }

        let mean_power = (power_sum / count.max(1.0)).max(1e-12);
        let mean_db = 10.0 * mean_power.log10();
        let norm = ((mean_db - min_db) / span).clamp(0.0, 1.0);
        bands.push(norm);
    }

    bands
}

/// Fit `source` levels into `out`, taking the peak value within each target span.
pub fn resample_levels(source: &[f32], out: &mut [f32]) {
    let src_len = source.len();
    let out_len = out.len();
    if src_len == 0 || out_len == 0 {
        out.fill(0.0);
        return;
    }

    for (i, val) in out.iter_mut().enumerate() {
        let from = (i * src_len) / out_len;
        let to = (((i + 1) * src_len) / out_len).max(from + 1).min(src_len);
        let mut peak = 0.0f32;
        for &s in &source[from..to] {
            if s > peak {
                peak = s;
            }
        }
        *val = peak;
    }
}

/// A spectrum visualizer widget.
///
/// ```ignore
/// let bands = [0.1, 0.4, 0.8, 0.6, 0.3, 0.2];
/// ui.add(Spectrum::new(&bands).variant(SpectrumVariant::Area));
/// ```
#[must_use = "a Spectrum draws nothing until it is added to a Ui"]
pub struct Spectrum<'a> {
    bands: &'a [f32],
    variant: SpectrumVariant,
    min_db: f32,
    max_db: f32,
    min_hz: f32,
    max_hz: f32,
    grid: bool,
    peak_hold: bool,
    peaks: Option<&'a mut [f32]>,
    peak_decay: f32,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> Spectrum<'a> {
    /// Create a new spectrum visualizer with pre-normalized band magnitudes (0.0..=1.0).
    pub fn new(bands: &'a [f32]) -> Self {
        Self {
            bands,
            variant: SpectrumVariant::Bars,
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_MAX_DB,
            min_hz: DEFAULT_MIN_HZ,
            max_hz: DEFAULT_MAX_HZ,
            grid: true,
            peak_hold: false,
            peaks: None,
            peak_decay: 0.96,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Display variant (`Bars`, `Line`, `Area`).
    pub fn variant(mut self, variant: SpectrumVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Decibel range for grid labels and scaling.
    pub fn db_range(mut self, min_db: f32, max_db: f32) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Frequency range in Hz.
    pub fn hz_range(mut self, min_hz: f32, max_hz: f32) -> Self {
        self.min_hz = min_hz;
        self.max_hz = max_hz;
        self
    }

    /// Whether to render background frequency & dB grid lines.
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Enable peak hold markers, updating `peaks` frame-by-frame.
    pub fn peak_hold(mut self, peaks: &'a mut [f32]) -> Self {
        self.peak_hold = true;
        self.peaks = Some(peaks);
        self
    }

    /// Explicit width in points.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Explicit height in points.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let (default_w, default_h) = match size {
            Size::Small => (180.0, 70.0),
            Size::Medium => (260.0, 100.0),
            Size::Large => (340.0, 140.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }

    fn format_hz(hz: f32) -> String {
        if hz >= 1000.0 {
            format!("{}k", (hz / 1000.0).round() as i32)
        } else {
            format!("{}", hz.round() as i32)
        }
    }
}

impl Widget for Spectrum<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter();
        let bg_color = theme.palette.track;
        let rounding = theme.radius.sm();

        // 1. Background container
        painter.rect_filled(rect, rounding, bg_color);
        painter.rect_stroke(
            rect,
            rounding,
            Stroke::new(1.0f32, theme.palette.border),
            StrokeKind::Inside,
        );

        let plot_rect = rect.shrink(3.0);
        let w = plot_rect.width();
        let h = plot_rect.height();

        // 2. Grid lines
        if self.grid {
            let grid_color = theme.palette.text_dim.linear_multiply(0.2);
            let text_color = theme.palette.text_dim.linear_multiply(0.6);
            let font_id = FontId::new(theme.size.label_font(), FontFamily::Monospace);

            // Horizontal dB grid lines (3 divisions)
            for div in 1..3 {
                let y = plot_rect.min.y + (h * div as f32 / 3.0);
                painter.line_segment(
                    [pos2(plot_rect.min.x, y), pos2(plot_rect.max.x, y)],
                    Stroke::new(1.0f32, grid_color),
                );
            }

            // Vertical frequency ticks (100 Hz, 1 kHz, 10 kHz)
            let log_taper = Taper::Log;
            for &hz in DEFAULT_FREQUENCY_TICKS {
                if hz > self.min_hz && hz < self.max_hz {
                    let norm_x = log_taper.to_position(hz, self.min_hz, self.max_hz);
                    let x = plot_rect.min.x + norm_x * w;
                    painter.line_segment(
                        [pos2(x, plot_rect.min.y), pos2(x, plot_rect.max.y)],
                        Stroke::new(1.0f32, grid_color),
                    );

                    painter.text(
                        pos2(x + 2.0, plot_rect.min.y + 2.0),
                        egui::Align2::LEFT_TOP,
                        Self::format_hz(hz),
                        font_id.clone(),
                        text_color,
                    );
                }
            }
        }

        let bands = self.bands;
        let count = bands.len();
        if count == 0 {
            return response;
        }

        let primary_color = theme.palette.primary;

        // 3. Render spectrum data according to variant
        match self.variant {
            SpectrumVariant::Bars => {
                let slot_w = w / count as f32;
                let gap = (slot_w * 0.25).clamp(1.0, 3.0);
                let bar_w = (slot_w - gap).max(1.0);

                for (i, &band) in bands.iter().enumerate() {
                    let val = clamp(band, 0.0, 1.0);
                    let bar_h = (val * h).max(1.0);
                    let bar_x = plot_rect.min.x + i as f32 * slot_w + (gap * 0.5);
                    let bar_y = plot_rect.max.y - bar_h;

                    let bar_rect = Rect::from_min_size(pos2(bar_x, bar_y), Vec2::new(bar_w, bar_h));
                    let bar_radius = (bar_w * 0.3).min(2.0);
                    painter.rect_filled(bar_rect, bar_radius, primary_color);
                }
            }
            SpectrumVariant::Line => {
                let mut points = Vec::with_capacity(count);
                let slot_w = w / count as f32;
                for (i, &band) in bands.iter().enumerate() {
                    let val = clamp(band, 0.0, 1.0);
                    let x = plot_rect.min.x + (i as f32 + 0.5) * slot_w;
                    let y = plot_rect.max.y - val * (h - 2.0) - 1.0;
                    points.push(pos2(x, y));
                }
                if points.len() >= 2 {
                    painter.line(points, Stroke::new(1.75f32, primary_color));
                }
            }
            SpectrumVariant::Area => {
                let mut points = Vec::with_capacity(count + 2);
                let slot_w = w / count as f32;

                points.push(pos2(plot_rect.min.x + 0.5 * slot_w, plot_rect.max.y));
                for (i, &band) in bands.iter().enumerate() {
                    let val = clamp(band, 0.0, 1.0);
                    let x = plot_rect.min.x + (i as f32 + 0.5) * slot_w;
                    let y = plot_rect.max.y - val * (h - 2.0) - 1.0;
                    points.push(pos2(x, y));
                }
                points.push(pos2(
                    plot_rect.min.x + (count as f32 - 0.5) * slot_w,
                    plot_rect.max.y,
                ));

                let fill_color = primary_color.linear_multiply(0.28);
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    fill_color,
                    Stroke::NONE,
                ));

                // Draw line stroke across the top boundary
                if points.len() > 3 {
                    painter.line(points[1..points.len() - 1].to_vec(), Stroke::new(1.75f32, primary_color));
                }
            }
        }

        // 4. Peak hold markers
        if self.peak_hold {
            if let Some(peaks) = self.peaks.as_mut() {
                let peak_color = theme.palette.meter_warn;
                let slot_w = w / count as f32;

                let len = peaks.len().min(count);
                for i in 0..len {
                    let cur = clamp(bands[i], 0.0, 1.0);
                    let held = (peaks[i] * self.peak_decay).max(cur);
                    peaks[i] = held;

                    let peak_y = plot_rect.max.y - held * (h - 2.0) - 2.0;
                    let px = plot_rect.min.x + i as f32 * slot_w;
                    let marker_w = (slot_w - 1.0).max(2.0);

                    painter.line_segment(
                        [pos2(px, peak_y), pos2(px + marker_w, peak_y)],
                        Stroke::new(1.5f32, peak_color),
                    );
                }
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_band_edges_ascend_correctly() {
        let edges = log_band_edges(4, 20.0, 20000.0);
        assert_eq!(edges.len(), 5);
        assert!((edges[0] - 20.0).abs() < 1e-4);
        assert!((edges[4] - 20000.0).abs() < 1.0);
        for i in 0..4 {
            assert!(edges[i] < edges[i + 1]);
        }
    }

    #[test]
    fn bands_from_spectrum_reduces_bins() {
        let spectrum = vec![-12.0; 512];
        let edges = log_band_edges(8, 20.0, 20000.0);
        let bands = bands_from_spectrum(&spectrum, 44100.0, &edges, -90.0, 0.0);
        assert_eq!(bands.len(), 8);
        for &b in &bands {
            assert!(b > 0.0 && b <= 1.0);
        }
    }

    #[test]
    fn resample_levels_peaks() {
        let src = [0.1, 0.9, 0.2, 0.3];
        let mut out = [0.0; 2];
        resample_levels(&src, &mut out);
        assert_eq!(out[0], 0.9);
        assert_eq!(out[1], 0.3);
    }

    #[test]
    fn spectrum_renders_headlessly() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let bands = [0.2, 0.5, 0.8, 0.6, 0.3];
            let resp = ui.add(Spectrum::new(&bands).variant(SpectrumVariant::Area));
            assert!(resp.rect.width() > 0.0);
            assert!(resp.rect.height() > 0.0);
        });
    }
}
