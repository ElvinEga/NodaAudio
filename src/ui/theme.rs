//! Color and size tokens for the component set.
//!
//! The palette is a one-to-one port of audiocn's theme: every field names the
//! CSS custom property it stands for, and the values are those properties
//! converted from OKLCH to sRGB, so a NodaAudio editor set to the same preset
//! as a web page shows the same colors. `:root` becomes [`Palette::light`],
//! `.dark` becomes [`Palette::dark`], and each `[data-theme="…"]` block becomes
//! a [`Preset`] layered over whichever base is in use.

use egui::{Color32, Visuals};

use crate::zones::Zone;

/// Where a widget sits on the size ladder.
///
/// Sizes are relative, not absolute: a `Small` fader is a compact console
/// lane, a `Large` one is a solo control filling a plugin's center.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    /// Dense layout for a mixer with many lanes.
    Small,
    /// The default.
    Medium,
    /// Roomy layout for a single-purpose control.
    Large,
}

impl Size {
    /// Meter bar or bar-group thickness, in points.
    #[must_use]
    pub fn meter_thickness(self) -> f32 {
        match self {
            Size::Small => 4.0,
            Size::Medium => 8.0,
            Size::Large => 12.0,
        }
    }

    /// Fader and slider track thickness, in points.
    #[must_use]
    pub fn track_thickness(self) -> f32 {
        match self {
            Size::Small => 3.0,
            Size::Medium => 4.0,
            Size::Large => 6.0,
        }
    }

    /// Fader thumb diameter, in points.
    #[must_use]
    pub fn thumb_size(self) -> f32 {
        match self {
            Size::Small => 12.0,
            Size::Medium => 16.0,
            Size::Large => 20.0,
        }
    }

    /// Clip-light dot diameter, in points.
    #[must_use]
    pub fn clip_dot_size(self) -> f32 {
        match self {
            Size::Small => 6.0,
            Size::Medium => 8.0,
            Size::Large => 10.0,
        }
    }

    /// Knob diameter, in points.
    #[must_use]
    pub fn knob_diameter(self) -> f32 {
        match self {
            Size::Small => 40.0,
            Size::Medium => 56.0,
            Size::Large => 76.0,
        }
    }

    /// Font size for scale labels and readouts, in points.
    #[must_use]
    pub fn label_font(self) -> f32 {
        match self {
            Size::Small => 9.0,
            Size::Medium => 10.0,
            Size::Large => 12.0,
        }
    }

    /// Font size for a value readout, in points.
    #[must_use]
    pub fn value_font(self) -> f32 {
        match self {
            Size::Small => 10.0,
            Size::Medium => 12.0,
            Size::Large => 14.0,
        }
    }

    /// Space between the parts of a composite widget, in points.
    #[must_use]
    pub fn gap(self) -> f32 {
        match self {
            Size::Small => 4.0,
            Size::Medium => 6.0,
            Size::Large => 8.0,
        }
    }
}

/// audiocn's `--radius` ladder: a base of `0.625rem` scaled per step.
#[derive(Clone, Copy, Debug)]
pub struct Radius {
    /// The `--radius` token itself, which `rounded-lg` uses verbatim.
    pub base: f32,
}

impl Radius {
    /// The default base: `0.625rem` at a 16 px root.
    pub const DEFAULT_BASE: f32 = 10.0;

    /// A ladder with the given base, in points.
    #[must_use]
    pub const fn new(base: f32) -> Self {
        Self { base }
    }

    /// `rounded-sm`.
    #[must_use]
    pub const fn sm(self) -> f32 {
        self.base * 0.6
    }

    /// `rounded-md`: the radius on a control's own body.
    #[must_use]
    pub const fn md(self) -> f32 {
        self.base * 0.8
    }

    /// `rounded-lg`: panels, cards, strips.
    #[must_use]
    pub const fn lg(self) -> f32 {
        self.base
    }

