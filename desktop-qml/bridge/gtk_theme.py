"""The "Desktop (GTK)" theme source: follows the colours the desktop publishes in the user's GTK
config, with no GTK dependency. Port of desktop-gpui `crates/app/src/theme/gtk.rs`.

Desktops write their palette as `@define-color` entries (GNOME/libadwaita, and COSMIC, which
generates `~/.config/gtk-4.0/cosmic/{dark,light}.css` and symlinks `gtk.css` to the active one) or
as `--name:` custom properties. Files are read lowest priority first; later ones win.

Everything here is plain data and pure functions, so the mapping is unit-tested without a display.
When the user has no usable CSS the preset is simply not offered (`preset()` returns None).
"""
from __future__ import annotations

import math
import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

GTK_PRESET_NAME = "gtk"
GTK_PRESET_LABEL = "Desktop (GTK)"

# Files that make up the palette, lowest priority first.
USER_CSS_FILES = (
    ("gtk-3.0", "colors.css"),
    ("gtk-3.0", "gtk.css"),
    ("gtk-4.0", "colors.css"),
    ("gtk-4.0", "gtk.css"),
)

GNOME_BLUE = 0x3584E4
FALLBACK_ERROR = 0xEF4444
# Accent vs background contrast below which the accent is considered invisible (COSMIC themes may
# set the accent equal to the window colour).
MIN_ACCENT_CONTRAST = 1.6
MIN_MUTED_CONTRAST = 4.5

Rgba = tuple[float, float, float, float]  # 0..1 each


def _round(x: float) -> int:
    """Round half away from zero like Rust's f32::round (Python's round() is banker's)."""
    return int(math.floor(x + 0.5)) if x >= 0 else -int(math.floor(-x + 0.5))


def normalize_css_name(name: str) -> str:
    """`--window-bg-color`, `window_bg_color` and `@window_bg_color` collapse to `window_bg_color`."""
    n = name.strip()
    while n.startswith("--"):
        n = n[2:]
    n = n.lstrip("@").strip()
    return n.lower().replace("-", "_")


class UserCss:
    """Named colours from the user's GTK CSS. Values keep their raw text so `var()` / `@name`
    indirection is followed at lookup time."""

    def __init__(self) -> None:
        self.vars: dict[str, str] = {}
        self.defines: dict[str, str] = {}

    def is_empty(self) -> bool:
        return not self.vars and not self.defines

    def raw(self, name: str) -> str | None:
        key = normalize_css_name(name)
        return self.vars.get(key) if key in self.vars else self.defines.get(key)

    def get(self, name: str) -> Rgba | None:
        value = self.raw(name)
        if value is None:
            return None
        for _ in range(4):
            ref = None
            if value.startswith("var(") and value.endswith(")"):
                ref = value[4:-1].strip()
            elif value.startswith("@"):
                ref = value[1:].strip()
            if ref is None:
                return parse_css_color(value)
            value = self.raw(ref)
            if value is None:
                return None
        return None

    def get_any(self, names: list[str] | tuple[str, ...]) -> Rgba | None:
        for n in names:
            c = self.get(n)
            if c is not None:
                return c
        return None


def _channel(p: str) -> float | None:
    try:
        return float(p.split("%")[0].strip())
    except ValueError:
        return None


def parse_css_color(value: str) -> Rgba | None:
    """`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` and `transparent`. Anything else
    (`alpha()`, `color-mix`, `currentColor`) returns None."""
    value = value.strip()
    if value.startswith("#"):
        h = value[1:]
        if not h.isascii():
            return None
        try:
            if len(h) in (3, 4):
                n = [int(c, 16) * 17 for c in h]
                a = n[3] if len(h) == 4 else 255
                return (n[0] / 255, n[1] / 255, n[2] / 255, a / 255)
            if len(h) in (6, 8):
                b = [int(h[i:i + 2], 16) for i in range(0, len(h), 2)]
                a = b[3] if len(h) == 8 else 255
                return (b[0] / 255, b[1] / 255, b[2] / 255, a / 255)
        except ValueError:
            return None
        return None
    if value.lower() == "transparent":
        return (0.0, 0.0, 0.0, 0.0)
    op = value.find("(")
    if op < 0:
        return None
    if value[:op].strip().lower() not in ("rgb", "rgba"):
        return None
    inner = value[op + 1:].rstrip(")").strip()
    parts = [p.strip() for p in inner.split(",")]
    if len(parts) < 3:
        return None
    rgb = [_channel(p) for p in parts[:3]]
    if any(c is None for c in rgb):
        return None
    a = 1.0
    if len(parts) > 3:
        a = _channel(parts[3])
        if a is None:
            return None
    to_byte = lambda v: _round(min(255.0, max(0.0, v))) & 0xFF  # noqa: E731
    return (to_byte(rgb[0]) / 255, to_byte(rgb[1]) / 255, to_byte(rgb[2]) / 255,
            _round(min(1.0, max(0.0, a)) * 255) / 255)


