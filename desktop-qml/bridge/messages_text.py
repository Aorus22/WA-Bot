"""Pure helpers for the message list: WhatsApp text formatting, links, mentions, times, polls...
No Qt in here so everything is unit-testable (tests/test_messages.py). Ports of GPUI `text_format.rs`,
`components/{message_bubble,rich_bubbles,mention_text}.rs`, `views/chat/helpers.rs` and
`messages/{disappearing,star_pin}.rs`."""
from __future__ import annotations

import base64
import binascii
import re
import time
from datetime import datetime
from typing import Any

# ---------------------------------------------------------------------------- time

MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September",
          "October", "November", "December"]
MONTHS_SHORT = [m[:3] for m in MONTHS]


def to_secs(ts: int | float) -> int:
    """Seconds or milliseconds since the epoch -> seconds (GPUI `localtime::to_secs`)."""
    ts = int(ts or 0)
    return ts // 1000 if ts > 10_000_000_000 else ts


def _local(ts: int | float) -> datetime:
    try:
        return datetime.fromtimestamp(to_secs(ts))
    except (OverflowError, OSError, ValueError):
        return datetime.fromtimestamp(0)


def local_day(ts: int | float) -> int:
    """Local days since the epoch (the date separator bucket)."""
    d = _local(ts)
    return datetime(d.year, d.month, d.day).toordinal()


def format_time(ts: int | float) -> str:
    """GPUI `format_chat_time`: `08:05 PM`."""
    if not ts:
        return ""
    d = _local(ts)
    h12 = 12 if d.hour % 12 == 0 else d.hour % 12
    return f"{h12:02d}:{d.minute:02d} {'PM' if d.hour >= 12 else 'AM'}"


def format_hm(ts: int | float) -> str:
    """24 h `HH:MM` (search results, receipts)."""
    if not ts:
        return ""
    d = _local(ts)
    return f"{d.hour:02d}:{d.minute:02d}"


def format_date_label(ts: int | float, now: int | float | None = None) -> str:
    """Web's `formatDate`: `Today`, `Yesterday` or `7 September 2026`."""
    now = time.time() if now is None else now
    days, today = local_day(ts), local_day(now)
    if days == today:
        return "Today"
    if days == today - 1:
        return "Yesterday"
    d = _local(ts)
    return f"{d.day} {MONTHS[d.month - 1]} {d.year}"


def format_search_date(ts: int | float) -> str:
    """`10 Sep 2026` (GPUI `format_search_date`)."""
    if not ts:
        return ""
    d = _local(ts)
    return f"{d.day} {MONTHS_SHORT[d.month - 1]} {d.year}"


def format_receipt_time(ts: int | float, now: int | float | None = None) -> str:
    """`Today at 22:03`, `Yesterday at 22:03`, `7 September 2026 at 22:03`."""
    return f"{format_date_label(ts, now)} at {format_hm(ts)}"


def format_full(ts: int | float) -> str:
    """Edit-history timestamp: `10 Sep 2026, 08:05 PM`."""
    return f"{format_search_date(ts)}, {format_time(ts)}" if ts else ""


# ---------------------------------------------------------------------------- content

def is_markdown(content: str) -> bool:
    t = (content or "").strip()
    return t.startswith("{{md:") and t.endswith("}}")


def decode_content(content: str) -> str:
    """`{{md:<base64>}}` -> the markdown source; anything else unchanged."""
    t = (content or "").strip()
    if t.startswith("{{md:") and t.endswith("}}"):
        inner = t[5:-2]
        try:
            return base64.b64decode(inner + "=" * (-len(inner) % 4), validate=False).decode("utf-8")
        except (binascii.Error, UnicodeDecodeError, ValueError):
            return inner
    return content or ""


PLACEHOLDERS = {
    "[Image]", "[Video]", "[Sticker]", "[Document]", "[Audio]", "[Voice Message]", "[GIF]",
    "[Lokasi]", "[Lokasi Langsung]", "[Kontak]", "[Sekali Lihat]", "[Poll]", "[Location]", "[Contact]",
}
DOC_EXTS = {"pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv", "rtf", "odt", "ods", "zip",
            "rar", "7z", "json", "xml", "apk", "epub"}
