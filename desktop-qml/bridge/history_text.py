"""Plain-language reading of history sync / history media progress. Port of desktop-gpui
`views/history_sync.rs` and `views/history_media.rs` (same copy, same arithmetic). Pure functions
over the camelCase JSON the backend sends, so they are unit-tested without Qt."""
from __future__ import annotations

from typing import Any

# --- sync depth choices ---------------------------------------------------------------------

SYNC_DEPTHS: list[dict[str, Any]] = [
    {"id": "per50", "label": "50 / chat", "hint": "Quick: up to 50 older messages in every chat.",
     "request": {"perChat": 50, "sinceDays": 0, "all": False}},
    {"id": "per200", "label": "200 / chat", "hint": "Up to 200 older messages in every chat.",
     "request": {"perChat": 200, "sinceDays": 0, "all": False}},
    {"id": "per1000", "label": "1000 / chat",
     "hint": "Up to 1000 older messages in every chat. Takes a while on many chats.",
     "request": {"perChat": 1000, "sinceDays": 0, "all": False}},
    {"id": "days30", "label": "Last 30 days", "hint": "Every message from the last 30 days, however many that is.",
     "request": {"perChat": 0, "sinceDays": 30, "all": False}},
    {"id": "days365", "label": "Last year", "hint": "Every message from the last year. Can take a long time.",
     "request": {"perChat": 0, "sinceDays": 365, "all": False}},
    {"id": "all", "label": "Everything",
     "hint": "Everything your phone will give, back to the first message of each chat. "
             "Keep the phone online; this can take hours.",
     "request": {"perChat": 0, "sinceDays": 0, "all": True}},
]
DEFAULT_DEPTH = "per50"


def depth(depth_id: str) -> dict[str, Any]:
    return next((d for d in SYNC_DEPTHS if d["id"] == depth_id), SYNC_DEPTHS[0])


def sync_request(depth_id: str, retry_finished: bool = False, resume: bool = False) -> dict[str, Any]:
    """Body of `POST /api/history-sync`; `retryFinished`/`resume` only sent when set (like serde skip)."""
    if resume:
        return {"perChat": 0, "sinceDays": 0, "all": False, "resume": True}
    body = dict(depth(depth_id)["request"])
    if retry_finished:
        body["retryFinished"] = True
    return body


# --- generic ----------------------------------------------------------------------------------

def _n(st: dict | None, key: str) -> int:
    try:
        return int((st or {}).get(key) or 0)
    except (TypeError, ValueError):
        return 0


def _list(st: dict | None, key: str) -> list:
    v = (st or {}).get(key)
    return v if isinstance(v, list) else []


def is_active(state: str | None) -> bool:
    """True while a sync/download is running or being stopped."""
    return state in ("running", "cancelling")


def format_duration(secs: int) -> str:
    """'3 min', '1 h 20 min', '45 s'."""
    secs = max(0, int(secs))
    if secs < 60:
        return f"{secs} s"
    if secs < 3600:
        return f"{(secs + 30) // 60} min"
    mins = (secs + 30) // 60
    return f"{mins // 60} h" if mins % 60 == 0 else f"{mins // 60} h {mins % 60} min"


def format_bytes(b: int) -> str:
    """'812 B', '34.5 MB', '1.2 GB'."""
    if b < 1024:
        return f"{b} B"
    units = ["KB", "MB", "GB", "TB"]
    value, unit = b / 1024.0, 0
    while value >= 1024.0 and unit < len(units) - 1:
        value /= 1024.0
        unit += 1
    if value >= 100.0:
        return f"{int(value + 0.5)} {units[unit]}"
    return f"{value:.1f} {units[unit]}"


# --- history sync ----------------------------------------------------------------------------

def sync_state_label(state: str | None) -> str:
    return {
        "running": "Syncing", "cancelling": "Stopping…", "completed": "Done",
        "partial": "Done, with warnings", "cancelled": "Stopped", "interrupted": "Paused",
        "failed": "Failed",
    }.get(state or "", "Not run yet")