    /// `rounded-xl`.
    #[must_use]
    pub const fn xl(self) -> f32 {
        self.base * 1.4
    }

    /// `rounded-2xl`: the outer frame of a composed widget.
    #[must_use]
    pub const fn two_xl(self) -> f32 {
        self.base * 1.8
    }

    /// `rounded-3xl`.
    #[must_use]
    pub const fn three_xl(self) -> f32 {
        self.base * 2.2
    }

    /// `rounded-4xl`.
    #[must_use]
    pub const fn four_xl(self) -> f32 {
        self.base * 2.6
    }
}

impl Default for Radius {
    fn default() -> Self {
        Self::new(Self::DEFAULT_BASE)
    }
}

/// The `[data-theme="…"]` presets, applied over a base palette.
///
/// Each preset replaces only the few properties audiocn's CSS replaces: the
/// accent and ring, and whichever meter or channel colors the preset names.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Preset {
    /// The site default: warm stone accents, green/amber/red metering.
    #[default]
    Standard,
    /// Blue accents and a teal-ok meter.
    Ocean,
    /// Pink-red accents and an amber solo.
    Rose,
    /// Green accents and a brighter ok zone.
    Forest,
    /// Purple accents and a purple monitor lamp.
    Violet,
    /// No hue at all: the zones read as gray steps.
    Mono,
}

/// A color set for the component library.
///
/// The doc comment on each field names the CSS variable it ports, which is
/// how the widgets choose colors: a widget that audiocn paints `bg-muted`
/// takes [`Palette::track`], one painted `text-muted-foreground` takes
/// [`Palette::text_dim`].
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// `--background`: behind everything.
    pub background: Color32,
    /// `--foreground`: text and marks that must stand out.
    pub foreground: Color32,
    /// `--card`: a raised panel on the background.
    pub surface: Color32,
    /// `--card-foreground`: text on a raised panel.
    pub surface_foreground: Color32,
    /// `--muted`, `--secondary`, `--accent`: recessed fill, which is where
    /// meter and fader tracks live.
    pub track: Color32,
    /// `--muted-foreground`: secondary text, captions, inactive labels.
    pub text_dim: Color32,
    /// `--popover`: a floating surface above everything else.
    pub popover: Color32,
    /// `--border`: hairline edges.
    pub border: Color32,
    /// `--input`: the outline of an editable field, a touch stronger than
    /// [`Self::border`].
    pub input: Color32,
    /// `--primary`: the interactive accent — fills, focus, selection.
    pub primary: Color32,
    /// `--primary-foreground`: text sitting on [`Self::primary`].
    pub primary_foreground: Color32,
    /// `--secondary-foreground` and `--accent-foreground`: text on a
    /// recessed fill.
    pub on_track: Color32,
    /// `--destructive`: an error, and the clip color a meter reaches at
    /// full scale.
    pub destructive: Color32,
    /// `--ring`: focus and attention ring.
    pub ring: Color32,
    /// `--meter-ok`: a level a meter shows as acceptable.
    pub meter_ok: Color32,
    /// `--meter-warn`: a level a meter shows as getting loud.
    pub meter_warn: Color32,
    /// `--meter-clip`: a level at or near full scale.
    pub meter_clip: Color32,
    /// `--meter-ok-foreground`: text over the ok zone.
    pub meter_ok_foreground: Color32,
    /// `--meter-warn-foreground`: text over the warn zone.
    pub meter_warn_foreground: Color32,
    /// `--meter-clip-foreground`: text over the clip zone.
    pub meter_clip_foreground: Color32,
    /// `--channel-mute`: a muted channel.
    pub channel_mute: Color32,
    /// `--channel-solo`: a soloed channel.
    pub channel_solo: Color32,
    /// `--channel-monitor`: a monitored or live-input channel.
    pub channel_monitor: Color32,
    /// `--channel-mute-foreground`: the label of a lit mute.
    pub channel_mute_foreground: Color32,
    /// `--channel-solo-foreground`: the label of a lit solo.
    pub channel_solo_foreground: Color32,
    /// `--channel-monitor-foreground`: the label of a lit monitor.
    pub channel_monitor_foreground: Color32,
    /// `--chart-1` … `--chart-5`: the warm ramp the spectrum and bar
    /// visualizers climb.
    pub chart: [Color32; 5],
}

