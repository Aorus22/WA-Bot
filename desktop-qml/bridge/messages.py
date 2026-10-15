"""QML global `Messages`: the open conversation's messages and everything you can do to one.

* `Messages.model` — newest-first list model (index 0 = bottom of the chat; the QML ListView is
  `BottomToTop` so appending older pages never moves what you are reading). Roles are the REST message
  fields (id chatId from to content timestamp status type mediaUrl senderName replyToId forwarded
  reactions extra starred revokedAt ...) plus display fields (isMine kind segments timeText ticks showDay
  dayLabel firstInGroup lastInGroup quote poll location contact linkPreview statusReply ...).
* follows `AppState.activeChatId`; WS events keep it live (see `_on_event`).
* composer contract: `addPending(row)` / `ackPending(localId, serverId)` / `failPending(localId, error)`.
* other modules: `jumpTo(messageId)` (search / starred / pinned banner), `forwardSend(chatId, ids, targets)`.
Ports of GPUI `state/chat.rs`, `views/chat/data/loading.rs`, `views/chat/messages/*`."""
from __future__ import annotations

import json
import threading
import time
from typing import Any, Callable

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QDesktopServices, QGuiApplication, QImage
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest

from . import messages_text as T
from .models import DictListModel

PAGE_INITIAL = 100
PAGE_MORE = 40
CONTEXT_LIMIT = 50
FRESH_TTL = 0.6
TYPING_TTL = 6.0
HIGHLIGHT_MS = 2500
TEMP_PREFIX = "temp-"

ROLES = [
    # REST fields
    "id", "chatId", "from", "to", "content", "timestamp", "status", "type", "mediaUrl", "isAutomatic",
    "senderName", "replyToId", "forwarded", "reactions", "extra", "starred", "revokedAt",
    # display fields
    "isMine", "kind", "body", "isMarkdown", "segments", "hasText", "copyText", "timeText", "ticks", "pending",
    "failed", "errorText", "ts", "dayLabel", "showDay", "firstInGroup", "lastInGroup", "senderJid",
    "senderLabel", "senderColor", "mediaSrc", "fileName", "fileExt", "quote", "statusReply", "poll",
    "location", "contact", "linkPreview", "disappearing", "edited", "editCount", "editedAt", "pinned",
    "selected", "viewOnce", "mentionsMe", "chips", "canEdit", "canDelete", "canReact",
]

MEDIA_KINDS = ("image", "video", "gif")


def _now_ms() -> int:
    return int(time.time() * 1000)


def _ts_ms(m: dict) -> int:
    t = int(m.get("timestamp") or 0)
    return t if t > 10_000_000_000 else t * 1000


class MessageModel(DictListModel):
    """DictListModel plus in-place row replacement / re-keying (pending -> real id)."""

    def replace_at(self, i: int, row: dict) -> None:
        old = self._rows[i].get(self._key)
        self._rows[i] = row
        new = row.get(self._key)
        if new != old:
            self._index.pop(old, None)
            self._index[new] = i
        self._emit(i)

    def rekey(self, old: str, new: str, fields: dict | None = None) -> bool:
        i = self._index.get(old)
        if i is None:
            return False
        self._rows[i]["id"] = new
        if fields:
            self._rows[i].update(fields)
        self._index.pop(old, None)
        self._index[new] = i
        self._emit(i)
        return True

    def append_many(self, rows: list[dict]) -> None:
        self.extend(rows)  # end of list = older messages

    def reset_rows(self, rows: list[dict]) -> None:
        self.reset(rows)


