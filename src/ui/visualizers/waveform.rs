//! An interactive audio clip waveform with playhead, seeking, regions, and markers.
//!
//! Visual language and behavior directly follow audiocn's `waveform.tsx`.
//! Renders dual-pass played/unplayed color splits, scrub/seek interaction,
//! hover time tooltips, looping regions, and cue markers.

use egui::{
    Color32, FontFamily, FontId, Key, Rect, Response, Sense, Stroke,
    Ui, Vec2, Widget, pos2,
};

use crate::db::clamp;
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::spectrum::resample_levels;

/// Waveform render style.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WaveformVariant {
    /// Upward vertical bars from the bottom.
    Bars,
    /// Symmetric bars mirrored around the vertical middle (audiocn default).
    #[default]
    Mirror,
    /// Continuous envelope polygon.
    Line,
}

/// Format seconds into standard time string (`MM:SS` or `H:MM:SS`).
#[must_use]
pub fn format_time(seconds: f32) -> String {
    if seconds.is_nan() || seconds < 0.0 {
        return "00:00".to_string();
    }
    let total_secs = seconds.floor() as u64;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;

    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// A cue marker placed at a timestamp on the waveform timeline.
#[derive(Clone, Debug)]
pub struct WaveformMarker<'a> {
    /// Timestamp in seconds.
    pub time_secs: f32,
    /// Optional label (e.g. "Verse", "Chorus", "Drop").
    pub label: Option<&'a str>,
    /// Optional custom color.
    pub color: Option<Color32>,
}

impl<'a> WaveformMarker<'a> {
    /// Create a new marker at `time_secs`.
    pub fn new(time_secs: f32) -> Self {
        Self {
            time_secs,
            label: None,
            color: None,
        }
    }

    /// Label for the marker.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Custom marker flag color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// A highlighted region of the waveform (e.g. loop selection or active trim).
#[derive(Clone, Debug)]
pub struct WaveformRegion<'a> {
    /// Region start time in seconds.
    pub start_secs: f32,
    /// Region end time in seconds.
    pub end_secs: f32,
    /// Optional region name.
    pub label: Option<&'a str>,
    /// Tint color for region fill and borders.
    pub color: Option<Color32>,
}

impl<'a> WaveformRegion<'a> {
    /// Create a new region between `start_secs` and `end_secs`.
    pub fn new(start_secs: f32, end_secs: f32) -> Self {
        Self {
            start_secs,
            end_secs,
            label: None,
            color: None,
        }
    }

    /// Region label.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Region color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// An interactive audio file / buffer waveform display.
///
/// ```ignore
/// let peaks = [0.1, 0.4, 0.9, 0.7, 0.3, 0.2];
/// let mut progress = 0.35f32;
/// ui.add(Waveform::new(&peaks, &mut progress).duration(120.0));
/// ```
#[must_use = "a Waveform draws nothing until it is added to a Ui"]
pub struct Waveform<'a> {
    peaks: &'a [f32],
    progress: &'a mut f32,
    duration: f32,
    variant: WaveformVariant,
    interactive: bool,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
    show_playhead: bool,
    show_hover_time: bool,
    regions: &'a [WaveformRegion<'a>],
    markers: &'a [WaveformMarker<'a>],
    played_color: Option<Color32>,
    unplayed_color: Option<Color32>,
    cursor_color: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> Waveform<'a> {
    /// Create a new waveform widget with peak values (0.0..=1.0) and mutable progress (0.0..=1.0).
    pub fn new(peaks: &'a [f32], progress: &'a mut f32) -> Self {
        Self {
            peaks,
            progress,
            duration: 60.0,
            variant: WaveformVariant::Mirror,
            interactive: true,
            bar_width: 3.0,
            bar_gap: 1.0,
            bar_radius: 1.5,
            show_playhead: true,
            show_hover_time: true,
            regions: &[],
            markers: &[],
            played_color: None,
            unplayed_color: None,
            cursor_color: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Total audio duration in seconds.
    pub fn duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.1);
        self
    }

    /// Visual presentation style (`Mirror`, `Bars`, `Line`).
    pub fn variant(mut self, variant: WaveformVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Allow clicking and dragging to seek.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Bar width in points (default 3.0).
    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = width.max(1.0);
        self
    }

    /// Gap between bars in points (default 1.0).
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = gap.max(0.0);
        self
    }

    /// Bar corner radius (default 1.5).
    pub fn bar_radius(mut self, radius: f32) -> Self {
        self.bar_radius = radius.max(0.0);
        self
    }

    /// Whether to draw the vertical playhead line.
    pub fn show_playhead(mut self, show: bool) -> Self {
        self.show_playhead = show;
        self
    }

    /// Show hover position timestamp badge on mouseover.
    pub fn show_hover_time(mut self, show: bool) -> Self {
        self.show_hover_time = show;
        self
    }

    /// Attach regions (e.g. loop section, verses).
    pub fn regions(mut self, regions: &'a [WaveformRegion<'a>]) -> Self {
        self.regions = regions;
        self
    }

    /// Attach point markers (e.g. cues, beats).
    pub fn markers(mut self, markers: &'a [WaveformMarker<'a>]) -> Self {
        self.markers = markers;
        self
    }

    /// Custom color for the played portion (before playhead).
    pub fn played_color(mut self, color: Color32) -> Self {
        self.played_color = Some(color);
        self
    }

    /// Custom color for the unplayed portion (after playhead).
    pub fn unplayed_color(mut self, color: Color32) -> Self {
        self.unplayed_color = Some(color);
        self
    }

    /// Custom color for the playhead cursor line.
    pub fn cursor_color(mut self, color: Color32) -> Self {
        self.cursor_color = Some(color);
        self
    }

    /// Explicit widget width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Explicit widget height.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let (default_w, default_h) = match size {
            Size::Small => (200.0, 48.0),
            Size::Medium => (320.0, 68.0),
            Size::Large => (440.0, 96.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }
}

