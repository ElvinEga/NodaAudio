//! An electric lightning waveform, ported from audiocn's
//! `electric-waveform.tsx`.
//!
//! The line's shape comes from [`WaveLineState`](super::wave_line::WaveLineState):
//! a drifting band-driven wave or a triggered oscilloscope trace. On top of
//! it, the trace adds crackle built from shared-end midpoint-displacement
//! chunks, forks of lightning off the peaks, sparks thrown on sudden rises,
//! and a tall glow made of two ribbons and a wide stroke.

use std::time::Duration;

use egui::{Color32, Pos2, Response, Sense, Ui, Vec2, Widget, pos2};

use crate::db::clamp;
use crate::electric::{
    Crackle, ElectricColors, SparkLaunch, Sparks, displace, mix_white, with_alpha,
};
use crate::ui::theme::{Size, Theme};
use crate::ui::visualizers::electric_bar_visualizer::pseudo_random;
use crate::ui::visualizers::line_paint::{faded_line, soft_ribbon};
use crate::ui::visualizers::smooth_waveform::SmoothWaveformMode;
use crate::ui::visualizers::wave_line::{
    WAVE_LINE_MAX_POINTS, WaveLineMode, WaveLineOptions, WaveLineState,
};

/// One point every this many pixels along the line.
const POINT_SPACING_PX: f32 = 4.0;
/// Crackle is built in short chunks of 2^2 segments that share their ends.
const CHUNK: usize = 4;
const MIN_POINTS: usize = CHUNK * 8 + 1;
const MAX_POINTS: usize = WAVE_LINE_MAX_POINTS;
/// How much each finer split of the crackle bends compared with the one before.
const DEFAULT_ROUGHNESS: f32 = 0.8;
/// Each jagged shape holds at least this long, so the crackle never strobes.
const JITTER_MS: f64 = 42.0;
/// Brightness flicker, at most 12%.
const FLICKER: f32 = 0.12;
/// Crackle at rest, in pixels at full intensity.
const HUM_PX: f32 = 1.5;
/// Extra crackle at full level, in pixels at full intensity.
const CRACKLE_PX: f32 = 6.0;

const MAX_BRANCHES: usize = 3;
const BRANCH_POINTS: usize = (1 << 3) + 1;
/// A point must be this far from the middle to throw a fork.
const BRANCH_THRESHOLD: f32 = 0.3;
const BRANCH_CHANCE: f32 = 0.6;
const BRANCH_CANDIDATES: usize = 3;
const BRANCH_MIN_PX: f32 = 10.0;
const BRANCH_MAX_PX: f32 = 30.0;
const BRANCH_MIN_MS: f32 = 60.0;
const BRANCH_MAX_MS: f32 = 140.0;
/// How far a fork bends, relative to its length.
const BRANCH_BOW: f32 = 0.35;

const MAX_SPARKS: usize = 48;
/// A rise in loudness this big in one frame throws sparks.
const SPARK_RISE: f32 = 0.1;
const SPARK_MIN_LEVEL: f32 = 0.3;
const SPARK_COOLDOWN_MS: f64 = 90.0;
/// Pixels per second squared.
const SPARK_GRAVITY: f32 = 520.0;
/// Pixels per second.
const SPARK_SPEED: f32 = 170.0;
/// Pixels per second, sideways.
const SPARK_SPREAD: f32 = 90.0;
const SPARK_MIN_MS: f32 = 260.0;
const SPARK_MAX_MS: f32 = 560.0;
const SPARK_TRAIL_SECONDS: f32 = 0.03;

const FRAME_MS: f64 = 16.67;
const MAX_STEP_SECONDS: f32 = 0.05;
/// Width of the faded ends, in pixels.
const FADE_PX: f32 = 24.0;
/// Stand-in for audiocn's `0.5rem` CSS blur on the glow canvas.
const GLOW_BLUR_PX: f32 = 8.0;
const DEFAULT_LINE_WIDTH: f32 = 3.0;
const DEFAULT_INTENSITY: f32 = 0.6;

