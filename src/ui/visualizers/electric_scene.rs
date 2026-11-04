//! The moving parts of the electric bar look, ported from audiocn's
//! `electric-bar-visualizer.tsx`.
//!
//! Bars are drawn as jagged filaments: a midpoint-displaced line per bar,
//! rerolled every 42 ms, with a soft glow, a body and a hot core pass. Arcs
//! jump between loud neighbouring bars, and sudden rises throw sparks. The
//! scene keeps its random sequence and particle pool in egui's temporary data,
//! so a widget rebuilt every frame crackles the same way a browser canvas does.

use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};

use crate::db::clamp;
use crate::electric::{Crackle, ElectricColors, SparkLaunch, Sparks, displace, with_alpha};
use crate::ui::visualizers::bar_visualizer::BarAlign;

/// audiocn's defaults.
pub const DEFAULT_BAR_COUNT: usize = 16;
pub const DEFAULT_INTENSITY: f32 = 0.6;
pub const DEFAULT_BAR_WIDTH: f32 = 6.0;
pub const DEFAULT_BAR_GAP: f32 = 4.0;

/// A filament has 2^4 segments, so every split halves one cleanly.
const FILAMENT_DEPTH: u32 = 4;
const FILAMENT_POINTS: usize = (1 << FILAMENT_DEPTH) + 1;
/// Shortest segment in pixels before a filament drops to fewer points.
const SEGMENT_PX: f32 = 5.0;
/// How much each finer split bends compared with the one before.
const ROUGHNESS: f32 = 0.72;
/// How far a free tip wanders, relative to the jitter amplitude.
const FREE_TIP: f32 = 0.5;
/// Each jagged shape holds at least this long, so the crackle never strobes.
const JITTER_MS: f64 = 42.0;
/// Largest bend, relative to the filament's length.
const MAX_BEND: f32 = 0.35;
/// Brightness flicker, at most 12%.
const FLICKER: f32 = 0.12;
/// Tips start to glow above this level.
const TIP_LEVEL: f32 = 0.25;

const MAX_ARCS: usize = 3;
/// Arcs jump to the next bar or the one after.
const MAX_ARC_REACH: usize = 2;
const ARC_DEPTH: u32 = 3;
const ARC_POINTS: usize = (1 << ARC_DEPTH) + 1;
const ARC_ATTEMPTS: usize = 2;
/// Both neighbours must be this loud for an arc to jump between them.
const ARC_THRESHOLD: f32 = 0.45;
const ARC_CHANCE: f32 = 0.35;
const ARC_MIN_MS: f32 = 70.0;
const ARC_MAX_MS: f32 = 150.0;
/// How far an arc bends, relative to the distance it spans.
const ARC_BOW: f32 = 0.3;

const MAX_SPARKS: usize = 64;
/// A rise this big in one frame throws sparks.
const SPARK_RISE: f32 = 0.12;
const SPARK_MIN_LEVEL: f32 = 0.3;
const SPARK_COOLDOWN_MS: f64 = 90.0;
/// Pixels per second squared.
const SPARK_GRAVITY: f32 = 520.0;
/// Pixels per second.
const SPARK_SPEED: f32 = 180.0;
/// Pixels per second, sideways.
const SPARK_SPREAD: f32 = 70.0;
const SPARK_MIN_MS: f32 = 260.0;
const SPARK_MAX_MS: f32 = 560.0;
pub const SPARK_TRAIL_SECONDS: f32 = 0.03;

const FRAME_SECONDS: f32 = 1.0 / 60.0;
const MAX_STEP_SECONDS: f32 = 0.05;

/// Which end of a bar an arc joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcTip {
    /// The base; centred bars only.
    From,
    /// The tip.
    To,
}

/// Where the bars sit, in points. `across` runs along the row of bars and
/// `along` runs the way they grow, so both orientations share one drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricLayout {
    pub horizontal: bool,
    pub align: BarAlign,
    pub bar_width: f32,
    pub bar_gap: f32,
    /// Centre of the first bar, across.
    pub first: f32,
    /// Distance between bar centres.
    pub pitch: f32,
    /// Where bars grow from, along: the base, or the middle when centred.
    pub base: f32,
    /// Length of a bar at level 1.
    pub span: f32,
    /// Position of the layout's origin on the painter, in screen points.
    pub origin: Pos2,
}

