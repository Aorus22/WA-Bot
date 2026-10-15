"""Desktop UI preferences. Shares `~/.config/wa-bot-desktop/settings.json` with the
GPUI client (same keys), read-modify-write so unknown keys survive. UI preferences
only — server-owned settings live in the backend (`/api/settings`)."""
from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, Signal, Slot

DEFAULTS: dict[str, Any] = {
    "theme_preset": "default-dark",
    "theme_mode": "system",
    "chat_layout": "auto",
    "backend_path_override": None,
    "last_backend_url": None,
    "window_state": None,
    "settings_version": 1,
    "window_transparency": 28,
    "window_blur_enabled": True,
    "ui_font_scale": 100,
    "chat_font_scale": 100,
    "notifications_enabled": True,
    "format_messages": True,
}
MIN_FONT_SCALE, MAX_FONT_SCALE = 75, 150


def settings_path() -> Path:
    base = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return base / "wa-bot-desktop" / "settings.json"


class Settings(QObject):
    """`Settings.get("key")` / `Settings.set("key", v)`; `changed(key)` fires on writes."""
    changed = Signal(str)

    def __init__(self, path: Path | None = None, parent: QObject | None = None):
        super().__init__(parent)
        self._path = path or settings_path()
        self._data: dict[str, Any] = {}
        self._transient: set[str] = set()  # keys previewed but not yet written to disk
        self.reload()

    def reload(self) -> None:
        try:
            raw = json.loads(self._path.read_text())
            self._data = raw if isinstance(raw, dict) else {}
        except (OSError, ValueError):
            self._data = {}
        d = self._data
        try:
            legacy = int(d.get("settings_version") or 0) < 1
        except (TypeError, ValueError):
            legacy = True
        # GPUI schema v1 migration: the old slider stopped at 70, so a saved 70 meant "dragged to the end".
        if legacy and d.get("window_transparency") == 70:
            d["window_transparency"] = 100
        for k, v in DEFAULTS.items():
            d.setdefault(k, v)
        d["settings_version"] = max(1, d["settings_version"]) if isinstance(d["settings_version"], int) else 1
        for k in ("ui_font_scale", "chat_font_scale", "window_transparency"):
            try:
                d[k] = int(d[k])
            except (TypeError, ValueError):
                d[k] = DEFAULTS[k]
        d["ui_font_scale"] = max(MIN_FONT_SCALE, min(MAX_FONT_SCALE, int(d["ui_font_scale"])))
        d["chat_font_scale"] = max(MIN_FONT_SCALE, min(MAX_FONT_SCALE, int(d["chat_font_scale"])))
        d["window_transparency"] = max(0, min(100, int(d["window_transparency"])))

    def get_py(self, key: str, default: Any = None) -> Any:
        return self._data.get(key, default)

    @Slot(str, result="QVariant")
    def get(self, key: str):
        return self._data.get(key)

    @Slot(str, "QVariant")
    def set(self, key: str, value) -> None:
        if self._data.get(key) == value:
            if key in self._transient:  # value was only previewed: persist it now
                self._transient.discard(key)
                self.save()
            return
        self._data[key] = value
        self._transient.discard(key)
        self.changed.emit(key)
        self.save()

    def set_transient(self, key: str, value) -> None:
        """Change without writing to disk (live preview of sliders)."""
        self._data[key] = value
        self._transient.add(key)
        self.changed.emit(key)

    def save(self) -> None:
        """Merge into whatever is on disk so a concurrent GPUI write of other keys survives."""
        try:
            self._path.parent.mkdir(parents=True, exist_ok=True)
            try:
                disk = json.loads(self._path.read_text())
                disk = disk if isinstance(disk, dict) else {}
            except (OSError, ValueError):
                disk = {}
            disk.update(self._data)
            tmp = self._path.with_suffix(".json.tmp")
            tmp.write_text(json.dumps(disk, indent=2))
            os.replace(tmp, self._path)
        except OSError as e:
            print(f"[settings] could not save {self._path}: {e}")
