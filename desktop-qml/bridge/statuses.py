"""`Statuses` context property: status stories (list, viewer state, text post, reply, viewed receipts).

Mirrors GPUI `views/status.rs`:
  GET  /statuses                      -> [StatusGroup]    (own group is named "Status Saya")
  POST /statuses/text {text,background}
  POST /statuses/{id}/viewed          POST /statuses/{id}/reply {text} -> {id, chatId}

QML sees: `model` (other people's groups, ring state in `allViewed`), `own`, viewer state
(`viewerOpen/Index/Count/Entries/Entry/Name/Sender/IsOwn/Avatar`), `loading`, `posting`, `replying`.
"""
from __future__ import annotations

from typing import Any

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from .listsync import (
    avatar_color, error_text, initial, ok, status_time, sync_model,
)
from .models import DictListModel

OWN_NAMES = ("Status Saya", "My status")
OWN_KEY = "__own__"
DEFAULT_BG = 0xFF075E54
BACKGROUNDS = [0xFF075E54, 0xFF128C7E, 0xFF777A77, 0xFF2C3E50, 0xFF6A3080, 0xFFC43E00, 0xFFD4A017, 0xFF0E5A8A]

ROLES = ["sender", "name", "displayName", "initial", "color", "avatar", "allViewed", "latestTime",
         "timeLabel", "count", "unviewed"]


def is_own(group: dict) -> bool:
    return group.get("name") in OWN_NAMES or group.get("sender") == "me"


def display_name(group: dict) -> str:
    return group.get("name") or str(group.get("sender", "")).split("@")[0]