/// The inputs that decide the bar geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricLayoutOptions {
    pub align: BarAlign,
    pub bar_count: usize,
    pub bar_gap: f32,
    pub bar_width: f32,
    pub horizontal: bool,
}

/// Fits the bars into a rect, narrowing bars and gaps when space is short.
#[must_use]
pub fn layout_electric_bars(
    width: f32,
    height: f32,
    options: ElectricLayoutOptions,
) -> ElectricLayout {
    let horizontal = options.horizontal;
    let count = options.bar_count.max(1);
    let across_length = if horizontal { width } else { height };
    let along_length = if horizontal { height } else { width };
    let slot = options.bar_width + options.bar_gap;
    let pitch = slot.min(((across_length - options.bar_width * 2.0).max(0.0)) / count as f32);
    let scale = if slot > 0.0 { pitch / slot } else { 1.0 };
    let bar_width = options.bar_width * scale;
    let bar_gap = options.bar_gap * scale;
    let inset = bar_width.min(along_length / 4.0);
    let base = match options.align {
        BarAlign::Center => along_length * 0.5,
        BarAlign::End => along_length - inset,
        BarAlign::Start => inset,
    };
    ElectricLayout {
        horizontal,
        align: options.align,
        bar_width,
        bar_gap,
        first: (across_length - count as f32 * pitch + bar_gap + bar_width) * 0.5,
        pitch,
        base,
        span: (along_length - inset * 2.0).max(0.0),
        origin: Pos2::ZERO,
    }
}

/// Where a bar starts, along.
#[must_use]
pub fn bar_from(layout: &ElectricLayout, level: f32) -> f32 {
    if layout.align == BarAlign::Center {
        layout.base - level * layout.span * 0.5
    } else {
        layout.base
    }
}

/// Where a bar ends, along.
#[must_use]
pub fn bar_to(layout: &ElectricLayout, level: f32) -> f32 {
    if layout.align == BarAlign::Center {
        return layout.base + level * layout.span * 0.5;
    }
    let direction = if layout.align == BarAlign::End { -1.0 } else { 1.0 };
    layout.base + direction * level * layout.span
}

/// Which way a tip points, along: sparks fly that way.
#[must_use]
pub fn tip_direction(layout: &ElectricLayout, tip: ArcTip) -> f32 {
    let growth = if layout.align == BarAlign::End { -1.0 } else { 1.0 };
    if tip == ArcTip::To { growth } else { -growth }
}

/// How far a filament wanders sideways, in points. Louder bars crackle more,
/// and short ones never bend more than a third of their length.
#[must_use]
pub fn jitter_amplitude(layout: &ElectricLayout, intensity: f32, level: f32) -> f32 {
    (intensity * (layout.bar_width + layout.bar_gap) * 0.6 * (0.3 + 0.7 * level))
        .min(level * layout.span * MAX_BEND)
}

/// Short filaments use fewer points, so they stay a line and not a scribble.
#[must_use]
pub fn stride_for(length: f32) -> usize {
    let depth = ((length.max(1.0) / SEGMENT_PX).log2().ceil() as i32)
        .clamp(1, FILAMENT_DEPTH as i32) as u32;
    1 << (FILAMENT_DEPTH - depth)
}

fn plot(layout: &ElectricLayout, across: f32, along: f32) -> Pos2 {
    if layout.horizontal {
        layout.origin + Vec2::new(across, along)
    } else {
        layout.origin + Vec2::new(along, across)
    }
}

/// A jagged bolt jumping between two loud bars.
#[derive(Clone, Copy, Debug)]
pub struct ElectricArc {
    /// The arc joins this bar and the one `reach` bars on.
    pub index: usize,
    pub reach: usize,
    /// Which end of the bars it joins.
    pub tip: ArcTip,
    pub born_ms: f64,
    /// 0 when the arc is gone.
    pub life_ms: f32,
    /// Bend at each point, in units of the arc's bow.
    pub jitter: [f32; ARC_POINTS],
}