def progress_fraction(st: dict) -> float:
    total = _n(st, "chatsTotal")
    if total == 0:
        return 0.0
    return min(1.0, max(0.0, _n(st, "chatsProcessed") / total))


def in_progress_fraction(st: dict) -> float:
    """Share of chats being worked on right now (drawn after the finished part of the bar)."""
    total = _n(st, "chatsTotal")
    if total == 0 or not is_active(st.get("state")):
        return 0.0
    room = max(0, total - _n(st, "chatsProcessed"))
    return min(room, len(_list(st, "current"))) / total


def sync_elapsed_text(st: dict) -> str | None:
    if st.get("startedAt") is None:
        return None
    word = "running for" if is_active(st.get("state")) else "ran for"
    return f"{word} {format_duration(_n(st, 'elapsedMs') // 1000)}"


def sync_eta_text(st: dict) -> str | None:
    if not is_active(st.get("state")) or _n(st, "chatsTotal") == 0:
        return None
    eta = st.get("etaMs")
    return f"about {format_duration(int(eta) // 1000)} left" if eta is not None else "estimating time…"


def chats_text(st: dict) -> str | None:
    total = _n(st, "chatsTotal")
    if total <= 0:
        return None
    pct = int(progress_fraction(st) * 100.0 + 0.5)
    return f"{_n(st, 'chatsProcessed')} of {total} chats ({pct}%)"


def finished_chats_text(st: dict | None) -> str:
    n = _n(st, "chatsFinished")
    if n == 0:
        return "Chats that already reached the start of their history are skipped unless this is on."
    return f"{n} chats already reached the start of their history. They are skipped unless this is on."


def sync_paused_text(st: dict) -> str | None:
    if not st.get("resumable"):
        return None
    left = max(0, _n(st, "chatsTotal") - _n(st, "chatsProcessed"))
    remaining = f"{left} chats are left" if left > 0 else "some chats are left"
    why = {
        "interrupted": "The app or backend was restarted while syncing.",
        "cancelled": "The sync was stopped.",
    }.get(st.get("state") or "", "The sync stopped early.")
    return f"{why} Nothing imported so far is lost, and {remaining}. Resume continues where it left off."


def current_text(c: dict) -> str:
    """What a chat in progress is doing, for the 'Now:' lines."""
    name = c.get("name") if (c.get("name") or "").strip() else (c.get("chatId") or "")
    what = "asking your phone for older messages" if c.get("phase") == "asking-phone" else "importing"
    added = int(c.get("added") or 0)
    return f"{name} — {what}, +{added} so far" if added > 0 else f"{name} — {what}"


def sync_result_text(st: dict) -> str:
    parts = [f"Added {_n(st, 'messagesAdded')} messages in {_n(st, 'chatsWithNew')} chats"]
    if _n(st, "chatsExhausted") > 0:
        parts.append(f"{_n(st, 'chatsExhausted')} chats had nothing older")
    if _n(st, "chatsSkipped") > 0:
        parts.append(f"{_n(st, 'chatsSkipped')} skipped (already at their start)")
    if _n(st, "timeouts") > 0:
        parts.append(f"{_n(st, 'timeouts')} requests got no answer from the phone")
    return " · ".join(parts)


def staged_text(st: dict) -> str | None:
    if _n(st, "pendingMessages") <= 0:
        return None
    return (f"{_n(st, 'pendingMessages')} older messages in {_n(st, 'pendingChats')} chats are downloaded "
            "but not imported yet — choose a deeper option to import them.")


def sync_error_text(st: dict) -> str | None:
    errs = _list(st, "errors")
    if not errs:
        return None
    last = errs[-1]
    chat = last.get("chatId")
    msg = f"{chat}: {last.get('message', '')}" if chat else str(last.get("message", ""))
    return f"{msg} (and {len(errs) - 1} more)" if len(errs) > 1 else msg