MEDIA_TYPES = {"image", "video", "sticker", "document", "audio", "ptt", "voice", "gif"}
AUDIO_TYPES = {"audio", "ptt", "voice"}


def looks_like_document_name(content: str) -> bool:
    name = (content or "").strip()
    if not name or len(name.split()) != 1 or "." not in name:
        return False
    return name.rsplit(".", 1)[1].lower() in DOC_EXTS


def message_kind(msg: dict) -> str:
    """text | image | video | audio | document | sticker | gif | poll | location | contact.
    The declared `type` wins; GPUI's placeholder heuristics only fill in when it is missing."""
    t = (msg.get("type") or "").lower()
    if t in AUDIO_TYPES:
        return "audio"
    if t in ("image", "video", "sticker", "document", "gif", "poll", "location", "contact"):
        return t
    media = msg.get("mediaUrl")
    content = msg.get("content") or ""
    if media:
        if content in ("[Image]", ""):
            return "image"
        if content == "[Sticker]":
            return "sticker"
        if content == "[Video]":
            return "video"
        if content == "[GIF]":
            return "gif"
        if content in ("[Audio]", "[Voice Message]"):
            return "audio"
        if looks_like_document_name(content):
            return "document"
    return "text"


def quote_media_label(msg: dict) -> str:
    """What a quoted picture/voice note/file says instead of its (empty) text."""
    name = (msg.get("content") or "").strip()
    t = msg.get("type") or ""
    return {
        "image": "📷 Photo", "video": "🎥 Video", "gif": "GIF", "sticker": "Sticker",
        "audio": "🎤 Voice message", "ptt": "🎤 Voice message", "voice": "🎤 Voice message",
        "location": "📍 Location", "contact": "👤 Contact", "poll": "📊 Poll",
    }.get(t) or (
        (f"📄 {name}" if name and not name.startswith("[") else "📄 Document") if t == "document" else "Message")


def first_line(text: str) -> str:
    for line in (text or "").splitlines():
        if line.strip():
            return line.strip()
    return ""


def snippet_of(msg: dict) -> str:
    decoded = decode_content(msg.get("content") or "")
    line = first_line(decoded)
    return line or f"[{msg.get('type') or 'Message'}]"


def user_of(jid: str) -> str:
    return (jid or "").split("@", 1)[0]


def sender_name(msg: dict) -> str:
    n = (msg.get("senderName") or "").strip()
    return n or user_of(msg.get("from") or "")


# ---------------------------------------------------------------------------- WhatsApp text formatting

class Style:
    __slots__ = ("bold", "italic", "strike", "mono")

    def __init__(self, bold=False, italic=False, strike=False, mono=False):
        self.bold, self.italic, self.strike, self.mono = bold, italic, strike, mono

    def with_(self, marker: str) -> "Style":
        s = Style(self.bold, self.italic, self.strike, self.mono)
        if marker == "*":
            s.bold = True
        elif marker == "_":
            s.italic = True
        elif marker == "~":
            s.strike = True
        else:
            s.mono = True
        return s

    def plain(self) -> bool:
        return not (self.bold or self.italic or self.strike or self.mono)

    def key(self) -> tuple[bool, bool, bool, bool]:
        return (self.bold, self.italic, self.strike, self.mono)

    def __eq__(self, other):
        return isinstance(other, Style) and self.key() == other.key()

    def __repr__(self):
        return f"Style{self.key()}"


class Formatted:
    """The text with markers removed, the styled ranges of it, and a source->result offset map."""

    def __init__(self, source: str):
        self.text_parts: list[str] = []
        self.length = 0
        self.spans: list[list[Any]] = []  # [start, end, Style]
        self.offsets: list[int] = [0] * (len(source) + 1)

    @property
    def text(self) -> str:
        return "".join(self.text_parts)

    def map(self, source_offset: int) -> int:
        if 0 <= source_offset < len(self.offsets):
            return self.offsets[source_offset]
        return self.length

    def map_range(self, start: int, end: int) -> tuple[int, int]:
        return self.map(start), self.map(end)


def _word_char(c: str | None) -> bool:
    return bool(c) and c.isalnum()