impl ElectricArc {
    fn empty() -> Self {
        Self {
            index: 0,
            reach: 1,
            tip: ArcTip::To,
            born_ms: 0.0,
            life_ms: 0.0,
            jitter: [0.0; ARC_POINTS],
        }
    }
}

/// The inputs that decide how the scene moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricSceneOptions {
    pub bar_count: usize,
    pub intensity: f32,
    pub arcs: bool,
    pub sparks: bool,
    pub loading: bool,
    pub reduced_motion: bool,
    /// Makes the crackle repeatable.
    pub seed: u32,
}

impl ElectricSceneOptions {
    /// Defaults matching audiocn.
    #[must_use]
    pub fn new(bar_count: usize) -> Self {
        Self {
            bar_count,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            loading: false,
            reduced_motion: false,
            seed: 1,
        }
    }
}

/// Jagged filaments, arcs and sparks, apart from any painter.
#[derive(Clone, Debug)]
pub struct ElectricSceneState {
    options: ElectricSceneOptions,
    random: Crackle,
    jitter: Vec<f32>,
    flicker: Vec<f32>,
    arcs: Vec<ElectricArc>,
    sparks: Sparks,
    previous: Vec<f32>,
    last_emit_ms: Vec<f64>,
    last_ms: f64,
    last_jitter_ms: f64,
    primed: bool,
}

impl ElectricSceneState {
    /// A still scene with straight filaments.
    #[must_use]
    pub fn new(mut options: ElectricSceneOptions) -> Self {
        options.intensity = clamp(options.intensity, 0.0, 1.0);
        let bar_count = options.bar_count;
        Self {
            options,
            random: Crackle::new(options.seed as f32),
            jitter: vec![0.0; bar_count * FILAMENT_POINTS],
            flicker: vec![1.0; bar_count],
            arcs: vec![ElectricArc::empty(); MAX_ARCS],
            sparks: Sparks::new(MAX_SPARKS),
            previous: vec![0.0; bar_count],
            last_emit_ms: vec![f64::NEG_INFINITY; bar_count],
            last_ms: 0.0,
            last_jitter_ms: f64::NEG_INFINITY,
            primed: false,
        }
    }

    /// The options this scene was built for.
    #[must_use]
    pub fn options(&self) -> ElectricSceneOptions {
        self.options
    }

