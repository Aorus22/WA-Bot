"""Pure, Qt-free logic of the desktop-integration layer (port of GPUI `notifications.rs`,
`popover.rs` wire format and `settings::WindowState`). Everything here is unit-testable
without a display or a bus; `bridge/platform.py` wires it to Qt."""
from __future__ import annotations

import base64
import math
import os
import re
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

# --- command line -----------------------------------------------------------------

STANDALONE_FLAGS = ("--url", "--backend-url", "--port", "-p")


@dataclass(frozen=True)
class Flags:
    toggle: bool = False
    popover: bool = False       # --popover / --background / --toggle: resident, panel-card style
    standalone: bool = False    # explicit backend: a development run, never folded into the resident one

    @property
    def signal(self) -> str:
        return "toggle" if self.toggle else "open"


def parse_flags(argv: list[str]) -> Flags:
    """`argv` without the program name. Same rules as GPUI `main()`."""
    toggle = "--toggle" in argv
    popover = toggle or any(a in ("--popover", "--background") for a in argv)
    standalone = any(
        a in STANDALONE_FLAGS or a.startswith("--url=") or a.startswith("--port=") for a in argv
    )
    return Flags(toggle, popover, standalone)


# --- notification policy (GPUI `notifications::plan`) -----------------------------------

MAX_AGE_MS = 5 * 60 * 1000
MAX_BODY_CHARS = 140


@dataclass(frozen=True)
class Notice:
    title: str
    body: str
    avatar: str | None = None


def now_ms() -> int:
    return int(time.time() * 1000)


def is_muted(chat: dict, now: int) -> bool:
    mode = chat.get("muteMode") or ""
    if mode in ("", "off"):
        return False
    if mode == "until":
        end = chat.get("mutedUntil")
        return True if end is None else _to_ms(end) > now
    return True


def media_label(message_type: str) -> str:
    return {
        "image": "\U0001F4F7 Photo",
        "video": "\U0001F3A5 Video",
        "audio": "\U0001F3A4 Voice message", "ptt": "\U0001F3A4 Voice message", "voice": "\U0001F3A4 Voice message",
        "document": "\U0001F4C4 Document",
        "sticker": "Sticker",
        "location": "\U0001F4CD Location",
        "contact": "\U0001F464 Contact",
        "poll": "\U0001F4CA Poll",
    }.get(message_type, "New message")


def truncate(text: str, limit: int = MAX_BODY_CHARS) -> str:
    return text[:limit] + "…" if len(text) > limit else text


def decode_content(content: str) -> str:
    """`{{md:<base64>}}` wraps formatted text (GPUI `MessageBubbleHelper::decode_content`)."""
    t = content.strip()
    if t.startswith("{{md:") and t.endswith("}}"):
        inner = t[5:-2]
        try:
            return base64.b64decode(inner + "=" * (-len(inner) % 4)).decode("utf-8")
        except (ValueError, UnicodeDecodeError):
            return inner
    return content


def render_mentions(content: str, message: dict) -> str:
    """Replace `@<number>` by the names the backend resolved (GPUI `render_message_mentions_with_spans`)."""
    extra = message.get("extra") or {}
    mentions = extra.get("mentions") or []
    if not mentions:
        return content
    names = {}
    for jid in mentions:
        label = (extra.get("mentionNames") or {}).get(jid)
        if label and label.strip():
            names[jid.split("@", 1)[0]] = label
    parts = content.split("@")
    out = [parts[0]]
    for part in parts[1:]:
        m = re.match(r"[0-9]*", part)  # ASCII digits only, like the Rust helper
        number = m.group(0) if m else ""
        out.append("@" + names.get(number, number) + part[len(number):])
    return "".join(out)


def is_from_me(msg: dict) -> bool:
    return (msg.get("from") == "me" or str(msg.get("id", "")).startswith("temp-")
            or msg.get("status") == "pending")


def plan(msg: dict, chat: dict | None, now: int) -> Notice | None:
    """Decide whether `msg` deserves a notification and build its text.
    `chat` is the stored chat row for `msg["chatId"]`, if known."""
    if is_from_me(msg) or msg.get("isAutomatic") is True or msg.get("notificationSuppressed"):
        return None
    ts = msg.get("timestamp") or 0
    if ts > 0 and now - _to_ms(ts) > MAX_AGE_MS:
        return None
    if chat and (chat.get("archived") or is_muted(chat, now)):
        return None
    chat_id = msg.get("chatId") or ""
    is_group = bool(chat.get("isGroup")) if chat else chat_id.endswith("@g.us")
    is_group = is_group or chat_id.endswith("@g.us")
    sender = (msg.get("senderName") or "") or None
    name = (chat.get("name") if chat else "") or ""
    title = name or sender or chat_id
    decoded = render_mentions(decode_content(msg.get("content") or ""), msg)
    first = next((ln.strip() for ln in decoded.split("\n") if ln.strip()), None)
    if first is not None and not (first.startswith("[") and first.endswith("]")):
        text = truncate(first)
    else:
        text = media_label(msg.get("type") or "")
    body = f"{sender}: {text}" if is_group and sender else text
    avatar = ((chat.get("avatar") or "").strip() if chat else "") or None
    return Notice(title, body, avatar)