def strip_css_comments(css: str) -> str:
    """Drop `/* ... */` spans, keeping their newlines so declarations around one aren't glued."""
    out: list[str] = []
    rest = css
    while True:
        start = rest.find("/*")
        if start < 0:
            break
        out.append(rest[:start])
        body = rest[start + 2:]
        end = body.find("*/")
        skipped, rest = (body[:end], body[end + 2:]) if end >= 0 else (body, "")
        out.append("\n" * skipped.count("\n"))
    out.append(rest)
    return "".join(out)


def _clean_value(value: str) -> str:
    return value.split(";")[0].strip()


_DECL = re.compile(r"@define-color|--")


def parse_css_into(css: str, out: UserCss) -> None:
    """Parse `--name: value;` custom properties and `@define-color name value;` declarations."""
    css = strip_css_comments(css)
    i, n = 0, len(css)
    while i < n:
        m = _DECL.search(css, i)
        if not m:
            return
        i = m.start()
        if m.group() == "@define-color":
            start = i + len("@define-color")
            end = css.find(";", start)
            decl = css[start:end if end >= 0 else n]
            parts = decl.strip().split(None, 1)
            if len(parts) == 2:
                out.defines[normalize_css_name(parts[0])] = _clean_value(parts[1])
            i = end + 1 if end >= 0 else n
        else:
            end = css.find(";", i + 2)
            decl = css[i + 2:end if end >= 0 else n]
            if ":" in decl:
                name, value = decl.split(":", 1)
                out.vars[normalize_css_name(name)] = _clean_value(value)
            i = end + 1 if end >= 0 else n


def config_dir() -> Path | None:
    x = os.environ.get("XDG_CONFIG_HOME")
    if x:
        return Path(x)
    home = os.environ.get("HOME")
    return Path(home) / ".config" if home else None


def user_css_paths(config: Path | None = None) -> list[Path]:
    config = config or config_dir()
    if config is None:
        return []
    return [config / d / f for d, f in USER_CSS_FILES]


def load_user_css(config: Path | None = None) -> UserCss:
    out = UserCss()
    for p in user_css_paths(config):
        try:
            parse_css_into(p.read_text(errors="replace"), out)
        except OSError:
            continue
    return out


def current_key(config: Path | None = None) -> tuple:
    """Change detector for live updates. Covers each file's resolved target *and* its mtime: COSMIC
    switches dark/light by re-pointing the `gtk.css` symlink, which alone changes neither mtime."""
    key = []
    for p in user_css_paths(config):
        try:
            target = Path(os.path.realpath(p))
            key.append((str(target), int(target.stat().st_mtime)))
        except OSError:
            continue
    return tuple(key)


# --- colour helpers (0xRRGGBB) ----------------------------------------------------------------

def hex_from_rgba(c: Rgba) -> int:
    ch = lambda v: _round(min(1.0, max(0.0, v)) * 255) & 0xFF  # noqa: E731
    return (ch(c[0]) << 16) | (ch(c[1]) << 8) | ch(c[2])


def mix_hex(a: int, b: int, t: float) -> int:
    t = min(1.0, max(0.0, t))

    def lerp(shift: int) -> int:
        ca, cb = (a >> shift) & 0xFF, (b >> shift) & 0xFF
        return _round(ca + (cb - ca) * t) & 0xFF

    return (lerp(16) << 16) | (lerp(8) << 8) | lerp(0)