    /// True while a particle or bolt is still on screen.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.sparks.live().next().is_some() || self.arcs.iter().any(|arc| arc.life_ms > 0.0)
    }

    fn signed(&mut self) -> f32 {
        self.random.next() * 2.0 - 1.0
    }

    fn reroll_filaments(&mut self, centered: bool) {
        for index in 0..self.options.bar_count {
            let from = if centered { self.signed() * FREE_TIP } else { 0.0 };
            let to = self.signed() * FREE_TIP;
            displace(
                &mut self.jitter,
                index * FILAMENT_POINTS,
                FILAMENT_POINTS,
                (from, to),
                &mut self.random,
                ROUGHNESS,
            );
            self.flicker[index] = 1.0 - FLICKER * self.random.next();
        }
    }

    fn start_arc(&mut self, slot: usize, index: usize, reach: usize, tip: ArcTip, now_ms: f64) {
        let life = ARC_MIN_MS + self.random.next() * (ARC_MAX_MS - ARC_MIN_MS);
        let arc = &mut self.arcs[slot];
        arc.index = index;
        arc.reach = reach;
        arc.tip = tip;
        arc.born_ms = now_ms;
        arc.life_ms = life;
    }

    /// While loading, one arc rides the sweep across the bars.
    fn follow_sweep(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        let Some(peak) = levels
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
        else {
            return;
        };
        let left = levels.get(peak.wrapping_sub(1)).copied().unwrap_or(0.0);
        let right = levels.get(peak + 1).copied().unwrap_or(0.0);
        let index = if left > right {
            peak.saturating_sub(1)
        } else {
            peak
        }
        .min(self.options.bar_count.saturating_sub(2));
        if self.arcs.is_empty() {
            return;
        }
        self.start_arc(
            0,
            index,
            1,
            if centered { ArcTip::From } else { ArcTip::To },
            now_ms,
        );
        self.arcs[0].life_ms = JITTER_MS as f32 * 2.0;
    }

    fn spawn_arcs(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        for _ in 0..ARC_ATTEMPTS {
            let reach = 1 + (self.random.next() * MAX_ARC_REACH as f32).floor() as usize;
            let max_index = self.options.bar_count.saturating_sub(reach);
            let index = (self.random.next() * max_index as f32).floor() as usize;
            let strength = levels
                .get(index)
                .copied()
                .unwrap_or(0.0)
                .min(levels.get(index + reach).copied().unwrap_or(0.0));
            let charged = strength >= ARC_THRESHOLD
                && self.random.next() < ARC_CHANCE * self.options.intensity * strength;
            let free = self.arcs.iter().position(|arc| arc.life_ms <= 0.0);
            let taken = self
                .arcs
                .iter()
                .any(|arc| arc.life_ms > 0.0 && arc.index == index);
            if charged && !taken {
                if let Some(slot) = free {
                    let tip = if centered && self.random.next() < 0.5 {
                        ArcTip::From
                    } else {
                        ArcTip::To
                    };
                    self.start_arc(slot, index, reach, tip, now_ms);
                }
            }
        }
    }

    fn update_arcs(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        if self.options.bar_count < 2 {
            return;
        }
        if self.options.loading {
            self.follow_sweep(now_ms, levels, centered);
        } else {
            self.spawn_arcs(now_ms, levels, centered);
        }
        for arc in &mut self.arcs {
            if arc.life_ms > 0.0 {
                displace(&mut arc.jitter, 0, ARC_POINTS, (0.0, 0.0), &mut self.random, ROUGHNESS);
            }
        }
    }

    fn expire_arcs(&mut self, now_ms: f64) {
        for arc in &mut self.arcs {
            if arc.life_ms > 0.0 && now_ms - arc.born_ms >= f64::from(arc.life_ms) {
                arc.life_ms = 0.0;
            }
        }
    }

    fn launch_spark(&mut self, index: usize, level: f32, layout: &ElectricLayout) {
        let tip = if layout.align == BarAlign::Center && self.random.next() < 0.5 {
            ArcTip::From
        } else {
            ArcTip::To
        };
        let along = if tip == ArcTip::To {
            bar_to(layout, level)
        } else {
            bar_from(layout, level)
        };
        let across = layout.first + index as f32 * layout.pitch;
        let speed = SPARK_SPEED * (0.5 + self.random.next()) * (0.5 + self.options.intensity);
        let along_velocity = tip_direction(layout, tip) * speed;
        let across_velocity = self.signed() * SPARK_SPREAD;
        let life = SPARK_MIN_MS + self.random.next() * (SPARK_MAX_MS - SPARK_MIN_MS);
        let (x, y) = if layout.horizontal {
            (across, along)
        } else {
            (along, across)
        };
        let (vx, vy) = if layout.horizontal {
            (across_velocity, along_velocity)
        } else {
            (along_velocity, across_velocity)
        };
        self.sparks.emit(SparkLaunch {
            x: layout.origin.x + x,
            y: layout.origin.y + y,
            vx,
            vy,
            life_ms: life,
        });
    }

    fn emit_sparks(&mut self, now_ms: f64, levels: &[f32], layout: &ElectricLayout) {
        for (index, &level) in levels.iter().enumerate() {
            let rise = level - self.previous.get(index).copied().unwrap_or(0.0);
            let rested = now_ms - self.last_emit_ms.get(index).copied().unwrap_or(0.0)
                >= SPARK_COOLDOWN_MS;
            if rise >= SPARK_RISE && level >= SPARK_MIN_LEVEL && rested {
                self.last_emit_ms[index] = now_ms;
                let count = 2 + (self.random.next() * 3.0).floor() as usize;
                for _ in 0..count {
                    self.launch_spark(index, level, layout);
                }
            }
        }
    }

    /// Advances the crackle, arcs and sparks to `now_ms`.
    pub fn step(&mut self, now_ms: f64, levels: &[f32], layout: &ElectricLayout) {
        let seconds = if self.last_ms == 0.0 {
            FRAME_SECONDS
        } else {
            ((now_ms - self.last_ms) / 1000.0).clamp(0.0, MAX_STEP_SECONDS as f64) as f32
        };
        self.last_ms = now_ms;
        if self.options.reduced_motion {
            return;
        }
        let centered = layout.align == BarAlign::Center;
        if now_ms - self.last_jitter_ms >= JITTER_MS {
            self.last_jitter_ms = now_ms;
            self.reroll_filaments(centered);
            if self.options.arcs {
                self.update_arcs(now_ms, levels, centered);
            }
        }
        self.expire_arcs(now_ms);
        if self.options.sparks {
            if self.primed {
                self.emit_sparks(now_ms, levels, layout);
            }
            self.sparks.advance(seconds, SPARK_GRAVITY);
        }
        let count = levels.len().min(self.previous.len());
        self.previous[..count].copy_from_slice(&levels[..count]);
        self.primed = true;
    }
}