/// Fractal midpoint displacement for lightning crackle.
pub fn fractal_displace(
    points: &mut Vec<Pos2>,
    p1: Pos2,
    p2: Pos2,
    disp: f32,
    roughness: f32,
    depth: usize,
    seed: u32,
    step: &mut u32,
) {
    if depth == 0 {
        points.push(p2);
        return;
    }

    *step += 1;
    let rnd = pseudo_random(seed, *step) * 2.0 - 1.0;
    let mid_x = (p1.x + p2.x) * 0.5;
    let mid_y = (p1.y + p2.y) * 0.5 + rnd * disp;
    let mid = pos2(mid_x, mid_y);

    let next_disp = disp * roughness;
    fractal_displace(points, p1, mid, next_disp, roughness, depth - 1, seed, step);
    fractal_displace(points, mid, p2, next_disp, roughness, depth - 1, seed, step);
}

/// A fork of lightning off the line.
#[derive(Clone, Copy, Debug)]
struct ElectricBranch {
    /// The point on the line it leaves from.
    point: usize,
    /// 1 up, −1 down: away from the middle.
    direction: f32,
    /// Pixels.
    length: f32,
    /// Sideways drift of the far end, relative to the length.
    lean: f32,
    born_ms: f64,
    /// 0 when the fork is gone.
    life_ms: f32,
    /// Bend at each point, in units of the fork's bow.
    jitter: [f32; BRANCH_POINTS],
}

impl ElectricBranch {
    fn empty() -> Self {
        Self {
            point: 0,
            direction: 1.0,
            length: 0.0,
            lean: 0.0,
            born_ms: 0.0,
            life_ms: 0.0,
            jitter: [0.0; BRANCH_POINTS],
        }
    }
}

/// The geometry a trace is drawn at, in screen points.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TraceGeometry {
    origin: Pos2,
    width: f32,
    height: f32,
    line_width: f32,
}

impl TraceGeometry {
    fn middle(self) -> f32 {
        self.origin.y + self.height * 0.5
    }

    /// Pixels from the middle to a height of 1, leaving room for the line.
    fn reach(self) -> f32 {
        (self.height * 0.5 - (self.line_width * 2.0).max(6.0)).max(0.0)
    }

    fn x_of(self, count: usize, point: usize) -> f32 {
        self.origin.x + point as f32 / (count - 1) as f32 * self.width
    }
}

/// The inputs that decide how the trace moves.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TraceOptions {
    mode: WaveLineMode,
    loading: bool,
    intensity: f32,
    arcs: bool,
    sparks: bool,
    sensitivity: f32,
    roughness: f32,
    reduced_motion: bool,
    seed: u32,
}

/// Points for a width: one every few pixels, in whole crackle chunks.
fn point_count_for(width: f32) -> usize {
    (((width / POINT_SPACING_PX / CHUNK as f32).round() * CHUNK as f32 + 1.0) as usize)
        .clamp(MIN_POINTS, MAX_POINTS)
}

#[derive(Clone, Debug)]
struct ElectricTraceState {
    options: TraceOptions,
    line: WaveLineState,
    random: Crackle,
    crackle: Vec<f32>,
    flicker: f32,
    branches: [ElectricBranch; MAX_BRANCHES],
    sparks: Sparks,
    last_ms: f64,
    last_jitter_ms: f64,
    last_spark_ms: f64,
    last_step_seconds: f32,
    previous_peak: f32,
    primed: bool,
}