def luminance(h: int) -> float:
    def lin(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    c = lambda shift: lin(((h >> shift) & 0xFF) / 255)  # noqa: E731
    return 0.2126 * c(16) + 0.7152 * c(8) + 0.0722 * c(0)


def contrast(a: int, b: int) -> float:
    l1, l2 = luminance(a), luminance(b)
    hi, lo = (l1, l2) if l1 >= l2 else (l2, l1)
    return (hi + 0.05) / (lo + 0.05)


def on_color(h: int) -> int:
    return 0x000000 if luminance(h) > 0.45 else 0xFFFFFF


def readable_muted(muted: int, surfaces: list[int], fg: int) -> int:
    """Nudge `muted` toward `fg` until it reads on every surface."""
    ok = lambda c: all(contrast(c, s) >= MIN_MUTED_CONTRAST for s in surfaces)  # noqa: E731
    if ok(muted):
        return muted
    for step in range(1, 21):
        c = mix_hex(muted, fg, step / 20)
        if ok(c):
            return c
    return fg


@dataclass
class GtkPalette:
    """The resolved palette, as the tokens the app styles with."""
    background: int
    foreground: int
    card: int
    primary: int
    primary_foreground: int
    secondary: int
    muted: int
    muted_foreground: int
    accent: int
    accent_foreground: int
    destructive: int
    border: int

    def to_preset(self) -> dict[str, Any]:
        hx = lambda v: f"#{v:06x}"  # noqa: E731
        return {
            "name": GTK_PRESET_NAME,
            "label": GTK_PRESET_LABEL,
            "colors": {
                "background": hx(self.background), "foreground": hx(self.foreground),
                "card": hx(self.card), "cardForeground": hx(self.foreground),
                "primary": hx(self.primary), "primaryForeground": hx(self.primary_foreground),
                "secondary": hx(self.secondary), "secondaryForeground": hx(self.foreground),
                "muted": hx(self.muted), "mutedForeground": hx(self.muted_foreground),
                "accent": hx(self.accent), "accentForeground": hx(self.accent_foreground),
                "destructive": hx(self.destructive),
                "destructiveForeground": hx(on_color(self.destructive)),
                "border": hx(self.border), "input": hx(self.border), "ring": hx(self.accent),
            },
        }


def palette_from_css(css: UserCss) -> GtkPalette | None:
    """Map named colours onto the token table (pure)."""
    bg_c = css.get_any(["window_bg_color", "theme_bg_color_breeze", "theme_bg_color"])
    fg_c = css.get_any(["window_fg_color", "theme_fg_color_breeze", "theme_fg_color", "theme_text_color"])
    if bg_c is None or fg_c is None:
        return None
    bg, fg = hex_from_rgba(bg_c), hex_from_rgba(fg_c)
    # Deviation from GPUI: translucent surface colours (libadwaita's card_bg_color is
    # rgba(255,255,255,.08)) are composited over the window colour instead of dropping the alpha,
    # which turned the card pure white on dark themes.
    flat = lambda c, over: mix_hex(over, hex_from_rgba(c), c[3])  # noqa: E731
    base_c = css.get_any(["view_bg_color", "theme_base_color_breeze", "theme_base_color"])
    base = flat(base_c, bg) if base_c else bg
    card_c = css.get_any(["card_bg_color", "popover_bg_color", "theme_card_bg_color"])
    card = flat(card_c, bg) if card_c else base

    accent_c = css.get_any(["accent_bg_color", "accent_color", "theme_selected_bg_color_breeze",
                            "theme_selected_bg_color", "blue_1"])
    accent = hex_from_rgba(accent_c) if accent_c else GNOME_BLUE
    on_accent_c = css.get_any(["accent_fg_color", "theme_selected_fg_color_breeze", "theme_selected_fg_color"])
    on_accent: int | None = hex_from_rgba(on_accent_c) if on_accent_c else None
    if contrast(accent, bg) < MIN_ACCENT_CONTRAST:
        # The accent is indistinguishable from the window (some COSMIC themes): use the palette's
        # blue, else the foreground itself.
        blue = css.get("blue_1")
        blue_hex = hex_from_rgba(blue) if blue else None
        accent = blue_hex if blue_hex is not None and contrast(blue_hex, bg) >= MIN_ACCENT_CONTRAST else fg
        on_accent = None
    if on_accent is None:
        on_accent = on_color(accent)

    border_c = css.get_any(["borders_breeze", "borders", "border_color", "sidebar_border_color"])
    border = flat(border_c, bg) if border_c else mix_hex(base, fg, 0.18)

    muted_c = css.get_any(["insensitive_fg_color_breeze", "insensitive_fg_color"])
    muted_fg = mix_hex(bg, hex_from_rgba(muted_c), muted_c[3]) if muted_c else mix_hex(bg, fg, 0.55)

    destr_c = css.get_any(["destructive_bg_color", "error_color_breeze", "error_color", "red_1"])
    destructive = hex_from_rgba(destr_c) if destr_c else FALLBACK_ERROR

    return GtkPalette(
        background=bg, foreground=fg, card=card, primary=accent, primary_foreground=on_accent,
        secondary=mix_hex(card, fg, 0.08), muted=mix_hex(bg, fg, 0.06),
        muted_foreground=readable_muted(muted_fg, [card, bg], fg),
        accent=accent, accent_foreground=on_accent, destructive=destructive, border=border,
    )


def preset(config: Path | None = None) -> dict[str, Any] | None:
    """The desktop palette from the user's GTK CSS, or None without any."""
    css = load_user_css(config)
    if css.is_empty():
        return None
    p = palette_from_css(css)
    return p.to_preset() if p else None
