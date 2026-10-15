"""Pure helpers for the chat list / chat shell, ported from GPUI (`localtime.rs`,
`MessageBubbleHelper`, `ChatStore::typing_label`, `chat_actions.rs`). No Qt here so they are easy to test."""
from __future__ import annotations

import base64
import binascii
from datetime import datetime
from typing import Any, Iterable

MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
# Timers WhatsApp offers for a chat: seconds and label (0 = off).
DISAPPEARING_OPTIONS = [(0, "Off"), (86_400, "24 hours"), (604_800, "7 days"), (7_776_000, "90 days")]


def to_secs(ts: int | float | None) -> int:
    """Seconds or milliseconds since the epoch -> seconds."""
    ts = int(ts or 0)
    return ts // 1000 if ts > 10_000_000_000 else ts


def _local(ts: int | float | None) -> datetime:
    return datetime.fromtimestamp(to_secs(ts))


def format_chat_time(ts: int | float | None) -> str:
    """Chat-list time: `03:45 PM` (GPUI `format_chat_time`); empty for 0."""
    if not ts:
        return ""
    d = _local(ts)
    h12 = 12 if d.hour % 12 == 0 else d.hour % 12
    return f"{h12:02d}:{d.minute:02d} {'PM' if d.hour >= 12 else 'AM'}"


def format_search_date(ts: int | float | None) -> str:
    """`10 Sep 2026` (GPUI `format_search_date`)."""
    if not ts:
        return ""
    d = _local(ts)
    return f"{d.day} {MONTHS[d.month - 1]} {d.year}"


def format_search_time(ts: int | float | None) -> str:
    """`23:26` (GPUI `format_search_time`)."""
    if not ts:
        return ""
    d = _local(ts)
    return f"{d.hour:02d}:{d.minute:02d}"


def decode_content(content: str | None) -> str:
    """`{{md:<base64>}}` (markdown-mode text) -> the plain text (`MessageBubbleHelper::decode_content`)."""
    content = content or ""
    t = content.strip()
    if t.startswith("{{md:") and t.endswith("}}"):
        inner = t[5:-2]
        try:
            return base64.b64decode(inner + "=" * (-len(inner) % 4), validate=False).decode("utf-8")
        except (binascii.Error, ValueError, UnicodeDecodeError):
            return inner
    return content


def first_line(text: str | None) -> str:
    for line in (text or "").splitlines():
        if line.strip():
            return line.strip()
    return ""


def list_snippet(last_msg: str | None) -> str:
    """The chat-list preview: first non-empty line, or `Tap to chat`."""
    line = first_line(last_msg)
    return decode_content(line) if line else "Tap to chat"


def is_from_me(msg: dict[str, Any]) -> bool:
    return msg.get("from") == "me" or str(msg.get("id", "")).startswith("temp-") or msg.get("status") == "pending"


def sender_label(msg: dict[str, Any]) -> str:
    if is_from_me(msg):
        return "You"
    name = (msg.get("senderName") or "").strip()
    return name or str(msg.get("from", "")).split("@", 1)[0]


def snippet_of(msg: dict[str, Any]) -> str:
    """First non-empty line of a message, or its type in brackets when it has no text."""
    line = first_line(decode_content(msg.get("content")))
    return line or f"[{msg.get('type') or 'Message'}]"


def disappearing_label(seconds: int) -> str:
    for secs, label in DISAPPEARING_OPTIONS:
        if secs == seconds:
            return label
    if seconds % 86_400 == 0:
        return f"{seconds // 86_400} days"
    if seconds % 3_600 == 0:
        return f"{seconds // 3_600} hours"
    return f"{-(-seconds // 60)} min"


def typing_label(typers: list[tuple[str, str, bool]], is_group: bool) -> str:
    """WhatsApp's chat-list line: `typing…` in a 1:1 chat, `Name is typing…` in a group.
    `typers` = [(jid, name, recording_audio)] oldest first."""
    if not typers:
        return ""
    first, audio = typers[0][1], typers[0][2]
    verb = "recording audio…" if audio else "typing…"
    if not is_group:
        return verb
    n = len(typers)
    if n == 1:
        return f"{first} is {verb}"
    if n == 2:
        return f"{first} and {typers[1][1]} are {verb}"
    return f"{first} and {n - 1} others are {verb}"


def message_lines(messages: Iterable[dict[str, Any]]) -> str:
    """`[dd/mm/yyyy, HH.MM] sender: text`, one line per message."""
    out = []
    for m in messages:
        ts = to_secs(m.get("timestamp"))
        when = datetime.fromtimestamp(ts).strftime("%d/%m/%Y, %H.%M") if ts else ""
        who = "You" if m.get("from") == "me" else (
            (m.get("senderName") or "").strip() or str(m.get("from", "")).split("@", 1)[0])
        content = decode_content(m.get("content"))
        placeholder = not content.strip() or (content.startswith("[") and content.endswith("]"))
        body = quote_media_label(m) if placeholder else content
        deleted = " (deleted for everyone)" if m.get("revokedAt") else ""
        out.append(f"[{when}] {who}: {body}{deleted}\n")
    return "".join(out)


def export_chat_text(chat_name: str, messages: Iterable[dict[str, Any]]) -> str:
    return f"Chat with {chat_name}\n\n{message_lines(messages)}"


def quote_media_label(msg: dict[str, Any]) -> str:
    name = (msg.get("content") or "").strip()
    t = msg.get("type") or ""
    if t == "image":
        return "📷 Photo"
    if t == "video":
        return "🎥 Video"
    if t == "gif":
        return "GIF"
    if t == "sticker":
        return "Sticker"
    if t in ("audio", "ptt", "voice"):
        return "🎤 Voice message"
    if t == "document":
        return f"📄 {name}" if name and not name.startswith("[") else "📄 Document"
    return {"location": "📍 Location", "contact": "👤 Contact", "poll": "📊 Poll"}.get(t, "Message")


def safe_file_name(chat_name: str) -> str:
    safe = "".join(c if (c.isalnum() or c in " -_") else "_" for c in chat_name)
    return f"WhatsApp chat - {safe.strip()}.txt"
