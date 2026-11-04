"""Print every audiocn OKLCH token as the Color32 literals theme.rs needs."""
from color import oklch, dim  # reuse the converter


def line(name, triple, alpha=None):
    if alpha is None:
        return f"            {name}: Color32::from_rgb{triple},"
    r, g, b = triple
    return f"            {name}: Color32::from_rgb({r}, {g}, {b}).linear_multiply({alpha}),"


LIGHT_VARS = [
    ("background", (1, 0, 0)), ("foreground", (0.147, 0.004, 49.25)),
    ("card", (1, 0, 0)), ("card_foreground", (0.147, 0.004, 49.25)),
    ("popover", (1, 0, 0)), ("popover_foreground", (0.147, 0.004, 49.25)),
    ("primary", (0.216, 0.006, 56.043)), ("primary_foreground", (0.985, 0.001, 106.423)),
    ("secondary", (0.97, 0.001, 106.424)), ("secondary_foreground", (0.216, 0.006, 56.043)),
    ("muted", (0.97, 0.001, 106.424)), ("muted_foreground", (0.5, 0.013, 58.071)),
    ("accent", (0.97, 0.001, 106.424)), ("accent_foreground", (0.216, 0.006, 56.043)),
    ("destructive", (0.577, 0.245, 27.325)),
    ("border", (0.923, 0.003, 48.717)), ("input", (0.923, 0.003, 48.717)),
    ("ring", (0.709, 0.01, 56.259)),
    ("meter_ok", (0.696, 0.17, 162.48)), ("meter_warn", (0.795, 0.184, 86.047)),
    ("meter_clip", (0.577, 0.245, 27.325)),
    ("channel_mute", (0.577, 0.245, 27.325)), ("channel_solo", (0.795, 0.184, 86.047)),
    ("channel_monitor", (0.623, 0.214, 259.815)),
    ("chart_1", (0.808, 0.114, 19.571)), ("chart_2", (0.637, 0.237, 25.331)),
    ("chart_3", (0.577, 0.245, 27.325)), ("chart_4", (0.505, 0.213, 27.518)),
    ("chart_5", (0.444, 0.177, 26.899)),
]

DARK_VARS = [
    ("background", (0.147, 0.004, 49.25)), ("foreground", (0.985, 0.001, 106.423)),
    ("card", (0.216, 0.006, 56.043)), ("card_foreground", (0.985, 0.001, 106.423)),
    ("popover", (0.216, 0.006, 56.043)), ("popover_foreground", (0.985, 0.001, 106.423)),
    ("primary", (0.923, 0.003, 48.717)), ("primary_foreground", (0.216, 0.006, 56.043)),
    ("secondary", (0.268, 0.007, 34.298)), ("secondary_foreground", (0.985, 0.001, 106.423)),
    ("muted", (0.268, 0.007, 34.298)), ("muted_foreground", (0.709, 0.01, 56.259)),
    ("accent", (0.268, 0.007, 34.298)), ("accent_foreground", (0.985, 0.001, 106.423)),
    ("destructive", (0.704, 0.191, 22.216)),
    ("ring", (0.553, 0.013, 58.071)),
    ("meter_ok", (0.765, 0.177, 163.223)), ("meter_warn", (0.852, 0.199, 91.936)),
    ("meter_clip", (0.704, 0.191, 22.216)),
    ("channel_mute", (0.704, 0.191, 22.216)), ("channel_solo", (0.852, 0.199, 91.936)),
    ("channel_monitor", (0.707, 0.165, 254.624)),
    ("chart_1", (0.808, 0.114, 19.571)), ("chart_2", (0.637, 0.237, 25.331)),
    ("chart_3", (0.577, 0.245, 27.325)), ("chart_4", (0.505, 0.213, 27.518)),
    ("chart_5", (0.444, 0.177, 26.899)),
]