def _emit(out: Formatted, source: str, a: int, b: int, style: Style) -> None:
    start = out.length
    for k in range(a, b):
        out.offsets[k] = start
    chunk = source[a:b]
    out.text_parts.append(chunk)
    out.length += len(chunk)
    if out.length == start or style.plain():
        return
    if out.spans and out.spans[-1][1] == start and out.spans[-1][2] == style:
        out.spans[-1][1] = out.length
    else:
        out.spans.append([start, out.length, style])


def _skip(out: Formatted, a: int, b: int) -> None:
    for k in range(a, b):
        out.offsets[k] = out.length


def _find_close(source: str, marker: str, opened: int, end: int) -> int | None:
    inner = opened + 1
    for at in range(inner, end):
        c = source[at]
        if c == "\n":
            return None
        nxt = at + 1
        if (c == marker and at > inner and not source[at - 1].isspace()
                and not _word_char(source[nxt] if nxt < end else None)):
            return at
    return None


def _parse_range(source: str, a: int, end: int, style: Style, out: Formatted) -> None:
    i = a
    while i < end:
        c = source[i]
        if not style.mono and source.startswith("```", i, end):
            close = source.find("```", i + 3, end)
            if close != -1 and close > i + 3:
                _skip(out, i, i + 3)
                _emit(out, source, i + 3, close, style.with_("`"))
                _skip(out, close, close + 3)
                i = close + 3
                continue
        opens = (c in "*_~`" and not (c == "`" and style.mono)
                 and not _word_char(source[i - 1] if i > 0 else None)
                 and i + 1 < end and not source[i + 1].isspace() and source[i + 1] != c)
        if opens:
            close = _find_close(source, c, i, end)
            if close is not None:
                _skip(out, i, i + 1)
                inner = style.with_(c)
                if c == "`":
                    _emit(out, source, i + 1, close, inner)
                else:
                    _parse_range(source, i + 1, close, inner, out)
                _skip(out, close, close + 1)
                i = close + 1
                continue
        _emit(out, source, i, i + 1, style)
        i += 1


def parse_format(source: str) -> Formatted:
    out = Formatted(source)
    _parse_range(source, 0, len(source), Style(), out)
    out.offsets[len(source)] = out.length
    return out


# ---------------------------------------------------------------------------- links

_SCHEMES = ("https://", "http://", "www.")


def find_links(text: str) -> list[tuple[int, int]]:
    """Ranges of the web addresses in `text` (GPUI `find_links`)."""
    links: list[tuple[int, int]] = []
    frm = 0
    n = len(text)
    while frm < n:
        found = [p for p in (text.find(s, frm) for s in _SCHEMES) if p != -1]
        if not found:
            break
        start = min(found)
        begins_word = start == 0 or not text[start - 1].isalnum()
        m = re.search(r"\s", text[start:])
        end = start + m.start() if m else n
        while end > start:
            last = text[end - 1]
            seg = text[start:end]
            unbalanced = ((last == ")" and seg.count("(") < seg.count(")"))
                          or (last == "]" and seg.count("[") < seg.count("]")))
            if last in ".,;:!?'\">}" or unbalanced:
                end -= 1
            else:
                break
        body = text[start:end]
        host = body.split("://")[-1]
        if begins_word and len(host) > 4 and "." in host:
            links.append((start, end))
        frm = max(end, start + 1)
    return links


def link_target(text: str) -> str:
    return f"https://{text}" if text.startswith("www.") else text


# ---------------------------------------------------------------------------- mentions

def render_mentions(content: str, extra: dict | None, participants: list[dict] | None = None
                    ) -> tuple[str, list[tuple[int, int, str]]]:
    """Rewrites `@<id>` tokens to names; returns the text and `(start, end, chat_jid)` spans of each mention."""
    extra = extra or {}
    mentions = extra.get("mentions") or []
    if not mentions:
        return content, []
    names: dict[str, str] = {}
    mentioned: dict[str, str] = {}
    mention_names = extra.get("mentionNames") or {}
    mention_chats = extra.get("mentionChats") or {}
    for jid in mentions:
        uid = user_of(jid)
        mentioned[uid] = mention_chats.get(jid) or jid
        label = (mention_names.get(jid) or "").strip()
        if not label and participants:
            for p in participants:
                if p.get("jid") == jid and (p.get("name") or "").strip():
                    label = p["name"]
                    break
        if label:
            names[uid] = label
    parts = content.split("@")
    rendered = parts[0]
    spans: list[tuple[int, int, str]] = []
    for part in parts[1:]:
        m = re.match(r"[0-9]*", part)
        number = m.group(0)
        start = len(rendered)
        rendered += "@" + names.get(number, number)
        if number in mentioned:
            spans.append((start, len(rendered), mentioned[number]))
        rendered += part[len(number):]
    return rendered, spans


