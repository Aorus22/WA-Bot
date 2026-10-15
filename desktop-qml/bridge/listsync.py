"""Keep a DictListModel in step with a freshly fetched list without resetting the view
(a ListView keeps its scroll position when only changed rows are patched)."""
from __future__ import annotations

from typing import Any


def sync_model(model, rows: list[dict[str, Any]], key: str = "id") -> None:
    cur = [r.get(key) for r in model.rows()]
    new = [r.get(key) for r in rows]
    if cur != new or len(set(new)) != len(new):
        model.reset(rows)
        return
    for r in rows:
        old = model.get_row(r.get(key)) or {}
        changes = {k: v for k, v in r.items() if old.get(k) != v}
        if changes:
            model.patch(r.get(key), changes)


def error_text(status: int, body: Any, fallback: str = "request failed") -> str:
    """Message from a backend error body ({"error": "..."}), mirroring GPUI `handle_response`."""
    if isinstance(body, dict) and body.get("error"):
        return str(body["error"])
    if isinstance(body, str) and body.strip():
        return body.strip()
    return f"HTTP {status}" if status else fallback


def ok(status: int) -> bool:
    return 200 <= status < 300


# --- time labels (GPUI `localtime` + `format_status_time` / `format_post_date`) ---------------
import time as _time
from datetime import datetime as _datetime


def to_secs(ts: int | float) -> int:
    """Seconds or milliseconds since the epoch -> seconds."""
    ts = int(ts or 0)
    return ts // 1000 if ts > 10_000_000_000 else ts


def to_ms(ts: int | float) -> int:
    return to_secs(ts) * 1000


def hhmm(ts: int | float) -> str:
    return _datetime.fromtimestamp(to_secs(ts)).strftime("%H:%M")


def day_diff(ts: int | float, now: float | None = None) -> int:
    d = _datetime.fromtimestamp(to_secs(ts)).date()
    today = _datetime.fromtimestamp(now if now is not None else _time.time()).date()
    return max((today - d).days, 0)


def status_time(ts: int | float, now: float | None = None) -> str:
    """"Today at 14:05" / "Yesterday at 14:05" / "Recent at 14:05" (GPUI wording)."""
    if not ts or int(ts) <= 0:
        return ""
    diff = day_diff(ts, now)
    word = "Today" if diff == 0 else "Yesterday" if diff == 1 else "Recent"
    return f"{word} at {hhmm(ts)}"


def post_time(ts: int | float, now: float | None = None) -> str:
    """"Today at 14:05" / "Yesterday at …" / "Earlier at …" (GPUI channels)."""
    if not ts or int(ts) <= 0:
        return ""
    diff = day_diff(ts, now)
    word = "Today" if diff == 0 else "Yesterday" if diff == 1 else "Earlier"
    return f"{word} at {hhmm(ts)}"


def grouped(n: int | float) -> str:
    """1234567 -> "1,234,567" (web `toLocaleString`)."""
    return f"{int(n or 0):,}"


AVATAR_COLORS = ["#00a884", "#0284c7", "#6366f1", "#8b5cf6", "#ec4899", "#f59e0b", "#10b981", "#14b8a6"]


def avatar_color(ident: str) -> str:
    return AVATAR_COLORS[sum(ident.encode()) % len(AVATAR_COLORS)]


def initial(name: str) -> str:
    return name[:1].upper() if name else "?"
