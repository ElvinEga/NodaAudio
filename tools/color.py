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
    return ""