impl ElectricTraceState {
    fn new(options: TraceOptions) -> Self {
        let mut line_options = WaveLineOptions::new(options.mode);
        line_options.loading = options.loading;
        line_options.sensitivity = options.sensitivity;
        line_options.reduced_motion = options.reduced_motion;
        Self {
            options,
            line: WaveLineState::new(line_options),
            random: Crackle::new(options.seed as f32),
            crackle: vec![0.0; MAX_POINTS],
            flicker: 1.0,
            branches: [ElectricBranch::empty(); MAX_BRANCHES],
            sparks: Sparks::new(MAX_SPARKS),
            last_ms: 0.0,
            last_jitter_ms: f64::NEG_INFINITY,
            last_spark_ms: f64::NEG_INFINITY,
            last_step_seconds: 0.0,
            previous_peak: 0.0,
            primed: false,
        }
    }

    fn options(&self) -> TraceOptions {
        self.options
    }

    fn count(&self) -> usize {
        self.line.count()
    }

    fn signed(&mut self) -> f32 {
        self.random.next() * 2.0 - 1.0
    }

    fn reroll_crackle(&mut self) {
        let count = self.count();
        let roughness = self.options.roughness;
        let first = self.signed() * 0.5;
        if let Some(slot) = self.crackle.first_mut() {
            *slot = first;
        }
        let mut start = 0;
        while start + CHUNK < count {
            let from = self.crackle[start];
            let to = self.signed() * 0.5;
            displace(
                &mut self.crackle,
                start,
                CHUNK + 1,
                (from, to),
                &mut self.random,
                roughness,
            );
            start += CHUNK;
        }
        self.flicker = 1.0 - FLICKER * self.random.next();
    }

    /// Picks the highest of a few random points, so forks favour the peaks.
    fn pick_peak(&mut self) -> usize {
        let count = self.count();
        let mut best = (self.random.next() * count as f32).floor() as usize;
        best = best.min(count.saturating_sub(1));
        for _ in 1..BRANCH_CANDIDATES {
            let point = ((self.random.next() * count as f32).floor() as usize)
                .min(count.saturating_sub(1));
            if self.line.heights()[point].abs() > self.line.heights()[best].abs() {
                best = point;
            }
        }
        best
    }

    fn update_branches(&mut self, now_ms: f64) {
        let point = self.pick_peak();
        let height = self.line.heights().get(point).copied().unwrap_or(0.0);
        let charged = height.abs() >= BRANCH_THRESHOLD
            && self.random.next() < BRANCH_CHANCE * self.options.intensity * height.abs();
        let free = self.branches.iter().position(|branch| branch.life_ms <= 0.0);
        if charged {
            if let Some(slot) = free {
                let length = (BRANCH_MIN_PX
                    + self.random.next() * (BRANCH_MAX_PX - BRANCH_MIN_PX))
                    * (0.6 + 0.8 * self.options.intensity);
                let lean = self.signed() * 0.6;
                let life = BRANCH_MIN_MS
                    + self.random.next() * (BRANCH_MAX_MS - BRANCH_MIN_MS);
                let branch = &mut self.branches[slot];
                branch.point = point;
                branch.direction = if height >= 0.0 { 1.0 } else { -1.0 };
                branch.length = length;
                branch.lean = lean;
                branch.born_ms = now_ms;
                branch.life_ms = life;
            }
        }
        let roughness = self.options.roughness;
        let random = &mut self.random;
        for branch in &mut self.branches {
            if branch.life_ms > 0.0 {
                let end = (random.next() * 2.0 - 1.0) * 0.5;
                displace(
                    &mut branch.jitter,
                    0,
                    BRANCH_POINTS,
                    (0.0, end),
                    random,
                    roughness,
                );
            }
        }
    }

    fn expire_branches(&mut self, now_ms: f64) {
        for branch in &mut self.branches {
            if branch.life_ms > 0.0
                && now_ms - branch.born_ms >= f64::from(branch.life_ms)
            {
                branch.life_ms = 0.0;
            }
        }
    }

    /// Crackle in pixels at a point: louder lines and higher peaks crackle more.
    fn crackle_at(&self, intensity: f32, point: usize) -> f32 {
        intensity
            * (HUM_PX
                + CRACKLE_PX
                    * (0.4 * self.line.loudness()
                        + 0.6 * self.line.heights().get(point).copied().unwrap_or(0.0).abs()))
    }