PRESETS = {
    "Ocean": {
        "light": {"primary": (0.5, 0.215, 262.881), "primary_foreground": (0.985, 0, 0),
                  "ring": (0.707, 0.165, 254.624), "meter_ok": (0.704, 0.14, 182.503),
                  "channel_monitor": (0.546, 0.215, 262.881)},
        "dark": {"primary": (0.707, 0.165, 254.624), "primary_foreground": (0.21, 0.006, 285.885),
                 "ring": (0.546, 0.215, 262.881), "meter_ok": (0.777, 0.152, 181.912)},
    },
    "Rose": {
        "light": {"primary": (0.5, 0.253, 17.585), "primary_foreground": (0.985, 0, 0),
                  "ring": (0.712, 0.194, 13.428), "meter_ok": (0.696, 0.17, 162.48),
                  "channel_solo": (0.769, 0.188, 70.08)},
        "dark": {"primary": (0.712, 0.194, 13.428), "primary_foreground": (0.21, 0.006, 285.885),
                 "ring": (0.586, 0.253, 17.585)},
    },
    "Forest": {
        "light": {"primary": (0.527, 0.154, 150.069), "primary_foreground": (0.985, 0, 0),
                  "ring": (0.792, 0.209, 151.711), "meter_ok": (0.723, 0.219, 149.579)},
        "dark": {"primary": (0.792, 0.209, 151.711), "primary_foreground": (0.266, 0.065, 152.934),
                 "ring": (0.627, 0.194, 149.214), "meter_ok": (0.792, 0.209, 151.711)},
    },
    "Violet": {
        "light": {"primary": (0.491, 0.27, 292.581), "primary_foreground": (0.985, 0, 0),
                  "ring": (0.702, 0.183, 293.541), "channel_monitor": (0.606, 0.25, 292.717)},
        "dark": {"primary": (0.702, 0.183, 293.541), "primary_foreground": (0.21, 0.006, 285.885),
                 "ring": (0.541, 0.281, 293.009), "channel_monitor": (0.702, 0.183, 293.541)},
    },
    "Mono": {
        "light": {"primary": (0.205, 0, 0), "primary_foreground": (0.985, 0, 0),
                  "meter_ok": (0.556, 0, 0), "meter_warn": (0.371, 0, 0),
                  "meter_clip": (0.145, 0, 0), "channel_mute": (0.145, 0, 0),
                  "channel_solo": (0.439, 0, 0), "channel_monitor": (0.556, 0, 0)},
        "dark": {"primary": (0.922, 0, 0), "primary_foreground": (0.205, 0, 0),
                 "meter_ok": (0.708, 0, 0), "meter_warn": (0.87, 0, 0),
                 "meter_clip": (0.985, 0, 0), "channel_mute": (0.985, 0, 0),
                 "channel_solo": (0.87, 0, 0), "channel_monitor": (0.708, 0, 0)},
    },
}

for label, table, alpha in (("dark", DARK_VARS, None), ("light", LIGHT_VARS, None)):
    print(f"// ---- Palette::{label}() ----")
    for name, lch in table:
        rgb = oklch(*lch)
        if name == "border" and label == "dark":
            print(line("border", (255, 255, 255), "0.10"))
        elif name == "input" and label == "dark":
            print(line("input", (255, 255, 255), "0.15"))
        else:
            print(line(name, rgb))
    # the *-foreground text colours: light mixes to black 40%, dark uses the raw colour
    for name, lch in table:
        if name.startswith(("meter_", "channel_")):
            suffix = "_foreground"
            if label == "light":
                print(line(name + suffix, dim(*lch)))
            else:
                print(line(name + suffix, oklch(*lch)))
    print()

for preset, sides in PRESETS.items():
    for side, overrides in sides.items():
        print(f"// ---- {preset} / {side} overrides ----")
        for name, lch in overrides.items():
            print(line(name, oklch(*lch)))
        print()

print("// radius: --radius 0.625rem = 10px; sm 6, md 8, lg 10, xl 14, 2xl 18, 3xl 22, 4xl 26")