# ---------------------------------------------------------------------------- segments

def build_segments(text: str, mention_spans: list[tuple[int, int, str]] | None = None,
                   formats: list[list[Any]] | None = None, link_detect: bool = True) -> list[dict]:
    """Split `text` into runs for rendering: {t, b, i, s, m (mono), link, mention}. QML joins them into
    rich text with theme colours."""
    mention_spans = mention_spans or []
    formats = formats or []
    n = len(text)
    if n == 0:
        return []
    cuts = {0, n}
    for a, b, _ in mention_spans:
        cuts.update((max(0, a), min(n, b)))
    for a, b, _ in formats:
        cuts.update((max(0, a), min(n, b)))
    links = find_links(text) if link_detect else []
    for a, b in links:
        cuts.update((a, b))
    points = sorted(cuts)
    segs: list[dict] = []
    for a, b in zip(points, points[1:]):
        if a >= b:
            continue
        seg = {"t": text[a:b], "b": False, "i": False, "s": False, "m": False, "link": "", "mention": ""}
        for fa, fb, st in formats:
            if fa <= a and b <= fb:
                seg["b"] |= st.bold
                seg["i"] |= st.italic
                seg["s"] |= st.strike
                seg["m"] |= st.mono
        for ma, mb, jid in mention_spans:
            if ma <= a and b <= mb:
                seg["mention"] = jid
        for la, lb in links:
            if la <= a and b <= lb and not seg["mention"]:
                seg["link"] = link_target(text[la:lb])
        if (segs and not seg["link"] and not seg["mention"] and not segs[-1]["link"] and not segs[-1]["mention"]
                and all(segs[-1][k] == seg[k] for k in "bism")):
            segs[-1]["t"] += seg["t"]
        else:
            segs.append(seg)
    return segs


def body_segments(content: str, extra: dict | None, participants: list[dict] | None, fmt: bool
                  ) -> tuple[str, list[dict], bool]:
    """(display text, segments, markdown?) of a message body the way GPUI renders it."""
    markdown = is_markdown(content)
    decoded = decode_content(content)
    text, spans = render_mentions(decoded, extra, participants)
    if fmt and not markdown:
        f = parse_format(text)
        mapped = [(*f.map_range(a, b), jid) for a, b, jid in spans]
        return f.text, build_segments(f.text, mapped, f.spans), markdown
    return text, build_segments(text, spans, None), markdown


# ---------------------------------------------------------------------------- cards

def poll_view(poll: dict | None) -> dict | None:
    """Tallies like GPUI `PollHelper`: per-option count/percent, voter total, my choices."""
    if not poll:
        return None
    votes = poll.get("votes") or {}
    counts: dict[str, int] = {}
    for opts in votes.values():
        for o in opts or []:
            counts[o] = counts.get(o, 0) + 1
    total = len(votes)
    mine = list(votes.get("me") or [])
    options = []
    for o in poll.get("options") or []:
        name = o.get("name", "") if isinstance(o, dict) else str(o)
        c = counts.get(name, 0)
        options.append({"name": name, "count": c, "percent": 0 if total == 0 else round(c * 100 / total),
                        "voted": name in mine})
    multi = bool(poll.get("multiSelect"))
    return {"question": poll.get("question", ""), "options": options, "total": total, "multi": multi,
            "mine": mine,
            "footer": f"{total} voter{'' if total == 1 else 's'}{' · select multiple' if multi else ''}"}


def toggle_vote(current: list[str], option: str, multi: bool) -> list[str]:
    if multi:
        nxt = list(current)
        if option in nxt:
            nxt.remove(option)
        else:
            nxt.append(option)
        return nxt
    return [] if option in current else [option]


def phone_of_vcard(vcard: str | None) -> str:
    for line in (vcard or "").splitlines():
        if line.upper().startswith("TEL") and ":" in line:
            num = line.split(":", 1)[1].strip()
            if num:
                return num
    return ""