def _to_ms(ts: int | float) -> int:
    """The backend sends unix ms; tolerate seconds (as the UI does)."""
    return int(ts * 1000) if ts < 100_000_000_000 else int(ts)


def reading_chat(chat_id: str, chat_visible: bool, selected: str | None) -> bool:
    return chat_visible and selected == chat_id


# --- Rust `DefaultHasher` (SipHash-1-3, zero keys) so receipts/avatars share GPUI's files ---

_M = 0xFFFFFFFFFFFFFFFF


def _rotl(x: int, b: int) -> int:
    return ((x << b) | (x >> (64 - b))) & _M


def siphash13(data: bytes, k0: int = 0, k1: int = 0) -> int:
    v0 = k0 ^ 0x736F6D6570736575
    v1 = k1 ^ 0x646F72616E646F6D
    v2 = k0 ^ 0x6C7967656E657261
    v3 = k1 ^ 0x7465646279746573

    def rnd() -> None:
        nonlocal v0, v1, v2, v3
        v0 = (v0 + v1) & _M; v1 = _rotl(v1, 13); v1 ^= v0; v0 = _rotl(v0, 32)
        v2 = (v2 + v3) & _M; v3 = _rotl(v3, 16); v3 ^= v2
        v0 = (v0 + v3) & _M; v3 = _rotl(v3, 21); v3 ^= v0
        v2 = (v2 + v1) & _M; v1 = _rotl(v1, 17); v1 ^= v2; v2 = _rotl(v2, 32)

    n = len(data)
    for i in range(0, n - n % 8, 8):
        m = int.from_bytes(data[i:i + 8], "little")
        v3 ^= m
        rnd()
        v0 ^= m
    tail = data[n - n % 8:]
    b = ((n & 0xFF) << 56) | int.from_bytes(tail, "little")
    v3 ^= b
    rnd()
    v0 ^= b
    v2 ^= 0xFF
    for _ in range(3):
        rnd()
    return v0 ^ v1 ^ v2 ^ v3


def rust_hash_str(s: str) -> int:
    """`s.hash(&mut DefaultHasher::new())` then `finish()`: bytes followed by 0xff."""
    return siphash13(s.encode() + b"\xff")


def claim_notice(directory: Path, key: str) -> Path | None:
    """First caller (any process, GPUI or QML) wins one notification per message.
    Returns the receipt path, or None when somebody else already claimed it."""
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"{rust_hash_str(key):016x}"
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    except FileExistsError:
        return None
    os.close(fd)
    return path


def notice_key(base_url: str, chat_id: str, message_id: str) -> str:
    return f"{base_url}\0{chat_id}\0{message_id}"


def avatar_cache_path(directory: Path, url: str) -> Path:
    return directory / f"{rust_hash_str(url):016x}.img"


AVATAR_CACHE_AGE = 3600
MAX_AVATAR_BYTES = 2 * 1024 * 1024


def is_image_bytes(b: bytes) -> bool:
    """Reject redirects to HTML/error pages and unsupported formats (same list as GPUI)."""
    return (b.startswith(b"\xff\xd8\xff") or b.startswith(b"\x89PNG\r\n\x1a\n")
            or (b.startswith(b"RIFF") and b[8:12] == b"WEBP"))


def fresh_cached(path: Path, now: float | None = None) -> bool:
    try:
        st = path.stat()
    except OSError:
        return False
    return st.st_size > 0 and (now or time.time()) - st.st_mtime < AVATAR_CACHE_AGE


def write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.replace(tmp, path)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise


# --- window state (settings `window_state`, shared with GPUI) -------------------------

MIN_WIDTH, MIN_HEIGHT = 800, 540
DEFAULT_WIDTH, DEFAULT_HEIGHT = 1100, 720


def normalize_window_state(ws: object) -> dict | None:
    """GPUI `WindowState::normalized`: None for unusable bounds (negative origin, zero size)."""
    if not isinstance(ws, dict):
        return None
    x, y, w, h = (ws.get(k) for k in ("x", "y", "width", "height"))
    for v in (x, y, w, h):
        if v is not None and (isinstance(v, bool) or not isinstance(v, int)):
            return None
    if (x is not None and x < 0) or (y is not None and y < 0) or w == 0 or h == 0:
        return None
    return {"x": x, "y": y, "width": w, "height": h, "maximized": bool(ws.get("maximized"))}


