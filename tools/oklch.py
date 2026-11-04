"""Convert audiocn's OKLCH theme tokens to the sRGB bytes egui needs."""


def oklch(L, C, H):
    a = C * __import__("math").cos(H * __import__("math").pi / 180.0)
    b = C * __import__("math").sin(H * __import__("math").pi / 180.0)
    l_ = L + 0.3963377774 * a + 0.2158037660 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_ ** 3, m_ ** 3, s_ ** 3
    rgb = [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
    out = []
    for c in rgb:
        c = max(0.0, min(1.0, c))
        c = 12.92 * c if c <= 0.0031308 else 1.055 * (c ** (1 / 2.4)) - 0.055
        out.append(round(max(0.0, min(1.0, c)) * 255))
    return tuple(out)


def dim(L, C, H):
    """color-mix(in oklch, color, black 40%) -> 60% of L and C, hue kept."""
    return oklch(0.6 * L, 0.6 * C, H)


def fmt(name, rgb, alpha=None):
    if alpha is None:
        return f"{name:26} Color32::from_rgb{rgb}"
    return f"{name:26} Color32::from_rgb({rgb[0]}, {rgb[1]}, {rgb[2]}).linear_multiply({alpha:.2f})"


LIGHT = [
    ("background", 1, 0, 0), ("foreground", 0.147, 0.004, 49.25),
    ("card", 1, 0, 0), ("card_foreground", 0.147, 0.004, 49.25),
    ("popover", 1, 0, 0), ("primary", 0.216, 0.006, 56.043),
    ("primary_foreground", 0.985, 0.001, 106.423),
    ("secondary", 0.97, 0.001, 106.424), ("muted", 0.97, 0.001, 106.424),
    ("muted_foreground", 0.5, 0.013, 58.071),
    ("accent", 0.97, 0.001, 106.424),
    ("destructive", 0.577, 0.245, 27.325),
    ("border", 0.923, 0.003, 48.717), ("input", 0.923, 0.003, 48.717),
    ("ring", 0.709, 0.01, 56.259),
    ("meter_ok", 0.696, 0.17, 162.48), ("meter_warn", 0.795, 0.184, 86.047),
    ("channel_monitor", 0.623, 0.214, 259.815),
    ("sidebar", 0.985, 0.001, 106.423),
    ("chart_1", 0.808, 0.114, 19.571), ("chart_2", 0.637, 0.237, 25.331),
    ("chart_3", 0.577, 0.245, 27.325), ("chart_4", 0.505, 0.213, 27.518),
    ("chart_5", 0.444, 0.177, 26.899),
]

DARK = [
    ("background", 0.147, 0.004, 49.25), ("foreground", 0.985, 0.001, 106.423),
    ("card", 0.216, 0.006, 56.043), ("popover", 0.216, 0.006, 56.043),
    ("primary", 0.923, 0.003, 48.717),
    ("primary_foreground", 0.216, 0.006, 56.043),
    ("secondary", 0.268, 0.007, 34.298), ("muted", 0.268, 0.007, 34.298),
    ("muted_foreground", 0.709, 0.01, 56.259),
    ("accent", 0.268, 0.007, 34.298),
    ("destructive", 0.704, 0.191, 22.216), ("ring", 0.553, 0.013, 58.071),
    ("meter_ok", 0.765, 0.177, 163.223), ("meter_warn", 0.852, 0.199, 91.936),
    ("channel_monitor", 0.707, 0.165, 254.624),
    ("sidebar", 0.216, 0.006, 56.043),
]

print("== light ==")
for n, L, C, H in LIGHT:
    print(fmt(n, oklch(L, C, H)))
for n, L, C, H in LIGHT:
    if n.startswith(("meter_", "channel_")) and not n.endswith("foreground"):
        print(fmt(n + "_foreground", dim(L, C, H)))
print("border/input are opaque here; dark: border=white 10%, input=white 15%")

print("== dark ==")
for n, L, C, H in DARK:
    print(fmt(n, oklch(L, C, H)))
print(fmt("border", (0, 0, 0), 0.10))
print(fmt("input", (0, 0, 0), 0.15))
for n, L, C, H in DARK:
    if n.startswith(("meter_", "channel_")) and not n.endswith("foreground"):
        print(fmt(n + "_dark", oklch(L, C, H)))
