//! Polyline fills for the line visualizers: per-vertex alpha for the faded
//! ends, and ribbons between two copies of a line for the soft glow.

use egui::{Color32, Mesh, Painter, Pos2, Vec2};

use crate::db::clamp;
use crate::electric::with_alpha;

fn edge_share(point: Pos2, left: f32, right: f32, fade: f32) -> f32 {
    if fade <= 0.0 {
        return 1.0;
    }
    let from_left = (point.x - left) / fade;
    let from_right = (right - point.x) / fade;
    clamp(from_left.min(from_right), 0.0, 1.0)
}

/// A stroke of `width` along `points`, with alpha fading out over `fade` points
/// at each end.
pub fn faded_line(
    painter: &Painter,
    points: &[Pos2],
    width: f32,
    color: Color32,
    fade: f32,
    alpha: f32,
) {
    if points.len() < 2 {
        return;
    }
    let left = points.first().map_or(0.0, |point| point.x);
    let right = points.last().map_or(0.0, |point| point.x);
    let half = width * 0.5;
    let mut mesh = Mesh::default();
    for segment in points.windows(2) {
        let (from, to) = (segment[0], segment[1]);
        let delta = to - from;
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }
        let normal = Vec2::new(-delta.y, delta.x) / length * half;
        let alpha_from = edge_share(from, left, right, fade) * alpha;
        let alpha_to = edge_share(to, left, right, fade) * alpha;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(from + normal, with_alpha(color, alpha_from));
        mesh.colored_vertex(from - normal, with_alpha(color, alpha_from));
        mesh.colored_vertex(to + normal, with_alpha(color, alpha_to));
        mesh.colored_vertex(to - normal, with_alpha(color, alpha_to));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    painter.add(mesh);
}

/// The band between two scaled copies of the same line, for a soft glow.
pub fn ribbon(
    painter: &Painter,
    outer: &[Pos2],
    inner: &[Pos2],
    color: Color32,
    alpha: f32,
    fade: f32,
) {
    if outer.len() < 2 || outer.len() != inner.len() {
        return;
    }
    let left = outer.first().map_or(0.0, |point| point.x);
    let right = outer.last().map_or(0.0, |point| point.x);
    let mut mesh = Mesh::default();
    for index in 0..outer.len() - 1 {
        let base = mesh.vertices.len() as u32;
        let alpha_from = alpha * edge_share(outer[index], left, right, fade);
        let alpha_to = alpha * edge_share(outer[index + 1], left, right, fade);
        mesh.colored_vertex(outer[index], with_alpha(color, alpha_from));
        mesh.colored_vertex(inner[index], with_alpha(color, alpha_from));
        mesh.colored_vertex(outer[index + 1], with_alpha(color, alpha_to));
        mesh.colored_vertex(inner[index + 1], with_alpha(color, alpha_to));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    painter.add(mesh);
}

/// [`ribbon`] softened by stacked vertical copies, standing in for the CSS
/// blur audiocn puts on its glow canvas.
pub fn soft_ribbon(
    painter: &Painter,
    outer: &[Pos2],
    inner: &[Pos2],
    color: Color32,
    alpha: f32,
    fade: f32,
    blur: f32,
) {
    const OFFSETS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];
    if blur <= 0.0 {
        ribbon(painter, outer, inner, color, alpha, fade);
        return;
    }
    let share = alpha / OFFSETS.len() as f32;
    let mut shifted_outer = Vec::with_capacity(outer.len());
    let mut shifted_inner = Vec::with_capacity(inner.len());
    for offset in OFFSETS {
        let shift = Vec2::new(0.0, offset * blur);
        shifted_outer.clear();
        shifted_outer.extend(outer.iter().map(|point| *point + shift));
        shifted_inner.clear();
        shifted_inner.extend(inner.iter().map(|point| *point + shift));
        ribbon(
            painter,
            &shifted_outer,
            &shifted_inner,
            color,
            share,
            fade,
        );
    }
}