struct FilamentPass {
    width: f32,
    min_width: f32,
    bend: f32,
    alpha_base: f32,
    alpha_slope: f32,
}

const GLOW_PASS: FilamentPass = FilamentPass {
    width: 1.6,
    min_width: 2.0,
    bend: 0.35,
    alpha_base: 0.1,
    alpha_slope: 0.35,
};

const SHEATH_PASS: FilamentPass = FilamentPass {
    width: 1.0,
    min_width: 1.0,
    bend: 0.35,
    alpha_base: 0.35,
    alpha_slope: 0.35,
};

const CORE_PASS: FilamentPass = FilamentPass {
    width: 0.3,
    min_width: 1.25,
    bend: 1.0,
    alpha_base: 0.8,
    alpha_slope: 0.2,
};

fn filament_points(
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    index: usize,
    level: f32,
    bend_share: f32,
    intensity: f32,
) -> Vec<Pos2> {
    let across = layout.first + index as f32 * layout.pitch;
    let amplitude = jitter_amplitude(layout, intensity, level) * bend_share;
    let from = bar_from(layout, level);
    let to = bar_to(layout, level);
    let stride = stride_for((to - from).abs());
    let offset = index * FILAMENT_POINTS;
    let mut points = Vec::with_capacity(FILAMENT_POINTS / stride + 1);
    for point in (0..FILAMENT_POINTS).step_by(stride) {
        let along = from + (to - from) * point as f32 / (FILAMENT_POINTS - 1) as f32;
        let bend = scene.jitter.get(offset + point).copied().unwrap_or(0.0) * amplitude;
        points.push(plot(layout, across + bend, along));
    }
    points
}

fn stroke_filaments(
    painter: &Painter,
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    levels: &[f32],
    intensity: f32,
    pass: &FilamentPass,
    color: Color32,
) {
    let line_width = (layout.bar_width * pass.width).max(pass.min_width);
    for (index, &level) in levels.iter().enumerate() {
        let flicker = scene.flicker.get(index).copied().unwrap_or(1.0);
        let alpha = clamp(
            (pass.alpha_base + pass.alpha_slope * level) * flicker,
            0.0,
            1.0,
        );
        let points = filament_points(layout, scene, index, level, pass.bend, intensity);
        painter.add(Shape::line(
            points,
            Stroke::new(line_width, with_alpha(color, alpha)),
        ));
    }
}

/// Where a bar's tip is, jitter included, as `(across, along)`.
fn tip_of(
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    index: usize,
    level: f32,
    tip: ArcTip,
    intensity: f32,
) -> (f32, f32) {
    let point = if tip == ArcTip::To { FILAMENT_POINTS - 1 } else { 0 };
    let bend = scene
        .jitter
        .get(index * FILAMENT_POINTS + point)
        .copied()
        .unwrap_or(0.0)
        * jitter_amplitude(layout, intensity, level);
    let along = if tip == ArcTip::To {
        bar_to(layout, level)
    } else {
        bar_from(layout, level)
    };
    (layout.first + index as f32 * layout.pitch + bend, along)
}