    /// Where a point of the line is drawn, in screen points.
    fn y_of(
        &self,
        geometry: TraceGeometry,
        intensity: f32,
        point: usize,
        scale: f32,
        crackle_share: f32,
    ) -> f32 {
        geometry.middle()
            - self.line.heights().get(point).copied().unwrap_or(0.0)
                * scale
                * geometry.reach()
            - self.crackle.get(point).copied().unwrap_or(0.0)
                * self.crackle_at(intensity, point)
                * crackle_share
    }

    fn throw_sparks(&mut self, now_ms: f64, geometry: TraceGeometry) {
        let rise = self.line.peak() - self.previous_peak;
        let rested = now_ms - self.last_spark_ms >= SPARK_COOLDOWN_MS;
        if rise < SPARK_RISE || self.line.peak() < SPARK_MIN_LEVEL || !rested {
            return;
        }
        self.last_spark_ms = now_ms;
        let count = self.count();
        let mut peak = 0;
        for point in 1..count {
            if self.line.heights()[point].abs() > self.line.heights()[peak].abs() {
                peak = point;
            }
        }
        let upward = if self.line.heights()[peak] >= 0.0 { -1.0 } else { 1.0 };
        let intensity = self.options.intensity;
        let x = geometry.x_of(count, peak);
        let y = self.y_of(geometry, intensity, peak, 1.0, 1.0);
        let count = 3 + (self.random.next() * 3.0).floor() as usize;
        for _ in 0..count {
            let life = SPARK_MIN_MS + self.random.next() * (SPARK_MAX_MS - SPARK_MIN_MS);
            let vx = self.signed() * SPARK_SPREAD;
            let vy = upward
                * SPARK_SPEED
                * (0.5 + self.random.next())
                * (0.5 + intensity);
            self.sparks.emit(SparkLaunch {
                x,
                y,
                vx,
                vy,
                life_ms: life,
            });
        }
    }

    fn animate(&mut self, now_ms: f64, geometry: TraceGeometry) {
        if now_ms - self.last_jitter_ms >= JITTER_MS {
            self.last_jitter_ms = now_ms;
            self.reroll_crackle();
            if self.options.arcs {
                self.update_branches(now_ms);
            }
        }
        self.expire_branches(now_ms);
        if self.options.sparks {
            if self.primed {
                self.throw_sparks(now_ms, geometry);
            }
            self.sparks.advance(self.last_step_seconds, SPARK_GRAVITY);
        }
    }

    fn step(&mut self, now_ms: f64, input: Option<&[f32]>, geometry: TraceGeometry) -> bool {
        let elapsed_ms = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            (now_ms - self.last_ms).max(0.0)
        };
        self.last_ms = now_ms;
        self.last_step_seconds =
            clamp((elapsed_ms / 1000.0) as f32, 0.0, MAX_STEP_SECONDS);
        let active = self
            .line
            .step(now_ms, input, point_count_for(geometry.width));
        if !self.options.reduced_motion {
            self.animate(now_ms, geometry);
        }
        self.previous_peak = self.line.peak();
        self.primed = true;
        active
    }
}

fn trace_line(
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    intensity: f32,
    scale: f32,
    crackle_share: f32,
) -> Vec<Pos2> {
    let count = trace.count();
    (0..count)
        .map(|point| {
            pos2(
                geometry.x_of(count, point),
                trace.y_of(geometry, intensity, point, scale, crackle_share),
            )
        })
        .collect()
}

