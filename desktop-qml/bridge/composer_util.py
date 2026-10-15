"""Pure helpers for the composer (no Qt): ports of GPUI `components/compose.rs`,
`views/chat/helpers.rs::prepare_mention_text` and `composer/link_preview.rs`.
Kept free of Qt so they are trivially unit-testable."""
from __future__ import annotations

import base64
import re

MD_PREFIX = "{{md:"
MD_SUFFIX = "}}"
POLL_MAX_OPTIONS = 12
POLL_MIN_OPTIONS = 2

AUDIO_EXT = {"mp3", "ogg", "oga", "opus", "m4a", "wav", "aac", "amr", "flac", "weba"}
VIDEO_EXT = {"mp4", "mov", "mkv", "avi", "webm", "3gp", "m4v", "mpeg", "mpg"}
AUDIO_MIME = {"wav": "audio/wav", "ogg": "audio/ogg", "oga": "audio/ogg", "opus": "audio/ogg",
              "mp3": "audio/mpeg", "m4a": "audio/mp4", "webm": "audio/webm", "weba": "audio/webm"}


# --- files / media kinds ------------------------------------------------------------
def extension_lower(name: str) -> str:
    return name.rsplit(".", 1)[1].lower() if "." in name else ""


def is_audio_file(name: str) -> bool:
    return extension_lower(name) in AUDIO_EXT


def is_video_file(name: str) -> bool:
    return extension_lower(name) in VIDEO_EXT


def media_type(kind: str, file_name: str) -> str:
    """Wire `type` for /send-media. kind: media | document | audio | gif."""
    if kind == "document":
        return "document"
    if kind == "audio":
        return "audio"
    if kind == "gif":
        return "gif"
    if is_audio_file(file_name):
        return "audio"
    if is_video_file(file_name):
        return "video"
    return "image"


def optimistic_type(mtype: str) -> str:
    return mtype if mtype in ("audio", "video", "gif", "document") else "image"


def optimistic_content(file_name: str, mtype: str) -> str:
    return {"image": "[Image]", "video": "[Video]", "audio": "[Audio]", "gif": "[GIF]"}.get(mtype, file_name)


def audio_mime(file_name: str) -> str | None:
    return AUDIO_MIME.get(extension_lower(file_name))


# --- markdown wire format -------------------------------------------------------------
def encode_markdown(text: str) -> str:
    return f"{MD_PREFIX}{base64.b64encode(text.encode()).decode()}{MD_SUFFIX}"


def decode_content(content: str) -> str:
    """Inverse of `encode_markdown` (GPUI MessageBubbleHelper::decode_content)."""
    if content.startswith(MD_PREFIX) and content.endswith(MD_SUFFIX):
        try:
            return base64.b64decode(content[len(MD_PREFIX):-len(MD_SUFFIX)]).decode()
        except (ValueError, UnicodeDecodeError):
            return content
    return content


def wire_text(text: str, markdown_mode: bool) -> str:
    return encode_markdown(text) if markdown_mode else text


# --- mentions ---------------------------------------------------------------------------
def jid_number(jid: str) -> str:
    return jid.split("@", 1)[0]


def participant_label(p: dict) -> str:
    name = (p.get("name") or "").strip()
    return name or jid_number(p.get("jid", ""))


def mention_query(text: str, is_group: bool, editing: bool) -> str | None:
    """GPUI `refresh_mention_query`: active `@query` at the end of the text, else None."""
    if editing or not is_group:
        return None
    at = text.rfind("@")
    if at < 0:
        return None
    prefix = text[:at]
    if prefix and not prefix[-1].isspace():
        return None
    query = text[at + 1:]
    if any(c.isspace() for c in query):
        return None
    return query


def mention_candidates(query: str | None, participants: list[dict], limit: int = 8) -> list[dict]:
    if query is None or not participants:
        return []
    needle = query.lower()
    out = []
    for p in participants:
        jid = p.get("jid", "")
        name = participant_label(p)
        if needle and needle not in name.lower() and needle not in jid.lower():
            continue
        out.append({"jid": jid, "name": name, "isAdmin": bool(p.get("isAdmin"))})
        if len(out) >= limit:
            break
    return out


def prepare_mention_text(text: str, participants: list[dict] | None) -> tuple[str, list[str]]:
    """Rewrite `@Name` tokens to `@<number>` and collect the mentioned JIDs (GPUI port)."""
    if not participants:
        return text, []
    content = text
    mentions: list[str] = []
    for p in sorted(participants, key=lambda p: -len((p.get("name") or ""))):
        jid = p.get("jid", "")
        token = "@" + participant_label(p)
        if token in content:
            content = content.replace(token, "@" + jid_number(jid))
            if jid not in mentions:
                mentions.append(jid)
    return content, mentions


def render_mentions(content: str, extra: dict | None, participants: list[dict] | None = None) -> tuple[str, list[list[int]]]:
    """Rewrite `@<number>` to the contact's name for display (GPUI render_message_mentions_with_spans).
    Returns (text, [[start, end], ...]) of mention ranges in characters."""
    extra = extra or {}
    mentions = extra.get("mentions") or []
    if not mentions:
        return content, []
    names: dict[str, str] = {}
    mentioned: set[str] = set()
    by_jid = {p.get("jid"): (p.get("name") or "").strip() for p in (participants or [])}
    mn = extra.get("mentionNames") or {}
    for jid in mentions:
        num = jid_number(jid)
        mentioned.add(num)
        label = (mn.get(jid) or "").strip() or by_jid.get(jid, "")
        if label:
            names[num] = label
    parts = content.split("@")
    out = parts[0]
    spans: list[list[int]] = []
    for part in parts[1:]:
        m = re.match(r"[0-9]*", part)
        number = m.group(0) if m else ""
        start = len(out)
        out += "@" + names.get(number, number)
        if number in mentioned:
            spans.append([start, len(out)])
        out += part[len(number):]
    return out, spans