impl Palette {
    /// The dark set: `.dark` in audiocn's CSS, converted from OKLCH.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            background: Color32::from_rgb(12, 10, 9),
            foreground: Color32::from_rgb(250, 250, 249),
            surface: Color32::from_rgb(28, 25, 23),
            surface_foreground: Color32::from_rgb(250, 250, 249),
            track: Color32::from_rgb(41, 37, 36),
            text_dim: Color32::from_rgb(166, 160, 155),
            popover: Color32::from_rgb(28, 25, 23),
            border: Color32::from_white_alpha(26),
            input: Color32::from_white_alpha(38),
            primary: Color32::from_rgb(231, 229, 228),
            primary_foreground: Color32::from_rgb(28, 25, 23),
            on_track: Color32::from_rgb(250, 250, 249),
            destructive: Color32::from_rgb(255, 100, 103),
            ring: Color32::from_rgb(121, 113, 107),
            meter_ok: Color32::from_rgb(0, 212, 146),
            meter_warn: Color32::from_rgb(253, 199, 0),
            meter_clip: Color32::from_rgb(255, 100, 103),
            meter_ok_foreground: Color32::from_rgb(0, 212, 146),
            meter_warn_foreground: Color32::from_rgb(253, 199, 0),
            meter_clip_foreground: Color32::from_rgb(255, 100, 103),
            channel_mute: Color32::from_rgb(255, 100, 103),
            channel_solo: Color32::from_rgb(253, 199, 0),
            channel_monitor: Color32::from_rgb(81, 162, 255),
            channel_mute_foreground: Color32::from_rgb(255, 100, 103),
            channel_solo_foreground: Color32::from_rgb(253, 199, 0),
            channel_monitor_foreground: Color32::from_rgb(81, 162, 255),
            chart: [
                Color32::from_rgb(255, 162, 162),
                Color32::from_rgb(251, 44, 54),
                Color32::from_rgb(231, 0, 11),
                Color32::from_rgb(193, 0, 7),
                Color32::from_rgb(159, 7, 18),
            ],
        }
    }

    /// The light set: audiocn's `:root`, where a zone used as text is mixed
    /// 40% toward black so it stays legible on white.
    #[must_use]
    pub fn light() -> Self {
        Self {
            background: Color32::from_rgb(255, 255, 255),
            foreground: Color32::from_rgb(12, 10, 9),
            surface: Color32::from_rgb(255, 255, 255),
            surface_foreground: Color32::from_rgb(12, 10, 9),
            track: Color32::from_rgb(245, 245, 244),
            text_dim: Color32::from_rgb(105, 98, 92),
            popover: Color32::from_rgb(255, 255, 255),
            border: Color32::from_rgb(231, 229, 228),
            input: Color32::from_rgb(231, 229, 228),
            primary: Color32::from_rgb(28, 25, 23),
            primary_foreground: Color32::from_rgb(250, 250, 249),
            on_track: Color32::from_rgb(28, 25, 23),
            destructive: Color32::from_rgb(231, 0, 11),
            ring: Color32::from_rgb(166, 160, 155),
            meter_ok: Color32::from_rgb(0, 188, 125),
            meter_warn: Color32::from_rgb(240, 177, 0),
            meter_clip: Color32::from_rgb(231, 0, 11),
            meter_ok_foreground: Color32::from_rgb(0, 93, 59),
            meter_warn_foreground: Color32::from_rgb(120, 87, 0),
            meter_clip_foreground: Color32::from_rgb(115, 0, 2),
            channel_mute: Color32::from_rgb(231, 0, 11),
            channel_solo: Color32::from_rgb(240, 177, 0),
            channel_monitor: Color32::from_rgb(43, 127, 255),
            channel_mute_foreground: Color32::from_rgb(115, 0, 2),
            channel_solo_foreground: Color32::from_rgb(120, 87, 0),
            channel_monitor_foreground: Color32::from_rgb(16, 60, 131),
            chart: [
                Color32::from_rgb(255, 162, 162),
                Color32::from_rgb(251, 44, 54),
                Color32::from_rgb(231, 0, 11),
                Color32::from_rgb(193, 0, 7),
                Color32::from_rgb(159, 7, 18),
            ],
        }
    }

    /// Is this the dark set? Decided by the background, since that is the
    /// one property every variant of the palette has.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        let [r, g, b, _] = self.background.to_array();
        u16::from(r) + u16::from(g) + u16::from(b) < 383
    }

    /// This palette with one of audiocn's `data-theme` presets applied.
    #[must_use]
    pub fn recolored(&self, preset: Preset) -> Self {
        let mut palette = self.clone();
        preset.apply(&mut palette, self.is_dark());
        palette
    }

    /// The fill color for a metering zone.
    #[must_use]
    pub fn zone_color(&self, zone: Zone) -> Color32 {
        match zone {
            Zone::Ok => self.meter_ok,
            Zone::Warn => self.meter_warn,
            Zone::Clip => self.meter_clip,
        }
    }

    /// The text color for a metering zone: the zone itself on a dark
    /// background, mixed toward black on a light one.
    #[must_use]
    pub fn zone_foreground(&self, zone: Zone) -> Color32 {
        match zone {
            Zone::Ok => self.meter_ok_foreground,
            Zone::Warn => self.meter_warn_foreground,
            Zone::Clip => self.meter_clip_foreground,
        }
    }

    /// The color of a lit channel flag.
    #[must_use]
    pub fn channel_color(&self, channel: Channel) -> Color32 {
        match channel {
            Channel::Mute => self.channel_mute,
            Channel::Solo => self.channel_solo,
            Channel::Monitor => self.channel_monitor,
        }
    }

    /// The label color of a lit channel flag.
    #[must_use]
    pub fn channel_foreground(&self, channel: Channel) -> Color32 {
        match channel {
            Channel::Mute => self.channel_mute_foreground,
            Channel::Solo => self.channel_solo_foreground,
            Channel::Monitor => self.channel_monitor_foreground,
        }
    }

    /// [`egui::Visuals`] painted from this palette, for editors that want
    /// the whole widget set to match and not just the audio components.
    #[must_use]
    pub fn visuals(&self) -> Visuals {
        let mut visuals = if self.is_dark() { Visuals::dark() } else { Visuals::light() };
        visuals.panel_fill = self.background;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.track;
        visuals.faint_bg_color = self.track;
        visuals.override_text_color = Some(self.foreground);

        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, self.text_dim);

        visuals.widgets.inactive.bg_fill = self.track;
        visuals.widgets.inactive.weak_bg_fill = self.surface;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, self.foreground);

        visuals.widgets.hovered.bg_fill = self.track;
        visuals.widgets.hovered.weak_bg_fill = self.popover;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, self.foreground);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, self.ring);

        visuals.widgets.active.bg_fill = self.primary;
        visuals.widgets.active.weak_bg_fill = self.primary;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, self.primary_foreground);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, self.primary);

        visuals.selection.bg_fill = self.primary.linear_multiply(0.45);
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, self.primary);
        visuals
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::dark()
    }
}