fn fill_tips(
    painter: &Painter,
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    levels: &[f32],
    intensity: f32,
    color: Color32,
    radius: f32,
) {
    let tips: &[ArcTip] = if layout.align == BarAlign::Center {
        &[ArcTip::From, ArcTip::To]
    } else {
        &[ArcTip::To]
    };
    for (index, &level) in levels.iter().enumerate() {
        let heat = (level - TIP_LEVEL) / (1.0 - TIP_LEVEL);
        if heat <= 0.0 {
            continue;
        }
        let alpha = clamp(heat, 0.0, 1.0) * scene.flicker.get(index).copied().unwrap_or(1.0);
        for &tip in tips {
            let (across, along) = tip_of(layout, scene, index, level, tip, intensity);
            painter.circle_filled(plot(layout, across, along), radius, with_alpha(color, alpha));
        }
    }
}

fn stroke_arcs(
    painter: &Painter,
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    levels: &[f32],
    intensity: f32,
    now_ms: f64,
    line_width: f32,
    alpha: f32,
    color: Color32,
) {
    for arc in &scene.arcs {
        if arc.life_ms <= 0.0 {
            continue;
        }
        let Some(&from_level) = levels.get(arc.index) else {
            continue;
        };
        let Some(&to_level) = levels.get(arc.index + arc.reach) else {
            continue;
        };
        let (from_across, from_along) =
            tip_of(layout, scene, arc.index, from_level, arc.tip, intensity);
        let (to_across, to_along) =
            tip_of(layout, scene, arc.index + arc.reach, to_level, arc.tip, intensity);
        let across_span = to_across - from_across;
        let along_span = to_along - from_along;
        let length = across_span.hypot(along_span).max(1.0);
        let bow = length * ARC_BOW;
        let age = ((now_ms - arc.born_ms) / f64::from(arc.life_ms)).clamp(0.0, 1.0) as f32;
        let alpha = clamp(alpha * (1.0 - age * age), 0.0, 1.0);
        let mut points = Vec::with_capacity(ARC_POINTS);
        for (point, &bend) in arc.jitter.iter().enumerate() {
            let progress = point as f32 / (ARC_POINTS - 1) as f32;
            let bend = bend * bow;
            points.push(plot(
                layout,
                from_across + across_span * progress - (along_span / length) * bend,
                from_along + along_span * progress + (across_span / length) * bend,
            ));
        }
        painter.add(Shape::line(
            points,
            Stroke::new(line_width, with_alpha(color, alpha)),
        ));
    }
}

/// Draws the soft glow layer: wide filaments, glowing tips and halo arcs.
pub fn paint_glow(
    painter: &Painter,
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    levels: &[f32],
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
) {
    let bar_width = layout.bar_width;
    stroke_filaments(painter, layout, scene, levels, intensity, &GLOW_PASS, colors.glow);
    fill_tips(
        painter,
        layout,
        scene,
        levels,
        intensity,
        colors.glow,
        bar_width * 1.2,
    );
    stroke_arcs(
        painter,
        layout,
        scene,
        levels,
        intensity,
        now_ms,
        bar_width,
        0.7,
        colors.glow,
    );
}

/// Draws the sharp layer: body, hot core, arcs, tips and sparks.
pub fn paint_main(
    painter: &Painter,
    layout: &ElectricLayout,
    scene: &ElectricSceneState,
    levels: &[f32],
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
) {
    let bar_width = layout.bar_width;
    stroke_filaments(painter, layout, scene, levels, intensity, &SHEATH_PASS, colors.body);
    stroke_arcs(
        painter,
        layout,
        scene,
        levels,
        intensity,
        now_ms,
        (bar_width * 0.45).max(1.5),
        0.6,
        colors.body,
    );
    stroke_filaments(painter, layout, scene, levels, intensity, &CORE_PASS, colors.core);
    stroke_arcs(
        painter,
        layout,
        scene,
        levels,
        intensity,
        now_ms,
        1.0,
        1.0,
        colors.core,
    );
    fill_tips(
        painter,
        layout,
        scene,
        levels,
        intensity,
        colors.core,
        (bar_width * 0.35).max(1.0),
    );
    scene.sparks.stroke(
        painter,
        (bar_width * 0.25).max(1.0),
        SPARK_TRAIL_SECONDS,
        colors.core,
    );
}