class Statuses(QObject):
    loadingChanged = Signal()
    ownChanged = Signal()
    viewerChanged = Signal()
    postingChanged = Signal()
    replyingChanged = Signal()
    errorChanged = Signal()
    unviewedChanged = Signal()
    # QML: open the "New status" composer (tapping an empty "Status Saya")
    composeRequested = Signal()
    # Signals a successful reply so the QML field clears only after the request succeeds.
    replySent = Signal(str)
    posted = Signal()

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._api = services.api
        self._model = DictListModel(ROLES, key="sender", parent=self)
        self._groups: list[dict] = []
        self._own: dict = {}
        self._loading = True
        self._error = ""
        self._posting = False
        self._replying = False
        self._seq = 0
        self._viewer: str | None = None
        self._index = 0
        self._unviewed = 0
        self._loaded_once = False

        services.app.authChanged.connect(self._on_auth)
        services.backend.apiUrlChanged.connect(self._on_backend)
        if services.backend.apiUrl:
            QTimer.singleShot(0, self.refresh)

    # ---------------------------------------------------------------- properties
    @Property(QObject, constant=True)
    def model(self) -> QObject:
        return self._model

    @Property("QVariantMap", notify=ownChanged)
    def own(self) -> dict:
        return self._own

    @Property(bool, notify=loadingChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=errorChanged)
    def error(self) -> str:
        return self._error

    @Property(bool, notify=postingChanged)
    def posting(self) -> bool:
        return self._posting

    @Property(bool, notify=replyingChanged)
    def replying(self) -> bool:
        return self._replying

    @Property(int, notify=unviewedChanged)
    def unviewedCount(self) -> int:
        """Number of people with a status you have not seen (for nav badges)."""
        return self._unviewed

    @Property("QVariantList", constant=True)
    def backgrounds(self) -> list:
        return BACKGROUNDS

    # -- viewer
    def _group(self, key: str | None) -> dict | None:
        if key is None:
            return None
        if key == OWN_KEY:
            return next((g for g in self._groups if is_own(g)), None)
        return next((g for g in self._groups if g.get("sender") == key and not is_own(g)), None)

    def _entries(self) -> list[dict]:
        g = self._group(self._viewer)
        return list(g["statuses"]) if g else []

    @Property(bool, notify=viewerChanged)
    def viewerOpen(self) -> bool:
        return self._viewer is not None and bool(self._entries())

    @Property(int, notify=viewerChanged)
    def viewerIndex(self) -> int:
        return self._index

    @Property(int, notify=viewerChanged)
    def viewerCount(self) -> int:
        return len(self._entries())

    @Property("QVariantList", notify=viewerChanged)
    def viewerEntries(self) -> list:
        return self._entries()

    @Property("QVariantMap", notify=viewerChanged)
    def viewerEntry(self) -> dict:
        ents = self._entries()
        return ents[self._index] if 0 <= self._index < len(ents) else {}

    @Property("QVariantMap", notify=viewerChanged)
    def viewerGroup(self) -> dict:
        """Header info of the open story: sender, displayName, initial, color, avatar, isOwn."""
        g = self._group(self._viewer)
        if not g:
            return {}
        return {"sender": g.get("sender", ""), "displayName": g["displayName"], "initial": g["initial"],
                "color": g["color"], "avatar": g.get("avatar") or "", "isOwn": is_own(g)}

    @Property(bool, notify=viewerChanged)
    def viewerIsOwn(self) -> bool:
        g = self._group(self._viewer)
        return bool(g) and is_own(g)

    # ---------------------------------------------------------------- loading
    def _on_auth(self) -> None:
        if self._svc.app.auth == "authenticated":
            self.refresh()

    def _on_backend(self) -> None:
        if self._svc.backend.apiUrl:
            self.refresh()

    @Slot()
    def refresh(self) -> None:
        self._seq += 1
        seq = self._seq
        self._api.fetch("GET", "/statuses", None, lambda st, body, s=seq: self._loaded(s, st, body))

    def _loaded(self, seq: int, status: int, body: Any) -> None:
        if seq != self._seq:
            return  # a newer refresh superseded this one
        self._set_loading(False)
        if not ok(status) or not isinstance(body, list):
            # GPUI: keep what we had on failure; surface the failure for the empty state only
            self._set_error(error_text(status, body, "Could not load statuses") if not self._loaded_once else "")
            return
        self._loaded_once = True
        self._set_error("")
        self._groups = [self._normalise(g) for g in body if isinstance(g, dict)]
        self._publish()

    def _normalise(self, g: dict) -> dict:
        sender = str(g.get("sender", ""))
        name = g.get("name") or ""
        out = dict(g)
        out["sender"] = sender
        out["name"] = name
        out["displayName"] = display_name(g)
        out["initial"] = initial(out["displayName"])
        out["color"] = avatar_color(sender)
        out["avatar"] = g.get("avatar") or ""
        out["statuses"] = [self._entry(e) for e in (g.get("statuses") or []) if isinstance(e, dict)]
        return out

    def _entry(self, e: dict) -> dict:
        typ = e.get("type") or "text"
        sid = str(e.get("id", ""))
        media = e.get("mediaUrl") or ""
        src = self._api.mediaUrl(media) if media else ""
        content = (e.get("content") or "")
        caption = "" if (typ != "text" and content.startswith("[") and content.endswith("]")) else content
        out = dict(e)
        out.update({"id": sid, "type": typ, "src": src, "caption": caption, "content": content,
                    "viewed": bool(e.get("viewed")), "timeLabel": status_time(e.get("timestamp", 0))})
        return out

    def _publish(self) -> None:
        """Recompute the derived rows (model, own, counters, viewer) from `_groups`."""
        rows = []
        own: dict = {}
        for g in self._groups:
            ents = g["statuses"]
            latest = max([int(g.get("latestTime") or 0)] + [int(e.get("timestamp") or 0) for e in ents])
            all_viewed = bool(g.get("allViewed")) if not ents else all(e["viewed"] for e in ents)
            row = {
                "sender": g["sender"], "name": g["name"], "displayName": g["displayName"],
                "initial": g["initial"], "color": g["color"], "avatar": g["avatar"],
                "allViewed": all_viewed, "latestTime": latest, "timeLabel": status_time(latest),
                "count": len(ents), "unviewed": sum(1 for e in ents if not e["viewed"]),
            }
            g["allViewed"], g["latestTime"] = all_viewed, latest
            if is_own(g):
                own = row
            else:
                rows.append(row)
        sync_model(self._model, rows, "sender")
        if own != self._own:
            self._own = own
            self.ownChanged.emit()
        n = sum(1 for r in rows if not r["allViewed"])
        if n != self._unviewed:
            self._unviewed = n
            self.unviewedChanged.emit()
        # keep the viewer valid: its group may have gone or shrunk
        if self._viewer is not None:
            count = len(self._entries())
            if count == 0:
                self._viewer, self._index = None, 0
            else:
                self._index = min(self._index, count - 1)
        self.viewerChanged.emit()

    def _set_loading(self, v: bool) -> None:
        if v != self._loading:
            self._loading = v
            self.loadingChanged.emit()

    def _set_error(self, msg: str) -> None:
        if msg != self._error:
            self._error = msg
            self.errorChanged.emit()

    # ---------------------------------------------------------------- viewer actions
    @Slot(str)
    def openGroup(self, sender: str) -> None:
        """Open a person's statuses; the own row (`""`/`__own__`) opens the composer when empty (GPUI)."""
        key = OWN_KEY if sender in ("", OWN_KEY) else sender
        g = self._group(key)
        if g is None or not g["statuses"]:
            if key == OWN_KEY:
                self.composeRequested.emit()
            return
        self._viewer, self._index = key, 0
        self.viewerChanged.emit()
        self._mark_current()

    @Slot()
    def openOwn(self) -> None:
        self.openGroup(OWN_KEY)

    @Slot()
    def close(self) -> None:
        """Close the viewer and reload the list (GPUI `Tutup`)."""
        if self._viewer is None:
            return
        self._viewer, self._index = None, 0
        self.viewerChanged.emit()
        self.refresh()

    @Slot(int)
    def goTo(self, index: int) -> None:
        """Jump to story `index`; past either end closes the viewer (GPUI `navigate`)."""
        if self._viewer is None:
            return
        if index < 0 or index >= len(self._entries()):
            self.close()
            return
        self._index = index
        self.viewerChanged.emit()
        self._mark_current()

    @Slot()
    def next(self) -> None:
        self.goTo(self._index + 1)

    @Slot()
    def prev(self) -> None:
        # GPUI closes on prev from the first story as well (navigate(-1) out of range)
        self.goTo(self._index - 1)

    def _mark_current(self) -> None:
        g = self._group(self._viewer)
        if not g or is_own(g):
            return
        ents = g["statuses"]
        if not 0 <= self._index < len(ents):
            return
        e = ents[self._index]
        if e["viewed"]:
            return
        e["viewed"] = True  # optimistic: the ring updates now; the receipt is fire-and-forget
        self._api.fetch("POST", f"/statuses/{self._api.enc(e['id'])}/viewed", None, None)
        self._publish()

    # ---------------------------------------------------------------- reply
    @Slot(str)
    def reply(self, text: str) -> None:
        text = (text or "").strip()
        g = self._group(self._viewer)
        ents = g["statuses"] if g else []
        if not text or self._replying or not g or is_own(g) or not 0 <= self._index < len(ents):
            return
        sid = ents[self._index]["id"]
        self._replying = True
        self.replyingChanged.emit()

        def done(status: int, body: Any) -> None:
            self._replying = False
            self.replyingChanged.emit()
            if ok(status):
                chat = (body or {}).get("chatId", "") if isinstance(body, dict) else ""
                self._svc.toast.success("Balasan terkirim")
                self.replySent.emit(chat)
            else:
                self._svc.toast.error(f"Failed to reply: {error_text(status, body)}")

        self._api.fetch("POST", f"/statuses/{self._api.enc(sid)}/reply", {"text": text}, done)

    # ---------------------------------------------------------------- posting
    def _set_posting(self, v: bool) -> None:
        self._posting = v
        self.postingChanged.emit()

    @Slot(str, float)
    def postText(self, text: str, background: int = DEFAULT_BG) -> None:
        text = (text or "").strip()
        if not text or self._posting:
            return
        self._set_posting(True)

        def done(status: int, body: Any) -> None:
            self._set_posting(False)
            if ok(status):
                self._svc.toast.success("Status posted")
                self.posted.emit()
                self.refresh()
            else:
                self._svc.toast.error(f"Failed to post: {error_text(status, body)}")

        self._api.fetch("POST", "/statuses/text", {"text": text, "background": int(background) or DEFAULT_BG}, done)

def register(services) -> None:
    services.statuses = Statuses(services)
    services.expose("Statuses", services.statuses)