/// Which of the three channel flags a toggle can be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    /// Post-fade silence.
    Mute,
    /// This channel alone, or this channel plus the soloed ones.
    Solo,
    /// Live input passing through.
    Monitor,
}

impl Preset {
    /// Overwrite the properties this preset names, for the given base.
    ///
    /// On a light base a `*-foreground` token is the zone mixed 40% toward
    /// black; on a dark base it is the zone itself, exactly as the CSS does
    /// it in `:root` versus `.dark`.
    fn apply(self, palette: &mut Palette, dark: bool) {
        match (self, dark) {
            (Self::Standard, _) => {}
            (Self::Ocean, false) => {
                palette.primary = Color32::from_rgb(23, 84, 219);
                palette.primary_foreground = Color32::from_rgb(250, 250, 250);
                palette.ring = Color32::from_rgb(81, 162, 255);
                palette.meter_ok = Color32::from_rgb(0, 187, 167);
                palette.meter_ok_foreground = Color32::from_rgb(0, 92, 82);
                palette.channel_monitor = Color32::from_rgb(37, 99, 235);
                palette.channel_monitor_foreground = Color32::from_rgb(13, 46, 117);
            }
            (Self::Ocean, true) => {
                palette.primary = Color32::from_rgb(81, 162, 255);
                palette.primary_foreground = Color32::from_rgb(24, 24, 27);
                palette.ring = Color32::from_rgb(37, 99, 235);
                palette.meter_ok = Color32::from_rgb(0, 213, 190);
                palette.meter_ok_foreground = palette.meter_ok;
            }
            (Self::Rose, false) => {
                palette.primary = Color32::from_rgb(204, 0, 38);
                palette.primary_foreground = Color32::from_rgb(250, 250, 250);
                palette.ring = Color32::from_rgb(255, 99, 126);
                palette.channel_solo = Color32::from_rgb(254, 154, 0);
                palette.channel_solo_foreground = Color32::from_rgb(127, 74, 0);
            }
            (Self::Rose, true) => {
                palette.primary = Color32::from_rgb(255, 99, 126);
                palette.primary_foreground = Color32::from_rgb(24, 24, 27);
                palette.ring = Color32::from_rgb(236, 0, 63);
            }
            (Self::Forest, false) => {
                palette.primary = Color32::from_rgb(0, 130, 54);
                palette.primary_foreground = Color32::from_rgb(250, 250, 250);
                palette.ring = Color32::from_rgb(5, 223, 114);
                palette.meter_ok = Color32::from_rgb(0, 201, 80);
                palette.meter_ok_foreground = Color32::from_rgb(0, 99, 36);
            }
            (Self::Forest, true) => {
                palette.primary = Color32::from_rgb(5, 223, 114);
                palette.primary_foreground = Color32::from_rgb(3, 46, 21);
                palette.ring = Color32::from_rgb(0, 166, 62);
                palette.meter_ok = Color32::from_rgb(5, 223, 114);
                palette.meter_ok_foreground = palette.meter_ok;
            }
            (Self::Violet, false) => {
                palette.primary = Color32::from_rgb(112, 8, 231);
                palette.primary_foreground = Color32::from_rgb(250, 250, 250);
                palette.ring = Color32::from_rgb(166, 132, 255);
                palette.channel_monitor = Color32::from_rgb(142, 81, 255);
                palette.channel_monitor_foreground = Color32::from_rgb(68, 36, 132);
            }
            (Self::Violet, true) => {
                palette.primary = Color32::from_rgb(166, 132, 255);
                palette.primary_foreground = Color32::from_rgb(24, 24, 27);
                palette.ring = Color32::from_rgb(127, 34, 254);
                palette.channel_monitor = Color32::from_rgb(166, 132, 255);
                palette.channel_monitor_foreground = palette.channel_monitor;
            }
            (Self::Mono, false) => {
                palette.primary = Color32::from_rgb(23, 23, 23);
                palette.primary_foreground = Color32::from_rgb(250, 250, 250);
                palette.meter_ok = Color32::from_rgb(115, 115, 115);
                palette.meter_warn = Color32::from_rgb(64, 64, 64);
                palette.meter_clip = Color32::from_rgb(10, 10, 10);
                palette.channel_mute = Color32::from_rgb(10, 10, 10);
                palette.channel_solo = Color32::from_rgb(82, 82, 82);
                palette.channel_monitor = Color32::from_rgb(115, 115, 115);
                // `color-mix(in oklch, gray, black 40%)` on a hueless colour is
                // simply a darker gray, so the ink follows its zone down.
                palette.meter_ok_foreground = Color32::from_rgb(54, 54, 54);
                palette.meter_warn_foreground = Color32::from_rgb(27, 27, 27);
                palette.meter_clip_foreground = Color32::from_rgb(2, 2, 2);
                palette.channel_mute_foreground = palette.meter_clip_foreground;
                palette.channel_solo_foreground = Color32::from_rgb(37, 37, 37);
                palette.channel_monitor_foreground = palette.meter_ok_foreground;
            }
            (Self::Mono, true) => {
                palette.primary = Color32::from_rgb(229, 229, 229);
                palette.primary_foreground = Color32::from_rgb(23, 23, 23);
                palette.meter_ok = Color32::from_rgb(161, 161, 161);
                palette.meter_warn = Color32::from_rgb(212, 212, 212);
                palette.meter_clip = Color32::from_rgb(250, 250, 250);
                palette.channel_mute = Color32::from_rgb(250, 250, 250);
                palette.channel_solo = Color32::from_rgb(212, 212, 212);
                palette.channel_monitor = Color32::from_rgb(161, 161, 161);
                palette.meter_ok_foreground = palette.meter_ok;
                palette.meter_warn_foreground = palette.meter_warn;
                palette.meter_clip_foreground = palette.meter_clip;
                palette.channel_mute_foreground = palette.channel_mute;
                palette.channel_solo_foreground = palette.channel_solo;
                palette.channel_monitor_foreground = palette.channel_monitor;
            }
        }
    }
}

