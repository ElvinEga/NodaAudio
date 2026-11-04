//! The shared drawing of the electric visualizers: repeatable crackle,
//! midpoint displacement, and a pool of sparks.
//!
//! This is the port of audiocn's `lib/electric.ts`. Both electric widgets
//! draw a sharp line and a blurred copy of it underneath; the line is made of
//! lightning, the lightning comes from this file, and the numbers here are
//! deliberately the same ones the browser runs so a crackle repeats the same
//! way frame after frame.

use crate::db::clamp;
use egui::{Color32, Painter, Pos2, Stroke, Vec2};

/// The period of the counter behind the pseudo-random sequence.
const RANDOM_PERIOD: f64 = 1_000_003.0;

/// A repeatable pseudo-random sequence in `0..1`, the same sequence for the
/// same seed.
///
/// `Math.sin(seed * 12.9898 + counter * 78.233) * 43_758.5453` fractionalised:
/// a hash that costs nothing and never wanders, so a bolt looks like itself
/// twice in a row rather than flickering noise.
#[derive(Clone, Copy, Debug)]
pub struct Crackle {
    seed: f64,
    counter: f64,
}

impl Crackle {
    /// The sequence for a seed.
    #[must_use]
    pub fn new(seed: f32) -> Self {
        Self { seed: f64::from(seed), counter: 0.0 }
    }

    /// The next value, and the counter moves on.
    #[must_use]
    pub fn next(&mut self) -> f32 {
        self.counter = (self.counter + 1.0) % RANDOM_PERIOD;
        let value = (self.seed * 12.9898 + self.counter * 78.233).sin() * 43_758.5453;
        (value - value.floor()) as f32
    }
}

/// Midpoint displacement: fills `count` offsets from `start` - `count - 1` a
/// power of two - between the two end values, big bends first and finer ones
/// on top. Each finer split bends `roughness` times the one before.
///
/// The recursion order is the browser's, so the same seed and the same
/// `roughness` produce the same bolt.
pub fn displace(
    out: &mut [f32],
    start: usize,
    count: usize,
    ends: (f32, f32),
    random: &mut Crackle,
    roughness: f32,
) {
    if count == 0 {
        return;
    }
    let last = count - 1;
    out[start] = ends.0;
    out[start + last] = ends.1;
    let mut amplitude = 1.0f32;
    let mut step = last;
    while step > 1 {
        let half = step / 2;
        let mut index = half;
        while index < last {
            let before = out[start + index - half];
            let after = out[start + index + half];
            out[start + index] =
                (before + after) / 2.0 + (random.next() * 2.0 - 1.0) * amplitude;
            index += step;
        }
        amplitude *= roughness;
        step = half;
    }
}

/// A spark leaving a bolt: where it starts, how fast, and for how long.
#[derive(Clone, Copy, Debug)]
pub struct SparkLaunch {
    pub x: f32,
    pub y: f32,
    /// Pixels per second.
    pub vx: f32,
    pub vy: f32,
    /// Milliseconds.
    pub life_ms: f32,
}

/// A fixed pool of sparks, in canvas pixels. A spark with `life` 0 is gone.
///
/// The pool is fixed and circular: the next spark takes the next slot and
/// replaces the oldest once it is full, so a burst can never grow the frame
/// time.
#[derive(Clone, Debug)]
pub struct Sparks {
    x: Vec<f32>,
    y: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    /// Milliseconds since the spark was emitted.
    age: Vec<f32>,
    /// Milliseconds it lives for; 0 when it is gone.
    life: Vec<f32>,
    /// The slot the next spark takes.
    next: usize,
}

impl Sparks {
    /// A pool of `size` slots, all empty.
    #[must_use]
    pub fn new(size: usize) -> Self {
        let filled = vec![0.0; size];
        Self {
            x: filled.clone(),
            y: filled.clone(),
            vx: filled.clone(),
            vy: filled.clone(),
            age: filled.clone(),
            life: filled,
            next: 0,
        }
    }

    /// How many slots the pool has.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.life.len()
    }

    /// Take the next slot for a spark, replacing the oldest when full.
    pub fn emit(&mut self, spark: SparkLaunch) {
        let slot = self.next;
        self.next = (slot + 1) % self.capacity().max(1);
        self.x[slot] = spark.x;
        self.y[slot] = spark.y;
        self.vx[slot] = spark.vx;
        self.vy[slot] = spark.vy;
        self.age[slot] = 0.0;
        self.life[slot] = spark.life_ms;
    }

    /// Move every live spark, pulled down by `gravity` in pixels per second
    /// squared, and retire the ones that have lived out their `life`.
    pub fn advance(&mut self, seconds: f32, gravity: f32) {
        for slot in 0..self.capacity() {
            if self.life[slot] <= 0.0 {
                continue;
            }
            let age = self.age[slot] + seconds * 1000.0;
            self.age[slot] = age;
            if age >= self.life[slot] {
                self.life[slot] = 0.0;
            } else {
                let vy = self.vy[slot] + gravity * seconds;
                self.vy[slot] = vy;
                self.x[slot] += self.vx[slot] * seconds;
                self.y[slot] += vy * seconds;
            }
        }
    }

    /// Drop every spark, for a widget that is idle or has been reset.
    pub fn clear(&mut self) {
        for (life, age) in self.life.iter_mut().zip(&mut self.age) {
            *life = 0.0;
            *age = 0.0;
        }
    }

    /// The live sparks, as `(position, velocity)`.
    #[must_use]
    pub fn live(&self) -> impl Iterator<Item = (Pos2, Vec2)> + '_ {
        (0..self.capacity())
            .filter(move |&slot| self.life[slot] > 0.0)
            .map(move |slot| {
                (Pos2::new(self.x[slot], self.y[slot]), Vec2::new(self.vx[slot], self.vy[slot]))
            })
    }

    /// Draw each live spark as a short streak that fades with age: the body
    /// at full alpha, the streak behind it at a third.
    pub fn stroke(&self, painter: &Painter, line_width: f32, trail_seconds: f32, color: Color32) {
        let stroke = Stroke::new(line_width, color);
        for slot in 0..self.capacity() {
            if self.life[slot] <= 0.0 {
                continue;
            }
            let at = Pos2::new(self.x[slot], self.y[slot]);
            let from = at - Vec2::new(self.vx[slot], self.vy[slot]) * trail_seconds;
            let alpha = clamp(1.0 - self.age[slot] / self.life[slot], 0.0, 1.0);
            painter.line_segment([from, at], Stroke::new(line_width, with_alpha(color, alpha * 0.34)));
            painter.line_segment([at, at], stroke);
        }
    }
}