def contact_view(contact: dict | None) -> dict | None:
    if not contact:
        return None
    subs = contact.get("contacts") or []
    entries = []
    if not subs:
        entries.append({"name": contact.get("displayName") or "Contact", "phone": "", "jid": ""})
    for c in subs:
        phone = phone_of_vcard(c.get("vcard"))
        digits = "".join(ch for ch in phone if ch.isdigit() and ch.isascii())
        entries.append({"name": c.get("displayName") or "Contact", "phone": phone,
                        "jid": f"{digits}@s.whatsapp.net" if len(digits) >= 6 else ""})
    return {"entries": entries}


def location_view(loc: dict | None, thumb_url: str = "") -> dict | None:
    if not loc:
        return None
    live = bool(loc.get("live"))
    lat, lng = loc.get("latitude", 0.0), loc.get("longitude", 0.0)
    title = ((loc.get("name") or "").strip() or (loc.get("address") or "").strip()
             or ("Live location" if live else "Location"))
    return {"title": title, "coords": f"{lat:.5f}, {lng:.5f}{' · live' if live else ''}",
            "mapsUrl": f"https://maps.google.com/?q={lat},{lng}", "thumb": thumb_url, "live": live}


def host_of(url: str) -> str:
    rest = (url or "").split("://", 1)[-1]
    host = re.split(r"[/?#]", rest, maxsplit=1)[0]
    return host[4:] if host.startswith("www.") else host


def status_reply_label(reply: dict) -> str:
    text = (reply.get("content") or "").strip()
    if text:
        return text
    return {"image": "Photo", "video": "Video", "audio": "Voice message"}.get(reply.get("type") or "", "Status")


# ---------------------------------------------------------------------------- disappearing / pins

DISAPPEARING_OPTIONS = [(0, "Off"), (86_400, "24 hours"), (604_800, "7 days"), (7_776_000, "90 days")]
PIN_DURATIONS = [(86_400, "24 hours"), (604_800, "7 days"), (2_592_000, "30 days")]
DEFAULT_PIN_SECS = 604_800


def disappearing_label(seconds: int) -> str:
    for secs, label in DISAPPEARING_OPTIONS:
        if secs == seconds:
            return label
    if seconds % 86_400 == 0:
        return f"{seconds // 86_400} days"
    if seconds % 3_600 == 0:
        return f"{seconds // 3_600} hours"
    return f"{-(-seconds // 60)} min"


def disappearing_note(meta: dict | None, now_ms: int) -> dict | None:
    if not meta:
        return None
    exp = int(meta.get("expiresAt") or 0)
    if exp > 0 and exp <= now_ms:
        return {"expired": True, "tip": "Disappeared on phone · kept here"}
    if exp > 0:
        when = f"{format_search_date(exp)}, {format_hm(exp)}"
    else:
        when = f"in {disappearing_label(max(0, int(meta.get('seconds') or 0)))}"
    return {"expired": False, "tip": f"Disappears {when}"}


AVATAR_COLORS = ["#00a884", "#0284c7", "#6366f1", "#8b5cf6", "#ec4899", "#f59e0b", "#10b981", "#14b8a6"]


def avatar_color(jid: str) -> str:
    return AVATAR_COLORS[sum((jid or "").encode()) % len(AVATAR_COLORS)]


def file_name_from_url(url: str, fallback: str = "file") -> str:
    path = re.split(r"[?#]", url or "", maxsplit=1)[0]
    name = path.rsplit("/", 1)[-1]
    return name or fallback


def message_lines(messages: list[dict]) -> str:
    """Copy of several messages (GPUI `message_lines`): `[dd/mm/yyyy, HH.MM] Who: text`."""
    out = []
    for m in messages:
        d = _local(m.get("timestamp") or 0)
        when = f"{d.day:02d}/{d.month:02d}/{d.year}, {d.hour:02d}.{d.minute:02d}"
        who = "You" if m.get("from") == "me" or m.get("isMine") else sender_name(m)
        content = decode_content(m.get("content") or "")
        placeholder = not content.strip() or (content.startswith("[") and content.endswith("]"))
        body = quote_media_label(m) if placeholder else content
        deleted = " (deleted for everyone)" if m.get("revokedAt") else ""
        out.append(f"[{when}] {who}: {body}{deleted}")
    return "\n".join(out)