def sync_card(st: dict | None, active_flag: bool = False) -> dict[str, Any]:
    """Everything the 'Chat history' card shows, as plain values for QML."""
    state = (st or {}).get("state") or ""
    active = is_active(state) or active_flag
    card: dict[str, Any] = {
        "state": state, "active": active, "stopping": state == "cancelling",
        "resumable": (not active) and bool((st or {}).get("resumable")),
        "finishedText": finished_chats_text(st), "detail": False,
    }
    if st is None:
        return card
    ran = st.get("startedAt") is not None
    if not active and not ran:
        return card
    fraction = progress_fraction(st)
    headline = [sync_state_label(state)]
    c = chats_text(st)
    if c:
        headline.append(c)
    timing = [t for t in (sync_elapsed_text(st), sync_eta_text(st)) if t]
    now = [f"Now: {current_text(c_)}" for c_ in _list(st, "current")[:3]] if active else []
    if active and not _list(st, "current") and _n(st, "chatsTotal") > 0:
        now = ["Now: picking the next chat…"]
    card.update(
        detail=True,
        headline=" · ".join(headline),
        showBar=active or _n(st, "chatsTotal") > 0,
        fraction=0.01 if active and fraction < 0.01 else fraction,
        working=in_progress_fraction(st),
        timing=" · ".join(timing),
        now=now,
        paused=sync_paused_text(st) or "",
        result=sync_result_text(st),
        staged=staged_text(st) or "",
        error=sync_error_text(st) or "",
    )
    return card


# --- history media ---------------------------------------------------------------------------

MEDIA_KINDS: list[dict[str, Any]] = [
    {"id": "image", "label": "Photos", "default": True},
    {"id": "sticker", "label": "Stickers", "default": True},
    {"id": "audio", "label": "Audio & voice notes", "default": True},
    {"id": "video", "label": "Videos", "default": False},
    {"id": "document", "label": "Documents", "default": False},
]


def default_kinds() -> list[str]:
    return [k["id"] for k in MEDIA_KINDS if k["default"]]


def media_state_label(state: str | None) -> str:
    return {
        "running": "Downloading", "cancelling": "Stopping…", "completed": "Done",
        "partial": "Done, with failures", "cancelled": "Stopped", "interrupted": "Paused",
        "failed": "Failed",
    }.get(state or "", "Not run yet")


def progress_shares(st: dict) -> tuple[float, float, float]:
    """Shares of the bar (downloaded, gone from the server, failed), each 0..1."""
    total = _n(st, "total")
    if total == 0:
        return (0.0, 0.0, 0.0)
    share = lambda n: min(1.0, max(0.0, n / total))  # noqa: E731
    return (share(_n(st, "done")), share(_n(st, "unavailable") + _n(st, "missing")), share(_n(st, "failed")))


def usage_text(usage: dict | None) -> str | None:
    """'Chats 62.4 MB · Media 540 MB'; None when neither size is known."""
    if not usage:
        return None
    parts = []
    if usage.get("chatBytes") is not None:
        parts.append(f"Chats {format_bytes(int(usage['chatBytes']))}")
    if usage.get("mediaBytes") is not None:
        parts.append(f"Media {format_bytes(int(usage['mediaBytes']))}")
    return " · ".join(parts) or None


def media_headline(st: dict) -> str:
    parts = [media_state_label(st.get("state"))]
    total = _n(st, "total")
    if total > 0:
        handled = _n(st, "done") + _n(st, "unavailable") + _n(st, "failed") + _n(st, "missing")
        parts.append(f"{handled} of {total} files ({(handled * 100 + total // 2) // total}%)")
    return " · ".join(parts)


def media_timing_text(st: dict) -> str | None:
    if st.get("startedAt") is None:
        return None
    active = is_active(st.get("state"))
    parts = [f"{'running for' if active else 'ran for'} {format_duration(_n(st, 'elapsedMs') // 1000)}"]
    if active:
        eta = st.get("etaMs")
        parts.append(f"about {format_duration(int(eta) // 1000)} left" if eta is not None else "estimating time…")
    return " · ".join(parts)