/// The three colours an electric widget draws with.
///
/// In the browser these are read off the two canvases' CSS; here a theme
/// resolves them, and the glow canvas's blur becomes `glow_spread`, the width
/// the halo is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricColors {
    /// The stroke on the sharp canvas: `--electric`, or the text colour.
    pub body: Color32,
    /// The highlight down the middle: `--electric-core`, or the body mixed
    /// toward white by `--electric-heat`.
    pub core: Color32,
    /// The blurred canvas underneath: `--electric-glow`, or the body.
    pub glow: Color32,
}

impl ElectricColors {
    /// The stock hardware look: the ink as the body, a heat-whitened core, and
    /// the body itself as the halo.
    #[must_use]
    pub fn new(body: Color32, heat: f32) -> Self {
        Self { body, core: mix_white(body, heat), glow: body }
    }
}

/// `color-mix(in oklch, currentColor, white X%)`.
#[must_use]
pub fn mix_white(color: Color32, amount: f32) -> Color32 {
    let at = clamp(amount, 0.0, 1.0);
    Color32::from_rgb(
        (color.r() as f32 + (255.0 - color.r() as f32) * at) as u8,
        (color.g() as f32 + (255.0 - color.g() as f32) * at) as u8,
        (color.b() as f32 + (255.0 - color.b() as f32) * at) as u8,
    )
}

/// The same colour at `alpha`, for the halo and the fading streaks.
#[must_use]
pub fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (clamp(alpha, 0.0, 1.0) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crackle_repeats_itself_for_a_seed() {
        let mut first_gen = Crackle::new(7.0);
        let first: Vec<f32> = (0..8).map(|_| first_gen.next()).collect();
        let mut again = Crackle::new(7.0);
        let second: Vec<f32> = (0..8).map(|_| again.next()).collect();
        assert_eq!(first, second, "a bolt looks like itself twice in a row");
        assert!(first.iter().all(|&v| (0.0..1.0).contains(&v)), "{first:?}");
        let mut other = Crackle::new(8.0);
        assert_ne!((0..8).map(|_| other.next()).collect::<Vec<f32>>(), first);
    }

    #[test]
    fn displacement_keeps_both_ends_and_bends_the_middle() {
        let mut out = [0.0f32; 9];
        let mut random = Crackle::new(3.0);
        displace(&mut out, 0, 9, (-1.0, 1.0), &mut random, 0.5);
        assert_eq!(out[0], -1.0);
        assert_eq!(out[8], 1.0);
        assert_ne!(out[4], 0.0, "midpoint should be displaced");
        assert!(out.iter().all(|&v| v.is_finite()));
    }

    #[test]
    fn displacement_bends_coarse_before_fine() {
        let mut out = [0.0f32; 5];
        displace(&mut out, 0, 5, (0.0, 0.0), &mut Crackle::new(1.0), 0.5);
        // The first split is the whole amplitude, the next is half of it.
        assert!(out[2].abs() > out[1].abs() || out[1].abs() < 0.6, "{out:?}");
    }

    #[test]
    fn sparks_fade_out_and_the_pool_wraps() {
        let mut sparks = Sparks::new(2);
        sparks.emit(SparkLaunch { x: 0.0, y: 0.0, vx: 100.0, vy: 0.0, life_ms: 100.0 });
        sparks.emit(SparkLaunch { x: 4.0, y: 4.0, vx: 0.0, vy: 50.0, life_ms: 400.0 });
        sparks.advance(0.05, 900.0);
        let live: Vec<(Pos2, Vec2)> = sparks.live().collect();
        assert_eq!(live.len(), 2, "both still alive after 50ms");
        assert_eq!(live[0].0.x, 5.0, "the first has drifted by its velocity");
        sparks.advance(0.1, 900.0);
        assert_eq!(sparks.live().count(), 1, "the short-lived one retired");
        sparks.emit(SparkLaunch { x: 9.0, y: 9.0, vx: 0.0, vy: 0.0, life_ms: 100.0 });
        sparks.emit(SparkLaunch { x: 8.0, y: 8.0, vx: 0.0, vy: 0.0, life_ms: 100.0 });
        assert_eq!(sparks.live().count(), 2, "a full pool replaces the oldest");
        sparks.clear();
        assert_eq!(sparks.live().count(), 0);
    }

    #[test]
    fn the_core_is_the_body_mixed_toward_white() {
        let body = Color32::from_rgb(100, 100, 100);
        assert_eq!(mix_white(body, 0.0), body);
        assert_eq!(mix_white(body, 1.0), Color32::WHITE);
        let half = mix_white(body, 0.5);
        assert!(half.r() > body.r() && half.r() < 255, "{half:?}");
    }
}