/// The palette, size and corner radius a widget should draw itself with.
///
/// Widgets read it from the [`egui::Context`] and fall back to
/// [`Theme::dark`] when nobody has installed one, so a widget dropped into
/// a bare egui app still looks right.
#[derive(Clone, Debug)]
pub struct Theme {
    /// Colors.
    pub palette: Palette,
    /// Size the widgets default to when a call site does not name one.
    pub size: Size,
    /// The `rounded-*` ladder every panel and control picks a step from.
    pub radius: Radius,
    /// Opacity of a meter's unlit segments, so the ladder stays readable
    /// when the level is low.
    pub meter_ghost_opacity: f32,
    /// Width of a focus ring, matching the browser's 2px outline.
    pub ring_width: f32,
}

impl Theme {
    /// The context key a theme is stored under.
    fn data_key() -> egui::Id {
        egui::Id::new("noda-audio::theme")
    }

    /// Dark palette, medium size, standard preset.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            palette: Palette::dark(),
            size: Size::Medium,
            radius: Radius::default(),
            meter_ghost_opacity: 0.2,
            ring_width: 2.0,
        }
    }

    /// Light palette, medium size.
    #[must_use]
    pub fn light() -> Self {
        Self { palette: Palette::light(), ..Theme::dark() }
    }

    /// A theme on the given base with one of audiocn's presets applied.
    #[must_use]
    pub fn with_preset(&self, preset: Preset) -> Self {
        Self { palette: self.palette.recolored(preset), ..self.clone() }
    }

    /// Store the theme in the context and paint egui's own visuals to
    /// match, so standard widgets stop outshining the audio ones.
    pub fn install(&self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(Self::data_key(), self.clone()));
        ctx.set_visuals(self.palette.visuals());
    }

    /// The installed theme from context, or [`Theme::dark`] when none is set.
    #[must_use]
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp::<Self>(Self::data_key())).unwrap_or_else(Self::dark)
    }

    /// The installed theme, or [`Theme::dark`] when none is set.
    #[must_use]
    pub fn of(ui: &egui::Ui) -> Self {
        Self::from_ctx(ui.ctx())
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_scale_monotonically() {
        for (small, medium, large, name) in [
            (
                Size::Small.meter_thickness(),
                Size::Medium.meter_thickness(),
                Size::Large.meter_thickness(),
                "thickness",
            ),
            (Size::Small.thumb_size(), Size::Medium.thumb_size(), Size::Large.thumb_size(), "thumb"),
            (Size::Small.gap(), Size::Medium.gap(), Size::Large.gap(), "gap"),
        ] {
            assert!(small < medium && medium < large, "{name} did not grow");
        }
    }

    #[test]
    fn the_radius_ladder_follows_the_css_multipliers() {
        let radius = Radius::new(10.0);
        assert_eq!(radius.sm(), 6.0);
        assert_eq!(radius.md(), 8.0);
        assert_eq!(radius.lg(), 10.0);
        assert_eq!(radius.two_xl(), 18.0);
        assert_eq!(radius.four_xl(), 26.0);
    }

    #[test]
    fn zones_map_to_their_colors() {
        let palette = Palette::dark();
        assert_eq!(palette.zone_color(Zone::Ok), palette.meter_ok);
        assert_eq!(palette.zone_color(Zone::Warn), palette.meter_warn);
        assert_eq!(palette.zone_color(Zone::Clip), palette.meter_clip);
        assert_eq!(palette.channel_color(Channel::Solo), palette.channel_solo);
    }

    #[test]
    fn a_zone_writes_darker_on_light_than_on_dark() {
        // audiocn mixes the `*-foreground` tokens toward black only in the
        // light theme; on dark the text is the zone itself.
        let light = Palette::light();
        let dark = Palette::dark();
        assert_ne!(light.meter_ok, light.meter_ok_foreground, "light mixes the ink");
        assert_eq!(dark.meter_ok, dark.meter_ok_foreground, "dark keeps it");
    }

    #[test]
    fn the_bases_answer_to_is_dark() {
        assert!(Palette::dark().is_dark());
        assert!(!Palette::light().is_dark());
    }

    #[test]
    fn a_preset_replaces_only_the_properties_it_names() {
        let base = Palette::dark();
        let ocean = base.clone().recolored(Preset::Ocean);
        assert_eq!(ocean.primary, Color32::from_rgb(81, 162, 255));
        assert_eq!(ocean.meter_ok, Color32::from_rgb(0, 213, 190));
        assert_eq!(ocean.meter_warn, base.meter_warn, "warn is not in the preset");
        assert_eq!(ocean.foreground, base.foreground, "nor is the text");
        assert_eq!(
            base.clone().recolored(Preset::Standard),
            base,
            "the standard preset is the base"
        );
    }

    #[test]
    fn mono_meters_have_no_hue_left() {
        for palette in [Palette::dark(), Palette::light()] {
            let mono = palette.recolored(Preset::Mono);
            let [r, g, b, _] = mono.meter_ok.to_array();
            assert_eq!((r, g, b), (g, b, r), "a gray zone has equal channels");
        }
    }

    #[test]
    fn a_theme_round_trips_through_the_context() {
        let ctx = egui::Context::default();
        let mut theme = Theme::dark();
        theme.size = Size::Large;
        theme.install(&ctx);
        let read = ctx
            .data(|data| data.get_temp::<Theme>(Theme::data_key()))
            .expect("theme should be stored");
        assert_eq!(read.size, Size::Large);
        assert_eq!(ctx.global_style().visuals.panel_fill, theme.palette.background);
    }

    #[test]
    fn visuals_take_the_palette_colors() {
        let palette = Palette::dark();
        let visuals = palette.visuals();
        assert_eq!(visuals.panel_fill, palette.background);
        assert_eq!(visuals.widgets.active.bg_fill, palette.primary);
        assert_eq!(visuals.override_text_color, Some(palette.foreground));
    }

    #[test]
    fn the_light_theme_gets_light_visuals() {
        let palette = Palette::light();
        assert!(!palette.visuals().dark_mode, "egui's own widgets should follow the base");
    }
}