def media_result_text(st: dict) -> str:
    parts = [f"Downloaded {_n(st, 'done')} files ({format_bytes(_n(st, 'bytes'))})"]
    if _n(st, "recovered") > 0:
        parts.append(f"{_n(st, 'recovered')} recovered from your phone")
    if _n(st, "unavailable") > 0:
        parts.append(f"{_n(st, 'unavailable')} no longer on WhatsApp's servers")
    if _n(st, "missing") > 0:
        parts.append(f"{_n(st, 'missing')} not on your phone either")
    if _n(st, "failed") > 0:
        parts.append(f"{_n(st, 'failed')} failed")
    return " · ".join(parts)


def media_paused_text(st: dict) -> str | None:
    if not st.get("resumable"):
        return None
    why = {
        "interrupted": "The app or backend was restarted while downloading.",
        "cancelled": "The download was stopped.",
    }.get(st.get("state") or "", "The download stopped early.")
    return (f"{why} What was downloaded is kept, and {_n(st, 'remaining')} files are left. "
            "Resume continues where it left off.")


def media_error_text(st: dict) -> str | None:
    errs = _list(st, "errors")
    if not errs:
        return None
    msg = str(errs[-1].get("message", ""))
    return f"{msg} (and {len(errs) - 1} more)" if len(errs) > 1 else msg


def files_of_kind(st: dict | None, kind: str, ask_phone: bool) -> int:
    """Files of a kind a download would try: those the servers may still have, plus the expired
    ones when the phone is asked."""
    if not st:
        return 0
    available = int((st.get("available") or {}).get(kind) or 0)
    expired = int((st.get("expired") or {}).get(kind) or 0) if ask_phone else 0
    return available + expired


def chosen_text(st: dict | None, kinds: list[str], ask_phone: bool) -> str:
    if not kinds:
        return "Choose at least one kind of media."
    count = sum(files_of_kind(st, k, ask_phone) for k in kinds)
    return f"{count} files to download. Videos and documents can be large, so they are off by default."


def media_request(kinds: list[str], retry_unavailable: bool, ask_phone: bool, resume: bool) -> dict[str, Any]:
    """Body of `POST /api/history-media`; flags only sent when set."""
    if resume:
        return {"types": [], "resume": True}
    body: dict[str, Any] = {"types": list(kinds)}
    if retry_unavailable:
        body["retryUnavailable"] = True
    if ask_phone:
        body["askPhone"] = True
    return body


def media_card(st: dict | None, kinds: list[str], retry: bool, ask_phone: bool,
               usage: dict | None, active_flag: bool = False) -> dict[str, Any]:
    """Everything the 'Chat media' section shows, as plain values for QML."""
    state = (st or {}).get("state") or ""
    active = is_active(state) or active_flag
    chips = []
    for k in MEDIA_KINDS:
        label = k["label"] if st is None else f"{k['label']} · {files_of_kind(st, k['id'], ask_phone)}"
        chips.append({"id": k["id"], "label": label, "selected": k["id"] in kinds})
    card: dict[str, Any] = {
        "state": state, "active": active, "stopping": state == "cancelling",
        "resumable": (not active) and bool((st or {}).get("resumable")),
        "nothingChosen": not kinds, "chips": chips,
        "chosenText": chosen_text(st, kinds, ask_phone),
        "usage": usage_text(usage) or "", "detail": False,
        "retryUnavailable": retry, "askPhone": ask_phone,
    }
    if st is None or (not active and st.get("startedAt") is None):
        return card
    done, gone, failed = progress_shares(st)
    card.update(
        detail=True,
        headline=media_headline(st),
        showBar=active or _n(st, "total") > 0,
        done=done, gone=gone, failed=failed,
        timing=media_timing_text(st) or "",
        now=f"Now: downloading {_n(st, 'current')} files at once" if active and _n(st, "current") > 0 else "",
        paused=media_paused_text(st) or "",
        result=media_result_text(st),
        error=media_error_text(st) or "",
    )
    return card