/// The width of the sweep, kept here so tests can check the Gaussian.
#[must_use]
pub fn sweep_gaussian(index: usize, count: usize, seconds: f32) -> f32 {
    crate::ui::visualizers::bar_levels::sweep_level(index, count, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(align: BarAlign) -> ElectricLayout {
        let mut layout = layout_electric_bars(
            320.0,
            64.0,
            ElectricLayoutOptions {
                align,
                bar_count: 16,
                bar_gap: DEFAULT_BAR_GAP,
                bar_width: DEFAULT_BAR_WIDTH,
                horizontal: true,
            },
        );
        layout.origin = Pos2::new(10.0, 20.0);
        layout
    }

    #[test]
    fn layout_centres_the_narrowest_bars() {
        let layout = layout(BarAlign::Center);
        assert_eq!(layout.pitch, 10.0);
        // 16 bars of 6 plus 15 gaps of 4 fill 156 points, centred in 320.
        assert!((layout.first - 85.0).abs() < 1e-5);
        assert!((layout.base - 32.0).abs() < 1e-6);
        assert!((bar_from(&layout, 0.5) - 19.0).abs() < 1e-5);
        assert!((bar_to(&layout, 0.5) - 45.0).abs() < 1e-5);
    }

    #[test]
    fn bars_narrow_when_the_row_is_too_short() {
        let layout = layout_electric_bars(
            60.0,
            40.0,
            ElectricLayoutOptions {
                align: BarAlign::Start,
                bar_count: 16,
                bar_gap: 4.0,
                bar_width: 6.0,
                horizontal: true,
            },
        );
        assert!(layout.pitch < 10.0);
        assert!(layout.bar_width < 6.0);
        assert!((layout.base - layout.bar_width).abs() < 1e-6);
    }

    #[test]
    fn filaments_crackle_and_arcs_only_between_loud_bars() {
        let mut options = ElectricSceneOptions::new(16);
        options.seed = 9;
        options.intensity = 1.0;
        let mut scene = ElectricSceneState::new(options);
        let layout = layout(BarAlign::Center);
        let levels = vec![0.1; 16];
        scene.step(0.0, &levels, &layout);
        scene.step(100.0, &levels, &layout);
        assert!(
            scene.jitter.iter().any(|&value| value != 0.0),
            "filaments should jitter"
        );
        assert!(
            scene.arcs.iter().all(|arc| arc.life_ms == 0.0),
            "quiet bars throw no arcs"
        );
        let loud = vec![0.9; 16];
        for frame in 0..40 {
            scene.step(140.0 + f64::from(frame) * 50.0, &loud, &layout);
        }
        assert!(
            scene.arcs.iter().any(|arc| arc.life_ms > 0.0),
            "loud neighbours throw arcs"
        );
    }

    #[test]
    fn sparks_fly_on_a_sudden_rise() {
        let mut options = ElectricSceneOptions::new(8);
        options.seed = 4;
        let mut scene = ElectricSceneState::new(options);
        let layout = layout(BarAlign::Center);
        scene.step(0.0, &[0.0; 8], &layout);
        scene.step(50.0, &[0.0; 8], &layout);
        scene.step(100.0, &[0.9; 8], &layout);
        assert!(scene.is_animating());
    }

    #[test]
    fn reduced_motion_keeps_the_filaments_straight() {
        let mut options = ElectricSceneOptions::new(8);
        options.reduced_motion = true;
        let mut scene = ElectricSceneState::new(options);
        let layout = layout(BarAlign::Center);
        scene.step(0.0, &[0.5; 8], &layout);
        scene.step(500.0, &[0.5; 8], &layout);
        assert!(scene.jitter.iter().all(|&value| value == 0.0));
        assert!(!scene.is_animating());
    }

    #[test]
    fn the_sweep_peaks_where_it_should() {
        for index in 0..24 {
            let level = sweep_gaussian(index, 24, 0.0);
            assert!((0.0..=0.55).contains(&level));
        }
    }
}