def restore_geometry(ws: object, screen: tuple[int, int, int, int] | None = None) -> dict:
    """-> {x, y, width, height, maximized}; x/y None when the platform will place it.
    Sizes are clamped to the window minimum and (when known) the available screen rect
    `(left, top, width, height)`; a position that would be off-screen is dropped."""
    n = normalize_window_state(ws)
    if n is None:
        return {"x": None, "y": None, "width": DEFAULT_WIDTH, "height": DEFAULT_HEIGHT, "maximized": False}
    w = max(MIN_WIDTH, n["width"] or DEFAULT_WIDTH)
    h = max(MIN_HEIGHT, n["height"] or DEFAULT_HEIGHT)
    x, y = n["x"], n["y"]
    if screen:
        sl, st, sw, sh = screen
        w, h = min(w, max(MIN_WIDTH, sw)), min(h, max(MIN_HEIGHT, sh))
        if x is not None and y is not None and not (sl <= x <= sl + sw - 80 and st <= y <= st + sh - 80):
            x = y = None
    if x is None or y is None:
        x = y = None
    return {"x": x, "y": y, "width": w, "height": h, "maximized": n["maximized"]}


def window_state_to_save(x: int | None, y: int | None, w: int, h: int, maximized: bool, previous: object) -> dict | None:
    """What to store. While maximized the normal-size rect is kept from `previous`, as the
    maximized geometry is meaningless to restore. Negative origins are stored as absent so
    GPUI's `normalized()` still accepts the record. Returns None when there is nothing usable."""
    prev = normalize_window_state(previous) or {}
    if maximized:
        if not prev:
            return {"x": None, "y": None, "width": None, "height": None, "maximized": True}
        return {**prev, "maximized": True}
    if w <= 0 or h <= 0:
        return None
    if x is not None and (x < 0 or y is None or y < 0):
        x = y = None
    return {"x": x, "y": y, "width": int(w), "height": int(h), "maximized": False}


# --- popover placement (GPUI `popover::card_layer_options`) ------------------------------

POPOVER_WIDTH, POPOVER_HEIGHT = 880, 620
PANEL_GAP = 4
SCREEN_MARGIN = 8
CARD_RADIUS = 14


@dataclass(frozen=True)
class Placement:
    edge: str = "top"                                   # top | bottom | left | right
    pointer: tuple[float, float] | None = None
    screen: tuple[float, float] = (1920.0, 1080.0)


def edge_from_env(env: dict[str, str] | None = None) -> str:
    v = (env if env is not None else os.environ).get("POPOVER_ANCHOR", "")
    return v if v in ("bottom", "left", "right") else "top"


def card_margins(p: Placement) -> tuple[tuple[str, ...], tuple[int, int, int, int]]:
    """-> (anchors, (top, right, bottom, left) margins) exactly as GPUI: anchored to the
    screen corner nearest the panel button, then pushed along the edge so it sits under it."""
    sw, sh = p.screen

    def along(pointer: float | None, extent: float, card: float) -> float:
        hi = max(extent - card - SCREEN_MARGIN, SCREEN_MARGIN)
        if pointer is None:
            return hi  # unknown: hug the far end, where tray-like applets live
        return min(max(pointer - card / 2.0, SCREEN_MARGIN), hi)

    x = round(along(p.pointer[0] if p.pointer else None, sw, POPOVER_WIDTH))
    y = round(along(p.pointer[1] if p.pointer else None, sh, POPOVER_HEIGHT))
    if p.edge == "bottom":
        return ("bottom", "left"), (0, 0, PANEL_GAP, x)
    if p.edge == "left":
        return ("top", "left"), (y, 0, 0, PANEL_GAP)
    if p.edge == "right":
        return ("top", "right"), (y, PANEL_GAP, 0, 0)
    return ("top", "left"), (PANEL_GAP, 0, 0, x)


def popover_pointer_path() -> Path:
    """Shared pointer cache used by GPUI and QML popover sessions."""
    cache = os.environ.get("XDG_CACHE_HOME")
    root = Path(cache) if cache else Path.home() / ".cache"
    return root / "wabot-popover-pointer"


def load_popover_pointer() -> tuple[float, float] | None:
    try:
        values = popover_pointer_path().read_text().split()
        x, y = float(values[0]), float(values[1])
    except (OSError, ValueError, IndexError):
        return None
    return (x, y) if math.isfinite(x) and math.isfinite(y) else None


def save_popover_pointer(pointer: tuple[float, float]) -> None:
    path = popover_pointer_path()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"{pointer[0]} {pointer[1]}\n")
    except OSError:
        pass


# --- misc -------------------------------------------------------------------------------

@dataclass
class RecentSet:
    """Bounded insertion-ordered set (ids of messages we already handled)."""
    limit: int = 512
    _d: dict = field(default_factory=dict)

    def add(self, key: str) -> bool:
        """True when `key` is new."""
        if key in self._d:
            return False
        self._d[key] = None
        if len(self._d) > self.limit:
            self._d.pop(next(iter(self._d)))
        return True
