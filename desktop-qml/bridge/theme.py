"""Theme presets (same 83 definitions as GPUI/web) and the active tokens for QML.

Exposed to QML as the context property `Theme` (no import needed):
  colours   Theme.background, Theme.foreground, Theme.card, Theme.cardForeground, Theme.primary,
            Theme.primaryForeground, Theme.secondary, Theme.secondaryForeground, Theme.muted,
            Theme.mutedForeground, Theme.accent, Theme.accentForeground, Theme.destructive,
            Theme.destructiveForeground, Theme.border, Theme.input, Theme.ring
  chat      Theme.hasChat (presets that restyle the conversation, e.g. WhatsApp) and
            Theme.chatChatBg, chatHeader, chatBubbleOut, chatBubbleOutText, chatBubbleIn,
            chatBubbleInText, chatListSelected, chatListHover (fall back to card/background when absent)
  scale     Theme.rem (16px * UI scale), Theme.uiScale, Theme.chatScale, Theme.transparency (0..1)
  helpers   Theme.alpha(color, a), Theme.mix(a, b, t), Theme.glass(color)
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, Signal, Slot
from PySide6.QtGui import QColor

from .settings import MAX_FONT_SCALE, MIN_FONT_SCALE, Settings

THEMES_JSON = Path(__file__).resolve().parent.parent / "qml" / "themes" / "themes_data.json"
GTK_PRESET_NAME = "gtk"


def luminance(hex_color: str) -> float:
    h = hex_color.lstrip("#")
    if len(h) < 6:
        return 0.0
    r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))
    lin = lambda v: v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4  # noqa: E731
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)


def is_dark(hex_color: str) -> bool:
    return luminance(hex_color) < 0.5


def load_presets() -> list[dict[str, Any]]:
    return json.loads(THEMES_JSON.read_text())


class ThemeManager(QObject):
    changed = Signal()

    def __init__(self, settings: Settings, parent: QObject | None = None):
        super().__init__(parent)
        self._settings = settings
        self._presets = load_presets()
        self._extra: dict[str, dict[str, Any]] = {}  # e.g. the live desktop (GTK) palette
        self._apply(settings.get_py("theme_preset"))
        settings.changed.connect(self._on_setting)

    # -- python API --------------------------------------------------------------
    def register_preset(self, preset: dict[str, Any]) -> None:
        """Add or replace a preset (used for the live desktop palette)."""
        self._extra[preset["name"]] = preset
        if preset["name"] in (self._preset["name"], self._settings.get_py("theme_preset")):
            self._apply(self._settings.get_py("theme_preset"))
            self.changed.emit()

    def unregister_preset(self, name: str) -> None:
        """Drop a preset added with `register_preset` (e.g. the desktop palette vanished)."""
        if self._extra.pop(name, None) is None:
            return
        if name == self._preset["name"]:
            self._apply(self._settings.get_py("theme_preset"))  # falls back to default-dark
            self.changed.emit()

    def find(self, name: str) -> dict[str, Any] | None:
        return self._extra.get(name) or next((p for p in self._presets if p["name"] == name), None)

    def _apply(self, name: str) -> None:
        self._preset = self.find(name) or self.find("default-dark") or self._presets[0]

    def _on_setting(self, key: str) -> None:
        if key == "theme_preset":
            self._apply(self._settings.get_py("theme_preset"))
        if key in ("theme_preset", "theme_mode", "ui_font_scale", "chat_font_scale", "window_transparency"):
            self.changed.emit()

    def _c(self, key: str) -> QColor:
        return QColor(self._preset["colors"][key])

    def _x(self, key: str) -> QColor:
        ex = self._preset.get("extras") or {}
        fallback = {
            "chatBg": "background", "header": "background", "bubbleOut": "primary",
            "bubbleOutText": "primaryForeground", "bubbleIn": "card", "bubbleInText": "cardForeground",
            "listSelected": "accent", "listHover": "muted",
        }[key]
        return QColor(ex[key]) if key in ex else self._c(fallback)

    # -- colour tokens -----------------------------------------------------------
    background = Property(QColor, lambda s: s._c("background"), notify=changed)
    foreground = Property(QColor, lambda s: s._c("foreground"), notify=changed)
    card = Property(QColor, lambda s: s._c("card"), notify=changed)
    cardForeground = Property(QColor, lambda s: s._c("cardForeground"), notify=changed)
    primary = Property(QColor, lambda s: s._c("primary"), notify=changed)
    primaryForeground = Property(QColor, lambda s: s._c("primaryForeground"), notify=changed)
    secondary = Property(QColor, lambda s: s._c("secondary"), notify=changed)
    secondaryForeground = Property(QColor, lambda s: s._c("secondaryForeground"), notify=changed)
    muted = Property(QColor, lambda s: s._c("muted"), notify=changed)
    mutedForeground = Property(QColor, lambda s: s._c("mutedForeground"), notify=changed)
    accent = Property(QColor, lambda s: s._c("accent"), notify=changed)
    accentForeground = Property(QColor, lambda s: s._c("accentForeground"), notify=changed)
    destructive = Property(QColor, lambda s: s._c("destructive"), notify=changed)
    destructiveForeground = Property(QColor, lambda s: s._c("destructiveForeground"), notify=changed)
    border = Property(QColor, lambda s: s._c("border"), notify=changed)
    input = Property(QColor, lambda s: s._c("input"), notify=changed)
    ring = Property(QColor, lambda s: s._c("ring"), notify=changed)

    # -- chat tokens ---------------------------------------------------------------
    chatChatBg = Property(QColor, lambda s: s._x("chatBg"), notify=changed)
    chatHeader = Property(QColor, lambda s: s._x("header"), notify=changed)
    chatBubbleOut = Property(QColor, lambda s: s._x("bubbleOut"), notify=changed)
    chatBubbleOutText = Property(QColor, lambda s: s._x("bubbleOutText"), notify=changed)
    chatBubbleIn = Property(QColor, lambda s: s._x("bubbleIn"), notify=changed)
    chatBubbleInText = Property(QColor, lambda s: s._x("bubbleInText"), notify=changed)
    chatListSelected = Property(QColor, lambda s: s._x("listSelected"), notify=changed)
    chatListHover = Property(QColor, lambda s: s._x("listHover"), notify=changed)

    @Property(bool, notify=changed)
    def hasChat(self) -> bool:
        return bool(self._preset.get("extras"))

    @Property(bool, notify=changed)
    def isDark(self) -> bool:
        mode = self._settings.get_py("theme_mode")
        if mode == "dark":
            return True
        if mode == "light":
            return False
        return is_dark(self._preset["colors"]["background"])

    @Property(str, notify=changed)
    def presetName(self) -> str:
        return self._preset["name"]

    @Property(float, notify=changed)
    def uiScale(self) -> float:
        return int(self._settings.get_py("ui_font_scale")) / 100

    @Property(float, notify=changed)
    def chatScale(self) -> float:
        return int(self._settings.get_py("chat_font_scale")) / 100

    @Property(float, notify=changed)
    def rem(self) -> float:
        return 16 * int(self._settings.get_py("ui_font_scale")) / 100

    @Property(float, notify=changed)
    def transparency(self) -> float:
        return int(self._settings.get_py("window_transparency")) / 100

    @Property(int, constant=True)
    def fastMs(self) -> int:
        return 140

    @Property(int, constant=True)
    def baseMs(self) -> int:
        return 220

    @Property(str, constant=True)
    def fontMono(self) -> str:
        return "JetBrains Mono"

    @Property(str, constant=True)
    def fontEmoji(self) -> str:
        return "Noto Color Emoji"

    # -- helpers ---------------------------------------------------------------------
    @Slot(QColor, float, result=QColor)
    def alpha(self, color: QColor, a: float) -> QColor:
        c = QColor(color)
        c.setAlphaF(max(0.0, min(1.0, a)))
        return c

    @Slot(QColor, QColor, float, result=QColor)
    def mix(self, a: QColor, b: QColor, t: float) -> QColor:
        return QColor.fromRgbF(
            a.redF() + (b.redF() - a.redF()) * t, a.greenF() + (b.greenF() - a.greenF()) * t,
            a.blueF() + (b.blueF() - a.blueF()) * t, a.alphaF() + (b.alphaF() - a.alphaF()) * t,
        )

    @Slot(QColor, result=QColor)
    def glass(self, color: QColor) -> QColor:
        """GPUI `glass_fill`: the colour at (1 - window transparency) opacity."""
        return self.alpha(color, 1 - self.transparency)

    # -- settings -----------------------------------------------------------------
    @Slot(result="QVariantList")
    def presets(self) -> list[dict[str, Any]]:
        return [*self._extra.values(), *self._presets]

    @Slot(str)
    def setPreset(self, name: str) -> None:
        if self.find(name):
            self._settings.set("theme_preset", name)

    @Slot(str)
    def setMode(self, mode: str) -> None:
        if mode in ("system", "light", "dark"):
            self._settings.set("theme_mode", mode)

    @Slot(int, bool)
    def setUiScale(self, percent: int, persist: bool) -> None:
        self._set_num("ui_font_scale", percent, persist, MIN_FONT_SCALE, MAX_FONT_SCALE)

    @Slot(int, bool)
    def setChatScale(self, percent: int, persist: bool) -> None:
        self._set_num("chat_font_scale", percent, persist, MIN_FONT_SCALE, MAX_FONT_SCALE)

    @Slot(int, bool)
    def setTransparency(self, percent: int, persist: bool) -> None:
        self._set_num("window_transparency", percent, persist, 0, 100)

    def _set_num(self, key: str, v: int, persist: bool, lo: int, hi: int) -> None:
        v = max(lo, min(hi, v))
        (self._settings.set if persist else self._settings.set_transient)(key, v)