class Messages(QObject):
    chatChanged = Signal()
    loadingChanged = Signal()
    pagingChanged = Signal()
    selectionChanged = Signal()
    typingChanged = Signal()
    pinsChanged = Signal()
    highlightChanged = Signal()
    infoChanged = Signal()
    formatChanged = Signal()
    revealRequested = Signal(int, str)        # model index, message id — scroll there
    scrollToBottomRequested = Signal()
    newMessage = Signal(bool)                 # arrived at the tail; True = mine
    forwardFinished = Signal(bool)
    saved = Signal(str, bool)                 # label, ok

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._api = services.api
        self._app = services.app
        self._settings = services.settings
        self.model = MessageModel(ROLES, key="id", parent=self)
        self._chat = ""
        self._gen = 0
        self._loading = False
        self._loading_more = False
        self._loading_newer = False
        self._has_more = False
        self._has_more_next = False
        self._group: dict | None = None
        self._pins: list[dict] = []
        self._pin_ids: set[str] = set()
        self._highlight = ""
        self._fresh: dict[str, float] = {}
        self._typers: dict[str, dict] = {}
        self._select = False
        self._confirm_delete = False
        self._info: dict = {}
        self._info_open = False
        self._info_loading = False
        self._seen_connected = False
        self._phone = ""
        self._nam = QNetworkAccessManager(self)
        self._highlight_timer = QTimer(self)
        self._highlight_timer.setSingleShot(True)
        self._highlight_timer.timeout.connect(self._clear_highlight)
        self._typing_timer = QTimer(self)
        self._typing_timer.setInterval(1000)
        self._typing_timer.timeout.connect(self._prune_typing)
        self._pins_timer = QTimer(self)  # a pin lapses by itself
        self._pins_timer.setInterval(30_000)
        self._pins_timer.timeout.connect(self._refresh_pin_state)
        self._pins_timer.start()
        self._app.activeChatChanged.connect(self._on_active_chat)
        self._app.windowVisibleChanged.connect(self._on_visible)
        services.hub.event.connect(self._on_event)
        services.hub.connectedChanged.connect(self._on_connected)
        self._settings.changed.connect(self._on_setting)
        if self._app.activeChatId:
            QTimer.singleShot(0, self._on_active_chat)

    # ------------------------------------------------------------------ properties
    @Property(QObject, constant=True)
    def modelObject(self) -> QObject:  # alias, in case a binding wants the explicit name
        return self.model

    @Property(str, notify=chatChanged)
    def chatId(self) -> str:
        return self._chat

    @Property(bool, notify=chatChanged)
    def isGroup(self) -> bool:
        return self._is_group()

    @Property(bool, notify=loadingChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(bool, notify=loadingChanged)
    def loadingMore(self) -> bool:
        return self._loading_more

    @Property(bool, notify=pagingChanged)
    def hasMore(self) -> bool:
        return self._has_more

    @Property(bool, notify=pagingChanged)
    def hasMoreNext(self) -> bool:
        return self._has_more_next

    @Property(bool, notify=formatChanged)
    def formatEnabled(self) -> bool:
        return bool(self._settings.get_py("format_messages", True))

    @Property(str, notify=highlightChanged)
    def highlightedId(self) -> str:
        return self._highlight

    # selection ----------------------------------------------------------
    @Property(bool, notify=selectionChanged)
    def selectMode(self) -> bool:
        return self._select

    @Property(int, notify=selectionChanged)
    def selectedCount(self) -> int:
        return sum(1 for r in self.model.rows() if r.get("selected"))

    @Property(bool, notify=selectionChanged)
    def selectConfirmDelete(self) -> bool:
        return self._confirm_delete

    @Property(bool, notify=selectionChanged)
    def selectionDeletable(self) -> bool:
        sel = self._selected_rows()
        return bool(sel) and all(r["isMine"] and not r["revokedAt"] for r in sel)

    # typing -------------------------------------------------------------
    @Property(bool, notify=typingChanged)
    def typingVisible(self) -> bool:
        return bool(self._live_typers())

    @Property(str, notify=typingChanged)
    def typingJid(self) -> str:
        t = self._live_typers()
        return t[0][0] if t else ""

    @Property(str, notify=typingChanged)
    def typingName(self) -> str:
        t = self._live_typers()
        return t[0][1]["name"] if t else ""

    @Property(bool, notify=typingChanged)
    def typingAudio(self) -> bool:
        t = self._live_typers()
        return bool(t and t[0][1]["audio"])

    @Property(str, notify=typingChanged)
    def typingLabel(self) -> str:
        """Chat-list style line: `typing…` / `Name is typing…`."""
        t = self._live_typers()
        if not t:
            return ""
        verb = "recording audio…" if t[0][1]["audio"] else "typing…"
        if not self._is_group():
            return verb
        names = [x[1]["name"] for x in t]
        if len(names) == 1:
            return f"{names[0]} is {verb}"
        if len(names) == 2:
            return f"{names[0]} and {names[1]} are {verb}"
        return f"{names[0]} and {len(names) - 1} others are {verb}"

    # pins ---------------------------------------------------------------
    @Property("QVariantList", notify=pinsChanged)
    def pins(self) -> list:
        """Active pins, newest first: {messageId, sender, snippet, expiresAt}."""
        out = []
        for p in self._active_pins():
            msg = p.get("message") or {}
            out.append({"messageId": p.get("messageId", ""), "sender": self._sender_label(msg),
                        "snippet": T.snippet_of(msg), "expiresAt": int(p.get("expiresAt") or 0)})
        return out

    # message info sheet ----------------------------------------------------
    @Property(bool, notify=infoChanged)
    def infoOpen(self) -> bool:
        return self._info_open

    @Property("QVariantMap", notify=infoChanged)
    def info(self) -> dict:
        return self._info

    # ------------------------------------------------------------------ helpers
    def _is_group(self) -> bool:
        if self._chat.endswith("@g.us"):
            return True
        chats = self._svc.exposed.get("Chats")
        try:
            c = chats.get(self._chat) if chats is not None else None
            return bool(c and c.get("isGroup"))
        except Exception:  # noqa: BLE001 - the chat list module is optional here
            return False

    def _toast(self, msg: str, kind: str = "info") -> None:
        if self._svc.toast is not None:
            self._svc.toast.show(msg, kind)

    def _path(self, chat_id: str, tail: str = "") -> str:
        return f"/chats/{self._api.enc(chat_id)}{tail}"

    def _media(self, url: str | None) -> str:
        if not url:
            return ""
        if url.startswith(("file:", "qrc:", "data:")):
            return url
        return self._api.mediaUrl(url)

    def _visible(self) -> bool:
        return bool(self._app.windowVisible)

    def _participants(self) -> list[dict]:
        return (self._group or {}).get("participants") or []

    def _is_mine(self, m: dict) -> bool:
        frm = m.get("from") or ""
        if frm == "me" or str(m.get("id") or "").startswith(TEMP_PREFIX) or m.get("status") == "pending":
            return True
        phone = self._app.phone or self._phone
        return bool(phone) and T.user_of(frm) == phone

    def _sender_label(self, m: dict) -> str:
        if self._is_mine(m):
            return "You"
        return T.sender_name(m)

    def _chat_row(self, chat_id: str) -> dict | None:
        chats = self._svc.exposed.get("Chats")
        try:
            return chats.get(chat_id) if chats is not None else None
        except Exception:  # noqa: BLE001
            return None

    # ------------------------------------------------------------------ row building
    def _norm(self, m: dict) -> dict:
        """Raw message -> normalised raw fields (consistent types for QML)."""
        return {
            "id": str(m.get("id") or ""), "chatId": m.get("chatId") or self._chat,
            "from": m.get("from") or "", "to": m.get("to") or "", "content": m.get("content") or "",
            "timestamp": int(m.get("timestamp") or 0), "status": m.get("status") or "",
            "type": m.get("type") or "", "mediaUrl": m.get("mediaUrl") or "",
            "isAutomatic": bool(m.get("isAutomatic")), "senderName": m.get("senderName") or "",
            "replyToId": m.get("replyToId") or "", "forwarded": bool(m.get("forwarded")),
            "reactions": list(m.get("reactions") or []), "extra": dict(m.get("extra") or {}),
            "starred": bool(m.get("starred")), "revokedAt": int(m.get("revokedAt") or 0),
            "errorText": m.get("errorText") or "",
        }

    def _build(self, m: dict, keep: dict | None = None) -> dict:
        raw = self._norm(m)
        extra = raw["extra"]
        kind = T.message_kind(raw)
        mine = self._is_mine(raw)
        status = raw["status"]
        pending = raw["id"].startswith(TEMP_PREFIX) or status == "pending"
        ticks = "none"
        if mine:
            ticks = {"read": "read", "delivered": "delivered", "failed": "failed"}.get(status, "sent")
        decoded = T.decode_content(raw["content"])
        placeholder = not decoded.strip() or decoded in T.PLACEHOLDERS
        has_card = bool((kind == "poll" and extra.get("poll")) or (kind == "location" and extra.get("location"))
                        or (kind == "contact" and extra.get("contact")))
        hide = (kind in ("document", "audio") or has_card
                or (kind in ("image", "sticker", "video", "gif") and placeholder))
        body, segments, markdown = T.body_segments(raw["content"], extra, self._participants(), self.formatEnabled)
        has_text = bool(decoded.strip()) and not hide
        copyable = bool(decoded.strip()) and decoded not in T.PLACEHOLDERS
        file_name = raw["content"].strip() if kind == "document" else ""
        if kind == "document" and not file_name:
            file_name = "Document"
        ext = file_name.rsplit(".", 1)[1].upper() if "." in file_name and file_name.rsplit(".", 1)[1] else "FILE"
        sender = raw["from"]
        if mine and not sender:
            sender = "me"
        edits = extra.get("edits") or []
        reactions = raw["reactions"]
        chips = [{"emoji": r.get("emoji", ""), "count": len(r.get("senders") or []),
                  "mine": "me" in (r.get("senders") or [])} for r in reactions]
        thumb = lambda u: self._media(u) if u else ""  # noqa: E731
        sr = extra.get("statusReply")
        status_reply = None
        if sr:
            status_reply = {"label": T.status_reply_label(sr),
                            "thumb": thumb(sr.get("mediaUrl")) if (sr.get("type") == "image") else ""}
        lp = extra.get("linkPreview")
        link_preview = None
        if lp and not (kind in ("poll", "location", "contact")):
            link_preview = {"url": lp.get("url", ""), "title": (lp.get("title") or "").strip(),
                            "description": (lp.get("description") or "").strip(),
                            "thumb": thumb(lp.get("thumbnailUrl")), "host": T.host_of(lp.get("url", ""))}
        vo = extra.get("viewOnce")
        row = dict(raw)
        row.update({
            "isMine": mine, "kind": kind, "body": body, "isMarkdown": markdown, "segments": segments,
            "hasText": has_text, "copyText": decoded if copyable else "", "timeText": T.format_time(raw["timestamp"]),
            "ticks": ticks, "pending": pending, "failed": status == "failed", "ts": _ts_ms(raw),
            "dayLabel": "", "showDay": False, "firstInGroup": True, "lastInGroup": True,
            "senderJid": sender, "senderLabel": self._sender_label(raw), "senderColor": T.avatar_color(sender),
            "mediaSrc": self._media(raw["mediaUrl"]), "fileName": file_name, "fileExt": ext,
            "quote": None, "statusReply": status_reply, "poll": T.poll_view(extra.get("poll")),
            "location": T.location_view(extra.get("location"), thumb((extra.get("location") or {}).get("thumbnailUrl"))),
            "contact": T.contact_view(extra.get("contact")), "linkPreview": link_preview,
            "disappearing": T.disappearing_note(extra.get("disappearing"), _now_ms()),
            "edited": bool(edits), "editCount": len(edits), "editedAt": int(extra.get("editedAt") or 0),
            "pinned": raw["id"] in self._pin_ids, "selected": bool(keep and keep.get("selected")),
            "viewOnce": ({"viewed": bool(vo.get("viewed")), "label": "video" if vo.get("mediaType") == "video" else "photo"}
                         if vo else None),
            "mentionsMe": bool(extra.get("mentionsMe")), "chips": chips,
            "canEdit": mine and not raw["mediaUrl"] and not raw["revokedAt"] and not pending and kind in ("text",),
            "canDelete": mine and not raw["revokedAt"] and not pending,
            "canReact": not pending,
        })
        if keep and keep.get("viewOnce") and row["viewOnce"] and keep["viewOnce"].get("viewed"):
            row["viewOnce"]["viewed"] = True
        return row

    def _flags(self, i: int, rows: list[dict] | None = None) -> dict:
        rows = self.model.rows() if rows is None else rows
        r = rows[i]
        older = rows[i + 1] if i + 1 < len(rows) else None
        newer = rows[i - 1] if i > 0 else None
        show_day = older is None or T.local_day(older["timestamp"]) != T.local_day(r["timestamp"])
        return {
            "showDay": show_day,
            "dayLabel": T.format_date_label(r["timestamp"]).upper() if show_day else "",
            "firstInGroup": older is None or older["senderJid"] != r["senderJid"],
            "lastInGroup": newer is None or newer["senderJid"] != r["senderJid"],
        }

    def _quote_for(self, r: dict, by_id: dict[str, dict]) -> dict | None:
        rid = r["replyToId"]
        if not rid or r["statusReply"]:
            return None
        orig = by_id.get(rid)
        if orig is None:
            return {"sender": "Message", "text": "Open original message", "thumb": "", "resolved": False,
                    "targetId": rid, "spans": []}
        sender = "You" if orig["isMine"] else (orig["senderName"] or "Sender")
        text, segments, _ = T.body_segments(T.first_line(T.decode_content(orig["content"])), orig["extra"],
                                            self._participants(), False)
        placeholder = not text.strip() or (text.startswith("[") and text.endswith("]"))
        if placeholder:
            text, segments = T.quote_media_label(orig), []
        thumb = orig["mediaSrc"] if orig["kind"] in ("image", "sticker", "gif") and orig["mediaUrl"] else ""
        return {"sender": sender, "text": text, "thumb": thumb, "resolved": True, "targetId": rid, "segments": segments}

    def _refresh_all(self, only_flags: bool = False) -> None:
        """Recompute neighbour-dependent fields (flags, quotes) for every row, patching only what changed."""
        rows = self.model.rows()
        by_id = {r["id"]: r for r in rows}
        for i, r in enumerate(rows):
            upd = self._flags(i, rows)
            if not only_flags:
                q = self._quote_for(r, by_id)
                if q != r.get("quote"):
                    upd["quote"] = q
            changed = {k: v for k, v in upd.items() if r.get(k) != v}
            if changed:
                self.model.patch(r["id"], changed)

    def _refresh_around(self, i: int) -> None:
        rows = self.model.rows()
        by_id = None
        for j in (i - 1, i, i + 1):
            if 0 <= j < len(rows):
                upd = self._flags(j, rows)
                r = rows[j]
                if r["replyToId"] or j == i:
                    by_id = by_id or {x["id"]: x for x in rows}
                    q = self._quote_for(r, by_id)
                    if q != r.get("quote"):
                        upd["quote"] = q
                changed = {k: v for k, v in upd.items() if r.get(k) != v}
                if changed:
                    self.model.patch(r["id"], changed)

    def _finish_rows(self, rows: list[dict]) -> list[dict]:
        """Fill flags + quotes for a freshly built list (newest first) before it enters the model."""
        by_id = {r["id"]: r for r in rows}
        for i, r in enumerate(rows):
            r.update(self._flags(i, rows))
            r["quote"] = self._quote_for(r, by_id)
        return rows

    # ------------------------------------------------------------------ loading
    def _on_active_chat(self) -> None:
        chat = self._app.activeChatId
        self._gen += 1
        self._chat = chat
        self._group = None
        self._pins, self._pin_ids = [], set()
        self._typers.clear()
        self._fresh.clear()
        self._highlight = ""
        self._select = False
        self._confirm_delete = False
        self._info_open = False
        self._info = {}
        self._has_more = self._has_more_next = False
        self._loading_more = self._loading_newer = False
        self.model.reset([])
        self.chatChanged.emit()
        self.pagingChanged.emit()
        self.pinsChanged.emit()
        self.typingChanged.emit()
        self.selectionChanged.emit()
        self.highlightChanged.emit()
        self.infoChanged.emit()
        if not chat:
            self._loading = False
            self.loadingChanged.emit()
            return
        self._loading = True
        self.loadingChanged.emit()
        self.reload()
        self._load_pins()
        if chat.endswith("@g.us"):
            self._load_group(chat)
        else:
            self._api.fetch("POST", self._path(chat, "/presence-subscribe"))  # 1:1 typing events need this
        if self._visible():
            self._mark_read()

    @Slot()
    def reload(self) -> None:
        """Fetch the latest page again and replace the list (also `Scroll to newest` after a jump)."""
        chat, gen = self._chat, self._gen
        if not chat:
            return
        self._loading = True
        self.loadingChanged.emit()

        def done(status: int, data: Any) -> None:
            if gen != self._gen:
                return
            self._loading = False
            self.loadingChanged.emit()
            if status != 200 or not isinstance(data, list):
                return
            self._set_messages(data, has_more=len(data) >= PAGE_INITIAL, has_next=False)
            self.scrollToBottomRequested.emit()
        self._api.fetch("GET", self._path(chat, f"/messages?limit={PAGE_INITIAL}"), None, done)

    def _set_messages(self, msgs: list[dict], has_more: bool, has_next: bool) -> None:
        msgs = sorted((m for m in msgs if isinstance(m, dict) and m.get("id")), key=_ts_ms)
        # de-dup (a context page can repeat ids)
        seen, uniq = set(), []
        for m in msgs:
            if m["id"] not in seen:
                seen.add(m["id"])
                uniq.append(m)
        rows = self._finish_rows([self._build(m) for m in reversed(uniq)])
        self.model.reset(rows)
        self._has_more, self._has_more_next = has_more, has_next
        self.pagingChanged.emit()
        self.selectionChanged.emit()

    @Slot()
    def loadOlder(self) -> None:
        if self._loading_more or not self._has_more or self._loading or not self._chat:
            return
        rows = self.model.rows()
        if not rows:
            return
        oldest = rows[-1]["timestamp"]
        if not oldest:
            return
        chat, gen = self._chat, self._gen
        self._loading_more = True
        self.loadingChanged.emit()

        def done(status: int, data: Any) -> None:
            if gen != self._gen:
                return
            self._loading_more = False
            self.loadingChanged.emit()
            if status != 200 or not isinstance(data, list):
                return
            if not data:
                self._has_more = False
                self.pagingChanged.emit()
                return
            self._has_more = len(data) >= PAGE_MORE
            fresh = [m for m in sorted((x for x in data if isinstance(x, dict) and x.get("id")), key=_ts_ms)
                     if not self.model.contains(m["id"])]
            old_len = len(self.model.rows())
            new_rows = [self._build(m) for m in reversed(fresh)]
            self.model.extend(new_rows)  # appended at the end = older; the view does not move
            self._refresh_around(old_len - 1)
            self._refresh_all()  # quotes whose original just arrived
            self.pagingChanged.emit()
        self._api.fetch("GET", self._path(chat, f"/messages?limit={PAGE_MORE}&before={oldest}"), None, done)

    @Slot()
    def loadNewer(self) -> None:
        if self._loading_newer or not self._has_more_next or self._loading or not self._chat:
            return
        rows = self.model.rows()
        if not rows:
            return
        newest = rows[0]["timestamp"]
        chat, gen = self._chat, self._gen
        self._loading_newer = True

        def done(status: int, data: Any) -> None:
            if gen != self._gen:
                return
            self._loading_newer = False
            if status != 200 or not isinstance(data, list):
                return
            if not data:
                self._has_more_next = False
                self.pagingChanged.emit()
                return
            self._has_more_next = len(data) >= PAGE_MORE
            fresh = [m for m in sorted((x for x in data if isinstance(x, dict) and x.get("id")), key=_ts_ms)
                     if not self.model.contains(m["id"])]
            for m in fresh:
                self._insert_sorted(self._build(m))
            self.pagingChanged.emit()
        self._api.fetch("GET", self._path(chat, f"/messages?limit={PAGE_MORE}&after={newest}"), None, done)

    @Slot()
    def jumpToLatest(self) -> None:
        """`Scroll to newest`: if we are inside an old context window the latest page is fetched again."""
        if self._has_more_next:
            self.reload()
        else:
            self.scrollToBottomRequested.emit()

    @Slot(str)
    def jumpTo(self, message_id: str) -> None:
        """Reveal (and flash) a message, fetching its surroundings when it is not loaded (GPUI `navigate_to_message`)."""
        if not message_id or not self._chat:
            return
        self._set_highlight(message_id)
        i = self.model.row_of(message_id)
        if i >= 0:
            self.revealRequested.emit(i, message_id)
            return
        chat, gen = self._chat, self._gen
        self._loading = True
        self.loadingChanged.emit()

        def done(status: int, data: Any) -> None:
            if gen != self._gen:
                return
            self._loading = False
            self.loadingChanged.emit()
            if status == 200 and isinstance(data, list) and data:
                self._set_messages(data, has_more=True, has_next=True)
                j = self.model.row_of(message_id)
                if j >= 0:
                    self.revealRequested.emit(j, message_id)
        self._api.fetch("GET", self._path(chat, f"/messages/{self._api.enc(message_id)}/context?limit={CONTEXT_LIMIT}"),
                        None, done)

    @Slot(str, str)
    def openChatAt(self, chat_id: str, message_id: str) -> None:
        """Open another chat and reveal a message once it is loaded (starred list)."""
        if chat_id != self._chat:
            self._pending_jump = (chat_id, message_id)
            self._app.openChat(chat_id)
            gen = self._gen

            def later():
                if self._chat == chat_id and not self._loading:
                    self.jumpTo(message_id)
                else:
                    QTimer.singleShot(150, later)
            QTimer.singleShot(150, later)
        else:
            self.jumpTo(message_id)

    def _set_highlight(self, message_id: str) -> None:
        self._highlight = message_id
        self.highlightChanged.emit()
        self._highlight_timer.start(HIGHLIGHT_MS)

    def _clear_highlight(self) -> None:
        self._highlight = ""
        self.highlightChanged.emit()

    @Slot(str, result=int)
    def indexOfMessage(self, message_id: str) -> int:
        return self.model.row_of(message_id)

    @Slot(str, result="QVariantMap")
    def rowOf(self, message_id: str) -> dict:
        return dict(self.model.get_row(message_id) or {})

    @Slot(str, result=bool)
    def isFresh(self, message_id: str) -> bool:
        t = self._fresh.get(message_id)
        return bool(t and time.monotonic() - t < FRESH_TTL)

    # ------------------------------------------------------------------ pins / group
    def _load_pins(self) -> None:
        chat, gen = self._chat, self._gen

        def done(status: int, data: Any) -> None:
            if gen != self._gen or chat != self._chat or status != 200 or not isinstance(data, list):
                return
            self._pins = [p for p in data if isinstance(p, dict) and p.get("message")]
            self._sync_pins()
        self._api.fetch("GET", self._path(chat, "/pins"), None, done)

    def _active_pins(self) -> list[dict]:
        now = _now_ms()
        return [p for p in self._pins if p.get("message") and (int(p.get("expiresAt") or 0) <= 0
                                                               or int(p.get("expiresAt") or 0) > now)]

    def _sync_pins(self) -> None:
        self._pin_ids = {p["messageId"] for p in self._active_pins()}
        for r in list(self.model.rows()):
            pinned = r["id"] in self._pin_ids
            if r["pinned"] != pinned:
                self.model.patch(r["id"], {"pinned": pinned})
        self.pinsChanged.emit()

    def _refresh_pin_state(self) -> None:
        if self._pins:
            self._sync_pins()

    def _load_group(self, chat: str) -> None:
        gen = self._gen

        def done(status: int, data: Any) -> None:
            if gen != self._gen or status != 200 or not isinstance(data, dict):
                return
            self._group = data
            self._rebuild_rows()  # mention names can resolve now
        self._api.fetch("GET", f"/groups/{self._api.enc(chat)}", None, done)

    def _rebuild_rows(self) -> None:
        rows = self.model.rows()
        for i, r in enumerate(list(rows)):
            raw = {k: r[k] for k in ("id", "chatId", "from", "to", "content", "timestamp", "status", "type",
                                     "mediaUrl", "isAutomatic", "senderName", "replyToId", "forwarded",
                                     "reactions", "extra", "starred", "revokedAt", "errorText")}
            new = self._build(raw, keep=r)
            new.update({k: r[k] for k in ("dayLabel", "showDay", "firstInGroup", "lastInGroup", "quote")})
            self.model.replace_at(i, new)
        self._refresh_all()

    # ------------------------------------------------------------------ websocket
    def _on_setting(self, key: str) -> None:
        if key == "format_messages":
            self.formatChanged.emit()
            self._rebuild_rows()

    def _on_connected(self) -> None:
        if not self._svc.hub.connected:
            return
        first = not self._seen_connected
        self._seen_connected = True
        if not first and self._chat and not self._has_more_next:
            self._refresh_latest()  # events may have been missed while offline

    def _refresh_latest(self) -> None:
        chat, gen = self._chat, self._gen

        def done(status: int, data: Any) -> None:
            if gen != self._gen or status != 200 or not isinstance(data, list):
                return
            for m in sorted((x for x in data if isinstance(x, dict) and x.get("id")), key=_ts_ms):
                self._apply_incoming(m, from_refresh=True)
        self._api.fetch("GET", self._path(chat, f"/messages?limit={PAGE_INITIAL}"), None, done)

    def _on_event(self, etype: str, payload_json: str) -> None:
        try:
            p = json.loads(payload_json) if payload_json else {}
        except ValueError:
            return
        if not isinstance(p, dict):
            return
        handler = getattr(self, f"_ev_{etype}", None)
        if handler is not None:
            handler(p)

    def _for_active(self, p: dict) -> bool:
        return bool(self._chat) and p.get("chatId") == self._chat

    def _ev_new_message(self, p: dict) -> None:
        if self._for_active(p):
            self._apply_incoming(p)

    def _ev_message_status(self, p: dict) -> None:
        mid, status = p.get("id"), p.get("status") or ""
        if mid and self.model.contains(mid) and (not p.get("chatId") or p.get("chatId") == self._chat):
            self._patch_raw(mid, {"status": status})

    def _ev_message_deleted(self, p: dict) -> None:
        if self._for_active(p) and p.get("id") in self.model._index:
            i = self.model.row_of(p["id"])
            self.model.remove(p["id"])
            self._refresh_around(min(i, len(self.model.rows()) - 1))
            if self._select:
                self.selectionChanged.emit()

    def _ev_message_revoked(self, p: dict) -> None:
        if self._for_active(p):
            self._patch_raw(p.get("id", ""), {"revokedAt": int(p.get("revokedAt") or _now_ms())})

    def _ev_message_edited(self, p: dict) -> None:
        if not self._for_active(p):
            return
        fields: dict = {"content": p.get("content") or ""}
        if isinstance(p.get("extra"), dict):
            fields["extra"] = p["extra"]
        self._patch_raw(p.get("id", ""), fields)

    def _ev_message_reaction(self, p: dict) -> None:
        if self._for_active(p):
            self._patch_raw(p.get("id", ""), {"reactions": p.get("reactions") or []})

    def _ev_poll_update(self, p: dict) -> None:
        mid = p.get("id") or p.get("messageId")
        if self._for_active(p) and isinstance(p.get("extra"), dict):
            self._patch_raw(mid or "", {"extra": p["extra"]})

    def _ev_message_starred(self, p: dict) -> None:
        if self._for_active(p):
            self._patch_raw(p.get("id", ""), {"starred": bool(p.get("starred"))})

    def _ev_chat_pins_changed(self, p: dict) -> None:
        if self._for_active(p):
            self._load_pins()

    def _ev_chat_cleared(self, p: dict) -> None:
        if self._for_active(p):
            self._has_more = self._has_more_next = False
            self.model.reset([])
            self.pagingChanged.emit()

    def _ev_chat_deleted(self, p: dict) -> None:
        self._ev_chat_cleared(p)

    def _ev_logged_out(self, p: dict) -> None:
        self.model.reset([])
        self._chat = ""
        self._gen += 1
        self.chatChanged.emit()

    def _ev_chats_changed(self, p: dict) -> None:
        if p.get("reason") in ("names", "history_sync") and self._chat and not self._has_more_next:
            self._refresh_latest()  # names changed: sender labels in cached rows are stale

    def _ev_chat_presence(self, p: dict) -> None:
        if not self._for_active(p):
            return
        sender = p.get("sender") or self._chat
        if p.get("state") == "composing":
            name = (p.get("senderName") or "").strip()
            if not name:
                row = self._chat_row(sender)
                name = (row or {}).get("name") or T.user_of(sender)
            self._typers[sender] = {"name": name, "audio": p.get("media") == "audio", "at": time.monotonic()}
            if not self._typing_timer.isActive():
                self._typing_timer.start()
        else:
            self._typers.pop(sender, None)
        self.typingChanged.emit()

    def _live_typers(self) -> list[tuple[str, dict]]:
        now = time.monotonic()
        live = [(j, t) for j, t in self._typers.items() if now - t["at"] < TYPING_TTL]
        live.sort(key=lambda x: x[1]["at"])
        return live

    def _prune_typing(self) -> None:
        before = len(self._typers)
        now = time.monotonic()
        self._typers = {j: t for j, t in self._typers.items() if now - t["at"] < TYPING_TTL}
        if len(self._typers) != before:
            self.typingChanged.emit()
        if not self._typers:
            self._typing_timer.stop()

    # -- applying messages --------------------------------------------------------------------
    def _patch_raw(self, mid: str, fields: dict) -> None:
        """Merge raw fields into a row and rebuild its display fields."""
        i = self.model.row_of(mid)
        if i < 0:
            return
        cur = self.model.rows()[i]
        raw = {k: cur[k] for k in ("id", "chatId", "from", "to", "content", "timestamp", "status", "type",
                                   "mediaUrl", "isAutomatic", "senderName", "replyToId", "forwarded", "reactions",
                                   "extra", "starred", "revokedAt", "errorText")}
        raw.update(fields)
        new = self._build(raw, keep=cur)
        new.update({k: cur[k] for k in ("dayLabel", "showDay", "firstInGroup", "lastInGroup", "quote")})
        self.model.replace_at(i, new)
        if "content" in fields or "extra" in fields or "revokedAt" in fields:
            # a quote of this message shows its text
            for r in list(self.model.rows()):
                if r["replyToId"] == mid:
                    q = self._quote_for(r, {x["id"]: x for x in self.model.rows()})
                    if q != r["quote"]:
                        self.model.patch(r["id"], {"quote": q})

    def _find_pending(self, m: dict) -> int:
        """GPUI `upsert_message` rule 4: a real message replaces the matching optimistic one in place."""
        mtype = m.get("type") or ""
        for i, r in enumerate(self.model.rows()):
            if r["pending"] and r["id"].startswith(TEMP_PREFIX) and (
                    r["content"] == (m.get("content") or "")
                    or (r["type"] == mtype and mtype in T.MEDIA_TYPES | {"ptt", "voice"})):
                return i
        return -1

    def _apply_incoming(self, m: dict, from_refresh: bool = False) -> None:
        mid = m.get("id")
        if not mid:
            return
        if self.model.contains(mid):
            cur = self.model.get_row(mid)
            i = self.model.row_of(mid)
            new = self._build(m, keep=cur)
            new.update({k: cur[k] for k in ("dayLabel", "showDay", "firstInGroup", "lastInGroup", "quote")})
            if i >= 0 and self._differs(cur, new):
                self.model.replace_at(i, new)
            return
        if self._has_more_next:
            return  # browsing an old window: the newest page is fetched when the user scrolls down
        if not str(mid).startswith(TEMP_PREFIX):
            pi = self._find_pending(m)
            if pi >= 0:
                cur = self.model.rows()[pi]
                new = self._build(m, keep=cur)
                new.update({k: cur[k] for k in ("dayLabel", "showDay", "firstInGroup", "lastInGroup", "quote")})
                self.model.replace_at(pi, new)
                self._refresh_around(pi)
                return
        row = self._build(m)
        idx = self._insert_sorted(row)
        if not from_refresh:
            if idx == 0:
                self._fresh[mid] = time.monotonic()
                self.newMessage.emit(row["isMine"])
            if row["status"] == "received" and self._visible():
                self._mark_read()

    @staticmethod
    def _differs(a: dict, b: dict) -> bool:
        keys = ("content", "status", "reactions", "extra", "starred", "revokedAt", "mediaUrl", "type", "senderName")
        return any(a.get(k) != b.get(k) for k in keys)

    def _insert_sorted(self, row: dict) -> int:
        rows = self.model.rows()
        i = 0
        while i < len(rows) and rows[i]["ts"] > row["ts"]:
            i += 1
        self.model.insert(i, row)
        self._refresh_around(i)
        return i

    def _mark_read(self) -> None:
        if self._chat:
            self._api.fetch("POST", self._path(self._chat, "/read"))

    def _on_visible(self) -> None:
        if self._visible() and self._chat:
            self._mark_read()

    # ------------------------------------------------------------------ composer contract
    @Slot("QVariantMap")
    def addPending(self, row: dict) -> None:
        """Optimistic bubble for a message being sent. `row` needs `id` (a `temp-...` local id), `chatId`
        and the content/type fields; defaults fill the rest (status pending, from me, now).
        For media pass `mediaUrl` as a `file://` URL of the local file (shown until the server echo arrives)."""
        row = dict(row or {})
        lid = str(row.get("id") or f"{TEMP_PREFIX}{int(time.time() * 1000)}")
        if not lid.startswith(TEMP_PREFIX):
            lid = TEMP_PREFIX + lid
        row["id"] = lid
        row.setdefault("chatId", self._chat)
        if row["chatId"] != self._chat:
            return  # not the open chat: the server echo / next load shows it
        row.setdefault("from", "me")
        row["status"] = "pending"
        row.setdefault("timestamp", _now_ms())
        row.setdefault("type", "text")
        if self.model.contains(lid):
            return
        if self._has_more_next:
            self.reload()  # own message: go back to the live end
        idx = self._insert_sorted(self._build(row))
        self._fresh[lid] = time.monotonic()
        self.newMessage.emit(True)
        self.scrollToBottomRequested.emit()
        _ = idx

    @Slot(str, str)
    def ackPending(self, local_id: str, server_id: str) -> None:
        """The server accepted the send. The echo (`new_message`) may already have replaced the temp row."""
        if not self.model.contains(local_id):
            return
        if server_id and self.model.contains(server_id):
            self.model.remove(local_id)
            self._refresh_around(0)
            return
        if server_id:
            self.model.rekey(local_id, server_id, {"pending": False, "status": "sent", "ticks": "sent",
                                                   "errorText": "", "failed": False})
            self._patch_raw(server_id, {"status": "sent"})

    @Slot(str, str)
    def failPending(self, local_id: str, error: str) -> None:
        if self.model.contains(local_id):
            self._patch_raw(local_id, {"status": "failed", "errorText": error or "Failed to send"})

    @Slot(str)
    def discardPending(self, local_id: str) -> None:
        """Drop a failed bubble (the composer re-sends it itself)."""
        if self.model.contains(local_id):
            self.model.remove(local_id)
            self._refresh_around(0)

    # ------------------------------------------------------------------ per-message actions
    def _row(self, mid: str) -> dict | None:
        return self.model.get_row(mid)

    @Slot(str, str)
    def react(self, message_id: str, emoji: str) -> None:
        """Set my reaction (tapping my current emoji again withdraws it)."""
        row = self._row(message_id)
        if not row or not self._chat or row["pending"]:
            return
        mine_now = next((r.get("emoji") for r in row["reactions"] if "me" in (r.get("senders") or [])), "")
        send = "" if mine_now == emoji else emoji
        out: list[dict] = []
        for r in row["reactions"]:
            kept = [s for s in (r.get("senders") or []) if s != "me"]
            if send and r.get("emoji") == send:
                kept.append("me")
            if kept:
                out.append({"emoji": r.get("emoji"), "senders": kept})
        if send and not any(r["emoji"] == send for r in out):
            out.append({"emoji": send, "senders": ["me"]})
        self._patch_raw(message_id, {"reactions": out})
        body: dict = {"emoji": send}
        if not row["isMine"] and row["from"]:
            body["from"] = row["from"]
        chat = self._chat

        def done(status: int, data: Any) -> None:
            if status >= 300 or status == 0:
                self._toast("Gagal mengirim reaksi", "error")
        self._api.fetch("POST", self._path(chat, f"/messages/{self._api.enc(message_id)}/react"), body, done)

    @Slot(str, str)
    def vote(self, message_id: str, option: str) -> None:
        row = self._row(message_id)
        if not row or not row["poll"] or not self._chat:
            return
        poll = row["poll"]
        options = T.toggle_vote(poll["mine"], option, poll["multi"])
        extra = json.loads(json.dumps(row["extra"]))
        votes = extra.setdefault("poll", {}).setdefault("votes", {}) or {}
        extra["poll"]["votes"] = votes
        if options:
            votes["me"] = options
        else:
            votes.pop("me", None)
        self._patch_raw(message_id, {"extra": extra})

        def done(status: int, data: Any) -> None:
            if status >= 300 or status == 0:
                self._toast("Gagal mengirim suara", "error")
        self._api.fetch("POST", self._path(self._chat, f"/messages/{self._api.enc(message_id)}/vote"),
                        {"options": options}, done)

    @Slot(str)
    def toggleStar(self, message_id: str) -> None:
        row = self._row(message_id)
        if not row or not self._chat or row["pending"]:
            return
        starred = not row["starred"]
        self._patch_raw(message_id, {"starred": starred})

        def done(status: int, data: Any) -> None:
            if status == 0 or status >= 300:
                self._patch_raw(message_id, {"starred": not starred})
                self._toast("Gagal memberi bintang" if starred else "Gagal melepas bintang", "error")
        self._api.fetch("POST", self._path(self._chat, f"/messages/{self._api.enc(message_id)}/star"),
                        {"starred": starred}, done)

    @Slot(str, int)
    def pin(self, message_id: str, duration: int) -> None:
        """Pin for `duration` seconds (86400 / 604800 / 2592000)."""
        self._set_pinned(message_id, True, duration or T.DEFAULT_PIN_SECS)

    @Slot(str)
    def unpin(self, message_id: str) -> None:
        self._set_pinned(message_id, False, 0)

    def _set_pinned(self, message_id: str, pinned: bool, duration: int) -> None:
        row = self._row(message_id)
        if not row or not self._chat:
            return
        chat = self._chat
        if pinned:
            now = _now_ms()
            self._pins = [p for p in self._pins if p.get("messageId") != message_id]
            self._pins.insert(0, {"messageId": message_id, "pinnedBy": "me", "pinnedAt": now,
                                  "expiresAt": now + duration * 1000, "message": self._raw_of(row)})
        else:
            self._pins = [p for p in self._pins if p.get("messageId") != message_id]
        self._sync_pins()

        def done(status: int, data: Any) -> None:
            if status and status < 300:
                self._toast("Pesan disematkan" if pinned else "Sematan dilepas", "success")
            else:
                reason = (data.get("error") if isinstance(data, dict) else "") or "permintaan terputus"
                self._toast(f"{'Gagal menyematkan pesan' if pinned else 'Gagal melepas sematan'}: {reason}", "error")
            if chat == self._chat:
                self._load_pins()  # the server decides what is pinned (it caps the count)
        self._api.fetch("POST", self._path(chat, f"/messages/{self._api.enc(message_id)}/pin"),
                        {"pinned": pinned, "duration": duration}, done)

    @staticmethod
    def _raw_of(row: dict) -> dict:
        return {k: row[k] for k in ("id", "chatId", "from", "to", "content", "timestamp", "status", "type",
                                    "mediaUrl", "senderName", "replyToId", "forwarded", "reactions", "extra",
                                    "starred", "revokedAt")}

    @Slot(str)
    def deleteMessage(self, message_id: str) -> None:
        """Delete for everyone (the only delete the backend has); the message stays, flagged (GPUI `delete_message`)."""
        row = self._row(message_id)
        if not row or not self._chat or not row["isMine"] or row["revokedAt"]:
            return
        chat = self._chat
        self._patch_raw(message_id, {"revokedAt": _now_ms()})

        def done(status: int, data: Any) -> None:
            if status and status < 300:
                self._toast("Pesan berhasil dihapus", "success")
            else:
                self._patch_raw(message_id, {"revokedAt": 0})
                self._toast("Gagal menghapus pesan", "error")
        self._api.fetch("POST", self._path(chat, f"/messages/{self._api.enc(message_id)}/delete"), None, done)

    @Slot(str)
    def copyMessage(self, message_id: str) -> None:
        row = self._row(message_id)
        if row and row["copyText"]:
            self.copyString(row["copyText"], "Pesan disalin ke clipboard")

    @Slot(str)
    def copyString(self, text: str, toast: str = "Disalin ke clipboard") -> None:
        cb = QGuiApplication.clipboard()
        if cb is not None:
            cb.setText(text)
        self._toast(toast, "success")

    @Slot(str)
    def openUrl(self, url: str) -> None:
        if url:
            QDesktopServices.openUrl(QUrl(url))

    @Slot(str, str, result=bool)
    def forwardSend(self, chat_id: str, message_ids: Any, targets: Any = None) -> bool:  # type: ignore[override]
        return False  # replaced below (QVariant overload)

    # ------------------------------------------------------------------ forwarding
    def _forward_request(self, ids: list[str]) -> None:
        chats = self._svc.exposed.get("Chats")
        fn = getattr(chats, "requestForward", None) if chats is not None else None
        if fn is None:
            self._toast("Forwarding is not available", "error")
            return
        fn(self._chat, ",".join(ids))  # several ids are comma separated (WhatsApp ids never contain ',')

    @Slot(str)
    def forward(self, message_id: str) -> None:
        self._forward_request([message_id])

    # ------------------------------------------------------------------ select mode
    def _selected_rows(self) -> list[dict]:
        return [r for r in reversed(self.model.rows()) if r.get("selected")]  # oldest first

    @Slot()
    @Slot(str)
    def enterSelect(self, first_id: str = "") -> None:
        self._set_selected(set())
        self._select = True
        self._confirm_delete = False
        if first_id:
            self.model.patch(first_id, {"selected": True})
        self.selectionChanged.emit()

    @Slot()
    def exitSelect(self) -> None:
        if not self._select:
            return
        self._select = False
        self._confirm_delete = False
        self._set_selected(set())
        self.selectionChanged.emit()

    def _set_selected(self, ids: set[str]) -> None:
        for r in list(self.model.rows()):
            want = r["id"] in ids
            if bool(r.get("selected")) != want:
                self.model.patch(r["id"], {"selected": want})

    @Slot(str)
    def toggleSelect(self, message_id: str) -> None:
        row = self._row(message_id)
        if row is None:
            return
        self.model.patch(message_id, {"selected": not row["selected"]})
        self._confirm_delete = False
        self.selectionChanged.emit()

    @Slot()
    def selectStar(self) -> None:
        sel = self._selected_rows()
        if not sel:
            return
        all_starred = all(r["starred"] for r in sel)
        for r in sel:
            if r["starred"] == all_starred:
                self.toggleStar(r["id"])
        self.exitSelect()

    @Slot()
    def selectCopy(self) -> None:
        sel = self._selected_rows()
        if not sel:
            return
        self.copyString(T.message_lines(sel), "Pesan disalin ke clipboard")
        self.exitSelect()

    @Slot()
    def selectForward(self) -> None:
        sel = self._selected_rows()
        if sel:
            self._forward_request([r["id"] for r in sel])

    @Slot()
    def selectDelete(self) -> None:
        if not self.selectionDeletable:
            return
        if not self._confirm_delete:  # the first click arms it
            self._confirm_delete = True
            self.selectionChanged.emit()
            return
        for r in self._selected_rows():
            self.deleteMessage(r["id"])
        self.exitSelect()

    # ------------------------------------------------------------------ receipts (message info)
    @Slot(str)
    def openInfo(self, message_id: str) -> None:
        row = self._row(message_id)
        if not row or not self._chat:
            return
        chat, gen = self._chat, self._gen
        self._info_open, self._info_loading = True, True
        self._info = self._build_info(row, None, loading=True)
        self.infoChanged.emit()
        state = {"group": None, "receipts": None, "left": 2 if chat.endswith("@g.us") else 1}

        def finish() -> None:
            state["left"] -= 1
            if state["left"] > 0 or gen != self._gen or not self._info_open:
                return
            if state["group"]:
                self._group = state["group"]
            self._info_loading = False
            self._info = self._build_info(row, state["receipts"], loading=False)
            self.infoChanged.emit()

        def got_receipts(status: int, data: Any) -> None:
            state["receipts"] = data if status == 200 and isinstance(data, dict) else {}
            finish()

        def got_group(status: int, data: Any) -> None:
            state["group"] = data if status == 200 and isinstance(data, dict) else None
            finish()
        if chat.endswith("@g.us"):
            self._api.fetch("GET", f"/groups/{self._api.enc(chat)}", None, got_group)
        self._api.fetch("GET", self._path(chat, f"/messages/{self._api.enc(message_id)}/receipts"), None, got_receipts)

    @Slot()
    def closeInfo(self) -> None:
        if self._info_open:
            self._info_open = False
            self.infoChanged.emit()

    def _name_of(self, jid: str) -> str:
        for p in self._participants():
            if p.get("jid") == jid or T.user_of(p.get("jid", "")) == T.user_of(jid):
                if (p.get("name") or "").strip():
                    return p["name"]
        return T.user_of(jid)

    def _build_info(self, row: dict, data: dict | None, loading: bool) -> dict:
        """Sheet content like GPUI `render_message_info_sheet`: read / delivered / remaining members, or the two
        stages of a 1:1 message."""
        data = data or {}
        receipts = data.get("receipts") or []
        recipients = data.get("recipients") or []
        is_group = (row.get("chatId") or "").endswith("@g.us")
        text = T.decode_content(row["content"])
        text = f"[{row['type']}]" if not text.strip() else text
        mk = lambda jid, at: {"jid": jid, "name": self._name_of(jid), "at": at,  # noqa: E731
                              "atText": T.format_receipt_time(at) if at else "", "color": T.avatar_color(jid)}
        read = sorted((r for r in receipts if (r.get("readAt") or 0) > 0), key=lambda r: r["readAt"])
        delivered = sorted((r for r in receipts if not (r.get("readAt") or 0) and (r.get("deliveredAt") or 0) > 0),
                           key=lambda r: r["deliveredAt"])
        reported = {T.user_of(r["jid"]) for r in receipts if (r.get("readAt") or 0) > 0 or (r.get("deliveredAt") or 0) > 0}
        remaining = [j for j in recipients if T.user_of(j) not in reported]
        out = {"loading": loading, "isGroup": is_group, "text": text, "messageId": row["id"],
               "read": [mk(r["jid"], r["readAt"]) for r in read],
               "delivered": [mk(r["jid"], r["deliveredAt"]) for r in delivered],
               "remaining": [mk(j, 0) for j in remaining], "stages": [], "empty": False, "notDelivered": False}
        if is_group:
            out["empty"] = not (read or delivered or remaining)
        else:
            read_at = read[0]["readAt"] if read else 0
            delivered_at = next((r.get("deliveredAt") or r.get("readAt") for r in receipts
                                 if (r.get("deliveredAt") or r.get("readAt") or 0) > 0), 0)
            status = row["status"]
            is_read = read_at > 0 or status == "read"
            is_delivered = is_read or (delivered_at or 0) > 0 or status == "delivered"
            if is_read:
                out["stages"].append({"label": "Read", "blue": True, "at": T.format_receipt_time(read_at) if read_at
                                      else "Time not recorded"})
            if is_delivered:
                out["stages"].append({"label": "Delivered", "blue": False,
                                      "at": T.format_receipt_time(delivered_at) if delivered_at else "Time not recorded"})
            out["notDelivered"] = not is_delivered
        return out

    # ------------------------------------------------------------------ media: gallery / save / copy
    @Slot(result="QVariantList")
    def gallery(self) -> list:
        """Images and videos of the open chat, oldest first, for the viewer."""
        out = []
        for r in reversed(self.model.rows()):
            if r["kind"] in MEDIA_KINDS and r["mediaSrc"] and not r["pending"]:
                out.append({"id": r["id"], "kind": r["kind"], "src": r["mediaSrc"],
                            "name": T.file_name_from_url(r["mediaUrl"], f"{r['id']}.bin"),
                            "caption": r["body"] if r["hasText"] else ""})
        return out

    @Slot(str, result=str)
    def suggestedName(self, message_id: str) -> str:
        r = self._row(message_id)
        if not r:
            return "file"
        if r["kind"] == "document" and r["fileName"]:
            return r["fileName"]
        return T.file_name_from_url(r["mediaUrl"], f"{r['id']}.bin")

    @Slot(str, result=str)
    def mediaLabel(self, message_id: str) -> str:
        r = self._row(message_id)
        return {"image": "Gambar", "video": "Video", "sticker": "Stiker", "audio": "Audio", "document": "Dokumen",
                "gif": "GIF"}.get(r["kind"] if r else "", "File")

    @Slot(str, str, str)
    def saveUrl(self, url: str, dest: str, label: str) -> None:
        """Download `url` and write it to `dest` (a path or file:// URL). Toasts like GPUI `save_file`."""
        path = QUrl(dest).toLocalFile() if dest.startswith("file:") else dest
        if not url or not path:
            return
        reply = self._nam.get(QNetworkRequest(QUrl(url)))

        def done() -> None:
            ok = reply.error() == QNetworkReply.NetworkError.NoError
            data = bytes(reply.readAll()) if ok else b""
            reply.deleteLater()
            if not ok:
                self._toast(f"Gagal mengunduh {label.lower()}", "error")
                self.saved.emit(label, False)
                return

            def write() -> None:
                try:
                    with open(path, "wb") as f:
                        f.write(data)
                    good = True
                except OSError:
                    good = False
                QTimer.singleShot(0, lambda: self._saved(label, good))
            threading.Thread(target=write, daemon=True).start()
        reply.finished.connect(done)

    def _saved(self, label: str, ok: bool) -> None:
        self._toast(f"{label} disimpan" if ok else f"Gagal menyimpan {label.lower()}", "success" if ok else "error")
        self.saved.emit(label, ok)

    @Slot(str, str)
    def saveMedia(self, message_id: str, dest: str) -> None:
        r = self._row(message_id)
        if r and r["mediaSrc"]:
            self.saveUrl(r["mediaSrc"], dest, self.mediaLabel(message_id))

    @Slot(str)
    def copyImage(self, url: str) -> None:
        if not url:
            return
        reply = self._nam.get(QNetworkRequest(QUrl(url)))

        def done() -> None:
            ok = reply.error() == QNetworkReply.NetworkError.NoError
            data = bytes(reply.readAll()) if ok else b""
            reply.deleteLater()
            img = QImage.fromData(data) if data else QImage()
            if not ok:
                self._toast("Gagal mengunduh gambar", "error")
            elif img.isNull():
                self._toast("Format gambar tidak dikenali", "error")
            else:
                cb = QGuiApplication.clipboard()
                if cb is not None:
                    cb.setImage(img)
                self._toast("Gambar disalin ke clipboard", "success")
        reply.finished.connect(done)

    @Slot(str)
    def favoriteSticker(self, message_id: str) -> None:
        r = self._row(message_id)
        if not r or not r["mediaUrl"]:
            return

        def done(status: int, data: Any) -> None:
            ok = 0 < status < 300
            self._toast("Stiker ditambahkan ke favorit" if ok else "Gagal menambahkan stiker",
                        "success" if ok else "error")
        self._api.fetch("POST", "/stickers/favorite", {"secret": "default-secret", "messageId": message_id,
                                                        "mediaUrl": r["mediaUrl"], "isAnimated": False}, done)

    @Slot(str)
    def markViewOnce(self, message_id: str) -> None:
        """View-once media: shown as `Viewed` once opened (local state, like the web client)."""
        r = self._row(message_id)
        if r and r["viewOnce"] and not r["viewOnce"]["viewed"]:
            self.model.patch(message_id, {"viewOnce": {**r["viewOnce"], "viewed": True}})


# Real forwardSend: accepts ids as list or comma-separated string, targets as list. Defined after the class so
# the QVariant signature is explicit.
def _forward_send(self: Messages, chat_id: str, message_ids: Any, targets: Any) -> None:
    ids = [i for i in (message_ids.split(",") if isinstance(message_ids, str) else list(message_ids or [])) if i]
    tgts = [str(t) for t in (targets or [])]
    if not ids or not tgts:
        self.forwardFinished.emit(False)
        return
    state = {"i": 0}

    def step() -> None:
        if state["i"] >= len(ids):
            self._toast("Pesan diteruskan", "success")
            self.forwardFinished.emit(True)
            self.exitSelect()
            return
        mid = ids[state["i"]]
        state["i"] += 1

        def done(status: int, data: Any) -> None:
            if 0 < status < 300:
                step()
            else:
                self._toast("Gagal meneruskan pesan", "error")
                self.forwardFinished.emit(False)
        self._api.fetch("POST", f"/chats/{self._api.enc(chat_id)}/messages/{self._api.enc(mid)}/forward",
                        {"targets": tgts}, done)
    step()


Messages.forwardSend = Slot(str, "QVariant", "QVariant")(  # type: ignore[assignment]
    lambda self, chat_id, ids, targets: _forward_send(self, chat_id, ids, targets))


def register(services) -> None:
    services.expose("Messages", Messages(services))
    # `Messages.model` is a property of the object: QML uses `model: Messages.model`.