fn trace_ribbon(
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    intensity: f32,
    inner: f32,
    outer: f32,
) -> (Vec<Pos2>, Vec<Pos2>) {
    let count = trace.count();
    let outer_points: Vec<Pos2> = (0..count)
        .map(|point| {
            pos2(
                geometry.x_of(count, point),
                trace.y_of(geometry, intensity, point, outer, 0.35),
            )
        })
        .collect();
    let inner_points: Vec<Pos2> = (0..count)
        .map(|point| {
            pos2(
                geometry.x_of(count, point),
                trace.y_of(geometry, intensity, point, inner, 0.35),
            )
        })
        .collect();
    (outer_points, inner_points)
}

fn trace_branch(
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    intensity: f32,
    branch: &ElectricBranch,
) -> Vec<Pos2> {
    let count = trace.count();
    let point = branch.point.min(count.saturating_sub(1));
    let x = geometry.x_of(count, point);
    let y = trace.y_of(geometry, intensity, point, 1.0, 1.0);
    let dx = branch.lean * branch.length;
    let dy = -branch.direction * branch.length;
    let length = dx.hypot(dy).max(1.0);
    let bow = length * BRANCH_BOW;
    (0..BRANCH_POINTS)
        .map(|index| {
            let progress = index as f32 / (BRANCH_POINTS - 1) as f32;
            let bend = branch.jitter[index] * bow;
            pos2(
                x + dx * progress - (dy / length) * bend,
                y + dy * progress + (dx / length) * bend,
            )
        })
        .collect()
}

fn stroke_branches(
    painter: &egui::Painter,
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    intensity: f32,
    now_ms: f64,
    line_width: f32,
    alpha: f32,
    color: Color32,
) {
    for branch in &trace.branches {
        if branch.life_ms <= 0.0 {
            continue;
        }
        let age = ((now_ms - branch.born_ms) / f64::from(branch.life_ms)).clamp(0.0, 1.0)
            as f32;
        let alpha = clamp(alpha * (1.0 - age * age), 0.0, 1.0);
        painter.add(egui::Shape::line(
            trace_branch(trace, geometry, intensity, branch),
            egui::Stroke::new(line_width, with_alpha(color, alpha)),
        ));
    }
}

/// Draws the tall, soft glow: ribbons echoing the wave, a wide stroke and forks.
fn paint_glow(
    painter: &egui::Painter,
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
    fade: f32,
) {
    let strength = (0.4 + 0.6 * trace.line.loudness()) * trace.flicker;
    let (outer, inner) = trace_ribbon(trace, geometry, intensity, 0.2, 2.6);
    soft_ribbon(painter, &outer, &inner, colors.glow, 0.22 * strength, fade, GLOW_BLUR_PX);
    let (outer, inner) = trace_ribbon(trace, geometry, intensity, 0.6, 1.6);
    soft_ribbon(painter, &outer, &inner, colors.glow, 0.4 * strength, fade, GLOW_BLUR_PX);
    let wide = trace_line(trace, geometry, intensity, 1.0, 0.6);
    faded_line(
        painter,
        &wide,
        geometry.line_width * 4.0,
        colors.glow,
        fade,
        0.6 * strength,
    );
    stroke_branches(
        painter,
        trace,
        geometry,
        intensity,
        now_ms,
        geometry.line_width * 1.5,
        0.7,
        colors.glow,
    );
}

/// Draws the sharp layer: body, hot core, forks and sparks.
fn paint_main(
    painter: &egui::Painter,
    trace: &ElectricTraceState,
    geometry: TraceGeometry,
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
    fade: f32,
) {
    let line = trace_line(trace, geometry, intensity, 1.0, 0.6);
    faded_line(
        painter,
        &line,
        geometry.line_width,
        colors.body,
        fade,
        0.9 * trace.flicker,
    );
    stroke_branches(
        painter,
        trace,
        geometry,
        intensity,
        now_ms,
        (geometry.line_width * 0.5).max(1.5),
        0.7,
        colors.body,
    );
    let core = trace_line(trace, geometry, intensity, 1.0, 1.0);
    faded_line(
        painter,
        &core,
        (geometry.line_width * 0.5).max(1.5),
        colors.core,
        fade,
        trace.flicker,
    );
    stroke_branches(
        painter,
        trace,
        geometry,
        intensity,
        now_ms,
        1.0,
        1.0,
        colors.core,
    );
    trace.sparks.stroke(
        painter,
        (geometry.line_width * 0.4).max(1.0),
        SPARK_TRAIL_SECONDS,
        colors.core,
    );
}