# --- link preview -------------------------------------------------------------------------
def first_url(text: str) -> str | None:
    for token in text.split():
        token = token.lstrip("(<\"'")
        low = token.lower()
        if low.startswith("https://"):
            scheme = 8
        elif low.startswith("http://"):
            scheme = 7
        else:
            continue
        url = token.rstrip(".,;:!?)]}>\"'")
        rest = url[scheme:]
        host = re.split(r"[/?#]", rest, maxsplit=1)[0]
        name = host.split(":", 1)[0]
        if ("." in name and not name.endswith(".")) or name == "localhost":
            return url
    return None


def host_of(url: str) -> str:
    rest = url.split("://", 1)[1] if "://" in url else url
    host = re.split(r"[/?#]", rest, maxsplit=1)[0]
    return host[4:] if host.startswith("www.") else host


def link_card(url: str, preview: dict | None) -> dict | None:
    if not preview:
        return None
    title = (preview.get("title") or "").strip()
    desc = (preview.get("description") or "").strip()
    thumb = (preview.get("thumbnailUrl") or "").strip()
    if not (title or desc or thumb):
        return None
    return {"url": url, "title": title or host_of(url), "description": desc, "thumbnailUrl": thumb,
            "host": host_of(url)}


# --- dialogs: same fields / messages as GPUI -----------------------------------------------
def validate_poll(question: str, options: list[str], multi: bool):
    """-> (error, (question, options, multi)). Error strings are GPUI's (Indonesian)."""
    q = (question or "").strip()
    if not q:
        return "Poll perlu pertanyaan", None
    opts = [o.strip() for o in options if o and o.strip()]
    if len(opts) < POLL_MIN_OPTIONS:
        return "Poll minimal 2 opsi", None
    return "", (q, opts, bool(multi))


def validate_location(lat: str, lon: str, name: str, address: str, live: bool):
    # Rust `str::parse::<f64>` accepts "inf"/"nan" and reports them via the finite check below.
    try:
        la = float(str(lat).strip())
    except ValueError:
        return "Latitude tidak valid", None
    try:
        lo = float(str(lon).strip())
    except ValueError:
        return "Longitude tidak valid", None
    if la != la or lo != lo or abs(la) == float("inf") or abs(lo) == float("inf"):
        return "Koordinat tidak valid", None
    return "", (la, lo, (name or "").strip(), (address or "").strip(), bool(live))


def validate_contact(name: str, phone: str):
    n, p = (name or "").strip(), (phone or "").strip()
    if not n or not p:
        return "Nama dan nomor telepon wajib diisi", None
    return "", (n, p)


# --- emoji ---------------------------------------------------------------------------------
# The first three categories are GPUI's `EMOJI_CATEGORIES`; the rest are additions.
EMOJI_CATEGORIES = [
    {"name": "Smileys", "emojis": [
        "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🤣", "😊", "😇", "🙂", "🙃", "😉", "😌", "😍", "🥰",
        "😘", "😗", "😙", "😚", "😋", "😛", "😜", "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🤐", "🤨",
        "😐", "😑", "😶", "😏", "😒", "🙄", "😬", "🤥", "😔", "😪", "🤤", "😴", "😷", "🤒", "🤕", "🤢"]},
    {"name": "Gestures", "emojis": [
        "👍", "👎", "👏", "🙌", "👐", "🤲", "🤝", "🙏", "✌️", "🤞", "🤟", "🤘", "👌", "🤌", "🤏", "👈",
        "👉", "👆", "👇", "✋", "🤚", "🖐️", "🖖", "👋", "🤙", "💪", "🦾", "🖕", "✍️", "🤳"]},
    {"name": "Hearts & Sparkles", "emojis": [
        "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞", "💓", "💗", "💖",
        "💘", "💝", "💟", "🔥", "✨", "⭐", "🌟", "💫", "💥", "🎉", "🎊", "💯"]},
    {"name": "Animals", "emojis": [
        "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐸", "🐵", "🐔",
        "🐧", "🐦", "🦆", "🦉", "🐴", "🦄", "🐝", "🦋", "🐢", "🐍", "🐙", "🐬", "🐳", "🦈"]},
    {"name": "Food", "emojis": [
        "🍎", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🥭", "🍍", "🥥", "🥑", "🍅", "🌽", "🥕",
        "🍞", "🧀", "🍳", "🥞", "🍔", "🍟", "🍕", "🌭", "🌮", "🍜", "🍣", "🍩", "🍪", "🎂", "☕", "🍺"]},
    {"name": "Activities", "emojis": [
        "⚽", "🏀", "🏈", "⚾", "🎾", "🏐", "🎱", "🏓", "🎯", "🎮", "🎲", "🎸", "🎹", "🎧", "🎬", "🎨",
        "🏆", "🥇", "🚴", "🏊", "🧘", "🎳", "🎤", "🎭"]},
    {"name": "Objects", "emojis": [
        "📱", "💻", "⌨️", "🖥️", "📷", "🔋", "💡", "📌", "📎", "✏️", "📝", "📚", "🔑", "🔒", "🎁", "💰",
        "⏰", "📞", "✉️", "🔔", "🔍", "🧸", "🛒", "🚗"]},
]

# --- misc ----------------------------------------------------------------------------------
def first_line(text: str) -> str:
    for line in text.splitlines():
        if line.strip():
            return line.strip()
    return ""