impl Widget for Waveform<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let sense = if self.interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };

        let (rect, mut response) = ui.allocate_exact_size(desired, sense);

        // Handle seeking interaction
        if self.interactive {
            if response.dragged() || response.clicked() {
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    let norm = ((mouse_pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                    *self.progress = norm;
                    response.mark_changed();
                }
            }

            if response.has_focus() {
                let step = 0.02f32;
                if ui.input(|i| i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown)) {
                    *self.progress = (*self.progress - step).clamp(0.0, 1.0);
                    response.mark_changed();
                } else if ui.input(|i| i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp)) {
                    *self.progress = (*self.progress + step).clamp(0.0, 1.0);
                    response.mark_changed();
                } else if ui.input(|i| i.key_pressed(Key::Home)) {
                    *self.progress = 0.0;
                    response.mark_changed();
                } else if ui.input(|i| i.key_pressed(Key::End)) {
                    *self.progress = 1.0;
                    response.mark_changed();
                }
            }
        }

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter();
        let w = rect.width();
        let h = rect.height();
        let mid_y = rect.center().y;

        let pitch = self.bar_width + self.bar_gap;
        let bar_count = ((w + self.bar_gap) / pitch).floor() as usize;

        // 1. Draw regions (e.g. loops) behind waveform
        for region in self.regions {
            let start_norm = (region.start_secs / self.duration).clamp(0.0, 1.0);
            let end_norm = (region.end_secs / self.duration).clamp(0.0, 1.0);
            if end_norm > start_norm {
                let rx0 = rect.min.x + start_norm * w;
                let rx1 = rect.min.x + end_norm * w;
                let r_rect = Rect::from_min_max(pos2(rx0, rect.min.y), pos2(rx1, rect.max.y));

                let r_color = region.color.unwrap_or(theme.palette.primary);
                painter.rect_filled(r_rect, 2.0, r_color.linear_multiply(0.18));
                painter.line_segment([pos2(rx0, rect.min.y), pos2(rx0, rect.max.y)], Stroke::new(1.0f32, r_color));
                painter.line_segment([pos2(rx1, rect.min.y), pos2(rx1, rect.max.y)], Stroke::new(1.0f32, r_color));

                if let Some(lbl) = region.label {
                    let font_id = FontId::new(theme.size.label_font(), FontFamily::Proportional);
                    painter.text(pos2(rx0 + 4.0, rect.min.y + 2.0), egui::Align2::LEFT_TOP, lbl, font_id, r_color);
                }
            }
        }

        // 2. Resample peak data
        let mut levels = vec![0.05f32; bar_count.max(1)];
        if !self.peaks.is_empty() {
            resample_levels(self.peaks, &mut levels);
        }

        let progress_norm = (*self.progress).clamp(0.0, 1.0);
        let split_x = rect.min.x + progress_norm * w;

        let played_color = self.played_color.unwrap_or(theme.palette.primary);
        let unplayed_color = self.unplayed_color.unwrap_or(theme.palette.track);

        // 3. Render waveform data (bars or line) with played / unplayed coloring
        match self.variant {
            WaveformVariant::Bars | WaveformVariant::Mirror => {
                for (i, &lvl) in levels.iter().enumerate() {
                    let level = clamp(lvl, 0.05, 1.0);
                    let bar_h = (level * h).clamp(2.0, h);
                    let x = rect.min.x + i as f32 * pitch;

                    let y = match self.variant {
                        WaveformVariant::Mirror => mid_y - bar_h * 0.5,
                        _ => rect.max.y - bar_h,
                    };

                    let bar_rect = Rect::from_min_size(pos2(x, y), Vec2::new(self.bar_width, bar_h));
                    let color = if x + self.bar_width <= split_x {
                        played_color
                    } else {
                        unplayed_color
                    };

                    painter.rect_filled(bar_rect, self.bar_radius, color);
                }
            }
            WaveformVariant::Line => {
                let mut points = Vec::with_capacity(bar_count);
                for (i, &lvl) in levels.iter().enumerate() {
                    let level = clamp(lvl, 0.05, 1.0);
                    let x = rect.min.x + i as f32 * pitch + self.bar_width * 0.5;
                    let y = mid_y - (level - 0.5) * (h - 2.0);
                    points.push(pos2(x, y));
                }

                for i in 0..points.len().saturating_sub(1) {
                    let p0 = points[i];
                    let p1 = points[i + 1];
                    let color = if p1.x <= split_x {
                        played_color
                    } else {
                        unplayed_color
                    };
                    painter.line_segment([p0, p1], Stroke::new(1.75f32, color));
                }
            }
        }

        // 4. Draw markers
        for marker in self.markers {
            let m_norm = (marker.time_secs / self.duration).clamp(0.0, 1.0);
            let mx = rect.min.x + m_norm * w;
            let m_color = marker.color.unwrap_or(theme.palette.meter_warn);

            painter.line_segment(
                [pos2(mx, rect.min.y), pos2(mx, rect.max.y)],
                Stroke::new(1.0f32, m_color.linear_multiply(0.7)),
            );

            // Little flag tick at the top
            painter.line_segment(
                [pos2(mx, rect.min.y), pos2(mx + 6.0, rect.min.y + 4.0)],
                Stroke::new(1.5f32, m_color),
            );

            if let Some(lbl) = marker.label {
                let font_id = FontId::new(theme.size.label_font(), FontFamily::Proportional);
                painter.text(pos2(mx + 3.0, rect.min.y + 6.0), egui::Align2::LEFT_TOP, lbl, font_id, m_color);
            }
        }

        // 5. Playhead cursor
        if self.show_playhead {
            let cursor_col = self.cursor_color.unwrap_or(theme.palette.foreground);
            painter.line_segment(
                [pos2(split_x, rect.min.y), pos2(split_x, rect.max.y)],
                Stroke::new(1.75f32, cursor_col),
            );

            // Small playhead handle circle at top
            painter.circle_filled(pos2(split_x, rect.min.y + 3.0), 3.0, cursor_col);
        }

        // 6. Hover line and timestamp tooltip badge
        if self.interactive && self.show_hover_time {
            if let Some(hover_pos) = response.hover_pos() {
                if rect.contains(hover_pos) {
                    let hover_norm = ((hover_pos.x - rect.min.x) / w).clamp(0.0, 1.0);
                    let hover_x = rect.min.x + hover_norm * w;
                    let hover_secs = hover_norm * self.duration;

                    let hover_col = theme.palette.text_dim.linear_multiply(0.5);
                    painter.line_segment(
                        [pos2(hover_x, rect.min.y), pos2(hover_x, rect.max.y)],
                        Stroke::new(1.0f32, hover_col),
                    );

                    let badge_text = format_time(hover_secs);
                    let font_id = FontId::new(theme.size.label_font(), FontFamily::Monospace);
                    let badge_pos = pos2(hover_x, rect.min.y - 12.0);

                    // Badge text
                    painter.text(
                        badge_pos,
                        egui::Align2::CENTER_CENTER,
                        badge_text,
                        font_id,
                        theme.palette.foreground,
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
    fn format_time_formats_standard_durations() {
        assert_eq!(format_time(0.0), "00:00");
        assert_eq!(format_time(65.0), "01:05");
        assert_eq!(format_time(3665.0), "1:01:05");
    }

    #[test]
    fn waveform_renders_with_markers_and_regions() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let peaks = [0.1, 0.4, 0.8, 0.6, 0.2];
            let mut progress = 0.5f32;
            let regions = [WaveformRegion::new(10.0, 30.0).label("Verse")];
            let markers = [WaveformMarker::new(45.0).label("Drop")];

            let resp = ui.add(
                Waveform::new(&peaks, &mut progress)
                    .duration(120.0)
                    .variant(WaveformVariant::Mirror)
                    .regions(&regions)
                    .markers(&markers),
            );
            assert!(resp.rect.width() > 0.0);
            assert!(resp.rect.height() > 0.0);
        });
    }
}