/// An electric lightning waveform visualizer.
///
/// ```ignore
/// let samples = [0.0, 0.4, 0.9, -0.2, -0.7, 0.0];
/// ui.add(ElectricWaveform::new(&samples));
/// ```
#[must_use = "an ElectricWaveform draws nothing until it is added to a Ui"]
pub struct ElectricWaveform<'a> {
    data: &'a [f32],
    mode: SmoothWaveformMode,
    loading: bool,
    intensity: f32,
    arcs: bool,
    sparks: bool,
    sensitivity: f32,
    roughness: f32,
    line_width: f32,
    fade_edges: bool,
    seed: Option<u32>,
    reduced_motion: bool,
    time: Option<f32>,
    color: Option<Color32>,
    glow_color: Option<Color32>,
    width: Option<f32>,
    height: Option<f32>,
    size: Option<Size>,
}

impl<'a> ElectricWaveform<'a> {
    /// Create a new electric waveform widget from bands or audio samples.
    pub fn new(data: &'a [f32]) -> Self {
        Self {
            data,
            mode: SmoothWaveformMode::Wave,
            loading: false,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            sensitivity: 1.0,
            roughness: DEFAULT_ROUGHNESS,
            line_width: DEFAULT_LINE_WIDTH,
            fade_edges: true,
            seed: None,
            reduced_motion: false,
            time: None,
            color: None,
            glow_color: None,
            width: None,
            height: None,
            size: None,
        }
    }

    /// Mode (`Wave` or `Scope`).
    pub fn mode(mut self, mode: SmoothWaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Run a pulse along the line, for connecting or thinking states.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// How jagged and restless the line is, 0..=1. Default 0.6.
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Forks of lightning branch off the peaks. Default true.
    pub fn arcs(mut self, arcs: bool) -> Self {
        self.arcs = arcs;
        self
    }

    /// Sparks fly off the peaks on sudden rises. Default true.
    pub fn sparks(mut self, sparks: bool) -> Self {
        self.sparks = sparks;
        self
    }

    /// Visual gain. Default 1.
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity.max(0.0);
        self
    }

    /// How much each finer split of the crackle bends. Default 0.8.
    pub fn roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.1, 0.9);
        self
    }

    /// Line width in points. Default 3.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Fade the left and right ends. Default true.
    pub fn fade_edges(mut self, fade: bool) -> Self {
        self.fade_edges = fade;
        self
    }

    /// Seed the crackle. Each widget gets its own by default.
    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = Some(seed);
        self
    }

    /// A still, smooth line, for reduced-motion settings.
    pub fn reduced_motion(mut self, reduced: bool) -> Self {
        self.reduced_motion = reduced;
        self
    }

    /// Animation clock in seconds.
    pub fn time(mut self, time: f32) -> Self {
        self.time = Some(time);
        self
    }

    /// Primary plasma color.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Glow halo color.
    pub fn glow_color(mut self, color: Color32) -> Self {
        self.glow_color = Some(color);
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

    /// Size ladder position.
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    /// Desired dimensions.
    pub fn desired_size(&self, _theme: &Theme, size: Size) -> Vec2 {
        let (default_w, default_h) = match size {
            Size::Small => (160.0, 48.0),
            Size::Medium => (240.0, 68.0),
            Size::Large => (320.0, 88.0),
        };
        Vec2::new(
            self.width.unwrap_or(default_w),
            self.height.unwrap_or(default_h),
        )
    }
}

