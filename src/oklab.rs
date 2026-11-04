//! OKLab colour maths, so a mixed token means the same thing here as in CSS.
//!
//! audiocn derives most of its LCD palette with `color-mix(in oklab, …)` and
//! `oklch(from … )`. Mixing those in sRGB gives the wrong grey, so the mixes
//! happen in the space the browser uses.

use egui::Color32;

/// Linear-light sRGB, the first step on the way to OKLab.
fn to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Gamma-encoded sRGB, the last step out of OKLab.
fn to_gamma(channel: f32) -> f32 {
    if channel <= 0.0031308 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// The L, a, b of a colour in OKLab.
#[must_use]
pub fn to_oklab(color: Color32) -> [f32; 3] {
    let [r, g, b] = [
        to_linear(f32::from(color.r()) / 255.0),
        to_linear(f32::from(color.g()) / 255.0),
        to_linear(f32::from(color.b()) / 255.0),
    ];
    // OKLab goes through a long-cone response space, not XYZ directly.
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_642_5 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_702_5 * b;
    let [l, m, s] = [l, m, s].map(|c| if c > 0.0 { c.cbrt() } else { -(-c).cbrt() });
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_702_6 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// The colour an OKLab triple sorts to, clipped into the sRGB gamut.
#[must_use]
pub fn from_oklab([l, a, b]: [f32; 3]) -> Color32 {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (l_, m_, s_) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = to_clamped(4.076_741_647_0 * l_ - 3.307_711_591_0 * m_ + 0.230_969_929_2 * s_);
    let g = to_clamped(-1.268_438_008_6 * l_ + 2.609_757_401_0 * m_ - 0.341_319_396_5 * s_);
    let b = to_clamped(-0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_);
    Color32::from_rgb(to_byte(r), to_byte(g), to_byte(b))
}

fn to_clamped(linear: f32) -> f32 {
    to_gamma(clamp01(linear))
}

fn to_byte(gamma: f32) -> u8 {
    (gamma.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// `color-mix(in oklab, color, black (1 - keep))` — the way audiocn dims an
/// ink to get its lit pixels, keeping `keep` of the lightness and chroma and
/// every bit of the hue.
#[must_use]
pub fn dim(color: Color32, keep: f32) -> Color32 {
    let [l, a, b] = to_oklab(color);
    from_oklab([l * keep, a * keep, b * keep])
}

/// A pale, low-chroma version of a colour: `oklch(from c 0.93 calc(c*0.35) h)`,
/// which is how the light LCD screen tints its glass with its own ink.
#[must_use]
pub fn pale(color: Color32) -> Color32 {
    let [_l, a, b] = to_oklab(color);
    from_oklab([0.93, a * 0.35, b * 0.35])
}

/// `oklch(L C H)` as a colour, for the fixed hardware inks.
#[must_use]
pub fn oklch(lightness: f32, chroma: f32, hue_degrees: f32) -> Color32 {
    let hue = hue_degrees.to_radians();
    from_oklab([lightness, chroma * hue.cos(), chroma * hue.sin()])
}

/// A colour with its alpha replaced, for the `color-mix(…, transparent)` tokens.
#[must_use]
pub fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_trip_through_oklab_keeps_the_colour() {
        for color in [
            Color32::from_rgb(12, 10, 9),
            Color32::from_rgb(0, 212, 146),
            Color32::from_rgb(253, 199, 0),
            Color32::from_rgb(231, 229, 228),
        ] {
            let back = from_oklab(to_oklab(color));
            // The float round trip and the gamma curve cost at most a byte.
            for (a, b) in [
                (color.r(), back.r()),
                (color.g(), back.g()),
                (color.b(), back.b()),
            ] {
                assert!((i16::from(a) - i16::from(b)).abs() <= 1, "{color:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn black_stays_black_and_white_loses_nothing() {
        assert_eq!(dim(Color32::BLACK, 0.38), Color32::BLACK);
        let grey = dim(Color32::WHITE, 0.6);
        assert_eq!(grey.r(), grey.g());
        assert_eq!(grey.g(), grey.b());
        assert!((i16::from(grey.r()) - 128).abs() <= 2, "OKLab L=0.6 corresponds to ~128: {grey:?}");
    }

    #[test]
    fn dimming_toward_black_keeps_the_hue_that_css_keeps() {
        let zone = Color32::from_rgb(0, 212, 146);
        let dimmed = dim(zone, 0.6);
        assert!(dimmed.g() > dimmed.r() && dimmed.g() > dimmed.b(), "still green: {dimmed:?}");
        assert!(dimmed.r() < 60, "and much darker: {dimmed:?}");
    }

    #[test]
    fn the_hardware_inks_sort_to_their_published_pixels() {
        // Measured against the browser's own oklch() conversion.
        assert_close(oklch(0.76, 0.13, 52.0), [242, 152, 95]);
        assert_close(oklch(0.84, 0.12, 205.0), [85, 225, 240]);
        assert_close(oklch(0.15, 0.004, 200.0), [9, 12, 12]);
    }

    #[test]
    fn the_pale_glass_is_light_and_only_slightly_tinted() {
        let glass = pale(Color32::from_rgb(28, 25, 23));
        assert!(glass.r() > 230 && glass.g() > 225 && glass.b() > 220, "{glass:?}");
        assert!(glass.r() > glass.b(), "a warm ink keeps a warm glass: {glass:?}");
    }

    fn assert_close(color: Color32, [r, g, b]: [u8; 3]) {
        for (got, want) in [(color.r(), r), (color.g(), g), (color.b(), b)] {
            assert!((i16::from(got) - i16::from(want)).abs() <= 2, "{color:?} vs {r},{g},{b}");
        }
    }

    #[test]
    fn alpha_is_a_separate_channel_from_colour() {
        let dim_ink = with_alpha(Color32::from_rgb(9, 200, 90), 0.07);
        assert_eq!(dim_ink, Color32::from_rgba_unmultiplied(9, 200, 90, 18));
        assert_eq!(dim_ink.a(), 18);
        assert!(
            dim_ink.r() < 9 && dim_ink.g() < 200,
            "a transparent ink is premultiplied, not an additive full-strength channel: {dim_ink:?}"
        );
        assert!(dim_ink.g() > dim_ink.r() && dim_ink.g() > dim_ink.b());
    }
}