impl Widget for ElectricWaveform<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = Theme::from_ctx(ui.ctx());
        let size = self.size.unwrap_or(theme.size);
        let desired = self.desired_size(&theme, size);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let now_ms = self
            .time
            .map_or_else(|| ui.input(|i| i.time), f64::from)
            * 1000.0;
        let geometry = TraceGeometry {
            origin: rect.min,
            width: rect.width(),
            height: rect.height(),
            line_width: self.line_width,
        };
        let seed = self.seed.unwrap_or_else(|| {
            let value = response.id.value();
            (value ^ (value >> 32)) as u32
        });
        let options = TraceOptions {
            mode: self.mode,
            loading: self.loading,
            intensity: self.intensity,
            arcs: self.arcs,
            sparks: self.sparks,
            sensitivity: self.sensitivity,
            roughness: self.roughness,
            reduced_motion: self.reduced_motion,
            seed,
        };

        let mut trace = ui
            .ctx()
            .data_mut(|data| data.get_temp::<ElectricTraceState>(response.id.with("electric_trace")))
            .filter(|trace| trace.options() == options)
            .unwrap_or_else(|| ElectricTraceState::new(options));
        trace.step(now_ms, Some(self.data), geometry);

        let body = self.color.unwrap_or(theme.palette.primary);
        let glow = self.glow_color.unwrap_or(body);
        let colors = ElectricColors {
            body,
            core: mix_white(body, 0.72),
            glow,
        };
        let intensity = if self.reduced_motion { 0.0 } else { self.intensity };
        let fade = if self.fade_edges {
            FADE_PX.min(rect.width() * 0.5)
        } else {
            0.0
        };
        let painter = ui.painter();
        paint_glow(
            painter, &trace, geometry, colors, intensity, now_ms, fade,
        );
        paint_main(
            painter, &trace, geometry, colors, intensity, now_ms, fade,
        );

        if !self.reduced_motion {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        ui.ctx()
            .data_mut(|data| data.insert_temp(response.id.with("electric_trace"), trace));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractal_displace_subdivides() {
        let mut pts = vec![pos2(0.0, 0.0)];
        let mut step = 0;
        fractal_displace(
            &mut pts,
            pos2(0.0, 0.0),
            pos2(100.0, 0.0),
            10.0,
            0.5,
            3,
            42,
            &mut step,
        );
        assert_eq!(pts.len(), 9); // 1 + 2^3 = 9
    }

    #[test]
    fn trace_points_snap_to_crackle_chunks() {
        assert_eq!(point_count_for(16.0), MIN_POINTS);
        assert_eq!((point_count_for(400.0) - 1) % CHUNK, 0);
        assert!(point_count_for(10_000.0) <= WAVE_LINE_MAX_POINTS);
    }

    #[test]
    fn crackle_rerolls_in_whole_chunks() {
        let options = TraceOptions {
            mode: WaveLineMode::Wave,
            loading: false,
            intensity: 1.0,
            arcs: true,
            sparks: true,
            sensitivity: 1.0,
            roughness: DEFAULT_ROUGHNESS,
            reduced_motion: false,
            seed: 5,
        };
        let mut trace = ElectricTraceState::new(options);
        let geometry = TraceGeometry {
            origin: Pos2::ZERO,
            width: 300.0,
            height: 80.0,
            line_width: 3.0,
        };
        trace.step(0.0, Some(&[0.9, 0.7, 0.4, 0.2]), geometry);
        assert!(trace.crackle.iter().any(|&value| value != 0.0));
        assert!(trace.flicker >= 1.0 - FLICKER && trace.flicker <= 1.0);
    }

    #[test]
    fn electric_waveform_renders() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let samples = [0.0, 0.5, -0.4, 0.2, 0.0];
            let resp = ui.add(ElectricWaveform::new(&samples));
            assert!(resp.rect.width() > 0.0);
            assert!(resp.rect.height() > 0.0);
        });
    }
}
