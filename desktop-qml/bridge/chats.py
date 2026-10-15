"""`Chats` (QML global): the chat list store + chat actions + groups + sheet data.

Mirrors GPUI's chat-list half of `state/chat.rs` (`ChatStore`) and `views/chat/{chat_actions,data/loading}.rs`.
  * `model`   every chat (REST `/chats` fields + derived `typing snippet timeLabel muted pinned`), kept sorted
              pinned-first (most recently pinned first) then newest last message first.
  * `list`    filtered view for the sidebar (`archivedMode`, `query`, `kind`, `count`, `pinnedCount`).
  * `activeChat` / `get(id)` the chat row as a dict; `group` the open group's `GroupCache`.
  * WS: new_message chat_update chat_state chat_name_update chats_changed chat_deleted chat_cleared
        chat_presence (typing) presence message_starred group_updated auth_success logged_out.
Message-level state lives in `Messages`; this module never touches it, it only reports unread counters.
"""
from __future__ import annotations

import json
import os
import threading
import time
from pathlib import Path
from typing import Any, Callable
from urllib.parse import quote

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from . import chatfmt
from .chatlist import ROLES, ChatFilterModel, derive, normalize, sort_key
from .models import DictListModel

TYPING_TTL = 6.0
PAGE_INFO = 30
STARRED_PAGE = 100


def enc(s: str) -> str:
    return quote(s, safe="")


def _err(body: Any) -> str:
    if isinstance(body, dict):
        return str(body.get("error") or body.get("message") or "")
    return body if isinstance(body, str) else ""


class Chats(QObject):
    # state
    loadedChanged = Signal()
    loadingChanged = Signal()
    activeChatChanged = Signal()
    statsChanged = Signal()
    groupChanged = Signal()
    typingChanged = Signal(str)            # chatId
    # events for other modules / QML
    forwardRequested = Signal(str, str)    # chatId, messageId
    forwardManyRequested = Signal(str, list)
    sheetRequested = Signal(str, str)      # name ("info"|"search"|"starred"|"media"), chatId
    namesChanged = Signal()                # contact names re-resolved: refetch what is on screen
    chatsReloaded = Signal()
    starredVersionChanged = Signal()
    jumpRequested = Signal(str, str)       # chatId, messageId (after a starred/search result was opened)
    # async action results
    createGroupFinished = Signal(bool, str)
    joinGroupFinished = Signal(bool)
    addMembersFinished = Signal(bool)
    forwardFinished = Signal(bool)
    exportFinished = Signal(bool, str)
    exportingChanged = Signal()
    _export_cb = Signal(bool, str)         # worker thread -> UI thread
    # sheet data
    infoChanged = Signal()
    searchChanged = Signal()
    starredChanged = Signal()

    def __init__(self, api, hub, app, toast=None, backend=None, settings=None, parent: QObject | None = None):
        super().__init__(parent)
        self._api, self._hub, self._app, self._toast, self._backend = api, hub, app, toast, backend
        self._settings = settings
        self.rows = DictListModel(ROLES, key="id", parent=self)
        self.view = ChatFilterModel(self.rows, self)
        self.view.statsChanged.connect(self.statsChanged)
        self._export_cb.connect(self._export_done)
        self._loaded = False
        self._loading = False
        self._reload_again = False
        self._retries = 0
        self._typing: dict[str, dict[str, dict[str, Any]]] = {}
        self._presence: dict[str, str] = {}
        self._groups: dict[str, dict[str, Any]] = {}
        self._group_loading: set[str] = set()
        self._info = {"media": [], "docs": [], "links": [], "loading": False, "chat": ""}
        self._search = {"results": [], "busy": False, "seq": 0, "query": ""}
        self._starred = {"rows": [], "more": False, "busy": False}
        self._starred_version = 0
        self._exporting = False
        self._last_active = ""
        # timers
        self._typing_timer = QTimer(self)
        self._typing_timer.setInterval(1000)
        self._typing_timer.timeout.connect(self._sweep_typing)
        self._reload_timer = QTimer(self)
        self._reload_timer.setSingleShot(True)
        self._reload_timer.setInterval(150)
        self._reload_timer.timeout.connect(self.reload)
        # wiring
        hub.event.connect(self._on_event)
        hub.connectedChanged.connect(self._on_connected)
        app.activeChatChanged.connect(self._on_active_changed)
        app.windowVisibleChanged.connect(self._on_visibility)
        app.authChanged.connect(self._on_auth)
        if backend is not None:
            backend.apiUrlChanged.connect(self._on_backend)
            if backend.apiUrl:
                QTimer.singleShot(0, self.ensureLoaded)

    # ------------------------------------------------------------------ properties
    @Property(QObject, constant=True)
    def model(self) -> QObject:
        """Every chat, sorted pinned-first then newest-first (contract role names, see chatlist.ROLES)."""
        return self.rows

    @Property(QObject, constant=True)
    def filtered(self) -> QObject:
        """Sidebar view: `archivedMode`, `query`, `kind`, `count`, `pinnedCount` (also the `ChatList` global)."""
        return self.view

    @Property(bool, notify=loadedChanged)
    def loaded(self) -> bool:
        return self._loaded

    @Property(bool, notify=loadingChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(int, notify=statsChanged)
    def archivedCount(self) -> int:
        return sum(1 for r in self.rows.rows() if r.get("archived"))

    @Property(int, notify=statsChanged)
    def totalUnread(self) -> int:
        return sum(r.get("unread", 0) for r in self.rows.rows() if not r.get("archived") and r.get("muteMode") in ("", "off"))

    @Property("QVariantMap", notify=activeChatChanged)
    def activeChat(self) -> dict:
        return dict(self.rows.get_row(self._app.activeChatId) or {})

    @Property(str, notify=activeChatChanged)
    def activeChatId(self) -> str:
        return self._app.activeChatId

    @Property("QVariantMap", notify=groupChanged)
    def group(self) -> dict:
        """`GroupCache` of the open chat when it is a group ({} otherwise / until loaded)."""
        g = self._groups.get(self._app.activeChatId)
        return dict(g) if g else {}

    @Property(bool, notify=groupChanged)
    def groupLoading(self) -> bool:
        return self._app.activeChatId in self._group_loading

    @Property("QVariantList", notify=infoChanged)
    def infoMedia(self) -> list:
        return self._info["media"]

    @Property("QVariantList", notify=infoChanged)
    def infoDocs(self) -> list:
        return self._info["docs"]

    @Property("QVariantList", notify=infoChanged)
    def infoLinks(self) -> list:
        return self._info["links"]

    @Property(bool, notify=infoChanged)
    def infoLoading(self) -> bool:
        return self._info["loading"]

    @Property("QVariantList", notify=searchChanged)
    def searchResults(self) -> list:
        return self._search["results"]

    @Property(bool, notify=searchChanged)
    def searching(self) -> bool:
        return self._search["busy"]

    @Property("QVariantList", notify=starredChanged)
    def starred(self) -> list:
        return self._starred["rows"]

    @Property(bool, notify=starredChanged)
    def starredHasMore(self) -> bool:
        return self._starred["more"]

    @Property(bool, notify=starredChanged)
    def starredLoading(self) -> bool:
        return self._starred["busy"]

    @Property(int, notify=starredVersionChanged)
    def starredVersion(self) -> int:
        return self._starred_version

    @Property(bool, notify=exportingChanged)
    def exporting(self) -> bool:
        return self._exporting

    # ------------------------------------------------------------------ helpers
    def _say(self, msg: str, kind: str = "info") -> None:
        if self._toast is not None:
            self._toast.show(msg, kind)

    def _call(self, method: str, path: str, body: Any = None, cb: Callable[[int, Any], None] | None = None) -> None:
        self._api.fetch(method, path, body, cb)

    def _touch(self, chat_id: str) -> None:
        if chat_id and chat_id == self._app.activeChatId:
            self.activeChatChanged.emit()

    def _resort(self) -> None:
        self.rows.sort_by(sort_key)
        self.statsChanged.emit()

    def _patch(self, chat_id: str, fields: dict[str, Any], resort: bool = True) -> bool:
        row = self.rows.get_row(chat_id)
        if row is None:
            return False
        merged = {**row, **fields}
        fields = {**fields, **derive(merged)}
        self.rows.patch(chat_id, fields)
        if resort:
            self._resort()
        self._touch(chat_id)
        return True

    def _put(self, row: dict[str, Any], at_top: bool = False) -> None:
        r = normalize(row)
        r["typing"] = ""
        r.update(derive(r))
        if at_top:
            self.rows.insert(0, r)
        else:
            self.rows.upsert(r)
        self._resort()

    @Slot(str, result="QVariant")
    def get(self, chat_id: str):
        return self.rows.get_row(chat_id)

    @Slot(str, result=bool)
    def has(self, chat_id: str) -> bool:
        return self.rows.contains(chat_id)

    # ------------------------------------------------------------------ loading
    @Slot()
    def ensureLoaded(self) -> None:
        if not self._loaded and not self._loading:
            self.reload()

    @Slot()
    def reload(self) -> None:
        """GET /chats -> model (`ChatView::load_chats` + `ChatStore::set_chats`)."""
        if self._loading:
            self._reload_again = True
            return
        if self._backend is not None and not self._backend.apiUrl:
            return
        self._loading = True
        self.loadingChanged.emit()
        self._call("GET", "/chats", None, self._on_chats)

    def _on_chats(self, status: int, body: Any) -> None:
        self._loading = False
        self.loadingChanged.emit()
        if status != 200 and not self._loaded and self._retries < 5 and self._app.auth != "unauthenticated":
            self._retries += 1  # backend not ready / not linked yet
            QTimer.singleShot(2000, self.ensureLoaded)
        if status == 200 and isinstance(body, list):
            self._retries = 0
            rows = []
            for d in body:
                if isinstance(d, dict) and d.get("id"):
                    r = normalize(d)
                    r["typing"] = self._typing_label_for(r["id"], r["isGroup"])
                    r.update(derive(r))
                    rows.append(r)
            rows.sort(key=sort_key)
            self.rows.reset(rows)
            self._loaded = True
            self.loadedChanged.emit()
            self.statsChanged.emit()
            self.chatsReloaded.emit()
            self.activeChatChanged.emit()
        if self._reload_again:
            self._reload_again = False
            self._reload_timer.start()

    def _on_backend(self) -> None:
        if self._backend.apiUrl:
            self.ensureLoaded()

    def _on_connected(self) -> None:
        if self._hub.connected and self._loaded:
            self.reload()  # events may have been missed while the socket was down

    def _on_auth(self) -> None:
        if self._app.auth == "authenticated":
            self.reload()
        elif self._app.auth == "unauthenticated":
            self._clear()

    def _clear(self) -> None:
        self.rows.reset([])
        self._typing.clear()
        self._groups.clear()
        self._loaded = False
        self.loadedChanged.emit()
        self.statsChanged.emit()
        self.activeChatChanged.emit()

    # ------------------------------------------------------------------ active chat
    def _on_active_changed(self) -> None:
        cid = self._app.activeChatId
        self.activeChatChanged.emit()
        self.groupChanged.emit()
        # per-conversation sheet data belongs to the previous chat
        self._info.update(media=[], docs=[], links=[], loading=False, chat="")
        self._search.update(results=[], busy=False, query="", seq=self._search["seq"] + 1)
        self.infoChanged.emit()
        self.searchChanged.emit()
        if not cid:
            return
        if not cid.endswith("@g.us"):
            # One-to-one typing indicators only arrive for chats we subscribed to; groups send them anyway.
            self._call("POST", f"/chats/{enc(cid)}/presence-subscribe")
        self.markRead(cid)
        if cid.endswith("@g.us"):
            self.loadGroup(cid)

    def _on_visibility(self) -> None:
        cid = self._app.activeChatId
        if cid and self._reading(cid):
            row = self.rows.get_row(cid)
            if row and (row.get("unread") or row.get("hasUnreadMention")):
                self.markRead(cid)

    def _reading(self, chat_id: str) -> bool:
        """The conversation counts as read only while it is the open one in a visible window on the chat page."""
        return bool(self._app.windowVisible and self._app.section == "chat" and self._app.activeChatId == chat_id)

    @Slot(str)
    def open(self, chat_id: str) -> None:
        self._app.openChat(chat_id)

    @Slot()
    def backToList(self) -> None:
        self._app.openChat("")

    @Slot(str, str)
    def openDirect(self, jid: str, name: str = "") -> None:
        """Open the 1:1 chat with `jid`, adding an empty row when there is no history yet (clicked mention)."""
        if not self.rows.contains(jid):
            label = name.strip() or jid.split("@", 1)[0]
            self._put({"id": jid, "name": label, "isActive": True}, at_top=True)
        self._app.openChat(jid)

    # ------------------------------------------------------------------ chat actions
    @Slot(str)
    def markRead(self, chat_id: str) -> None:
        """Local unread badge off + POST /chats/{id}/read (`mark_chat_read_action`)."""
        row = self.rows.get_row(chat_id)
        if row is not None and (row.get("unread") or row.get("hasUnreadMention")):
            self._patch(chat_id, {"unread": 0, "hasUnreadMention": False}, resort=False)
            self.statsChanged.emit()
        self._call("POST", f"/chats/{enc(chat_id)}/read")

    @Slot()
    def markAllRead(self) -> None:
        unread = [r["id"] for r in self.rows.rows() if r.get("unread")]
        if not unread:
            self._say("Tidak ada pesan belum dibaca", "success")
        for cid in unread:
            self.markRead(cid)

    def _apply_state(self, state: Any) -> None:
        if isinstance(state, dict) and state.get("chatId"):
            self._patch(state["chatId"], {
                "archived": bool(state.get("archived")), "pinnedAt": state.get("pinnedAt"),
                "muteMode": state.get("muteMode") or "off", "mutedUntil": state.get("mutedUntil")})

    @Slot(str, bool)
    def setPinned(self, chat_id: str, pinned: bool) -> None:
        self._call("POST", f"/chats/{enc(chat_id)}/pin", {"pinned": pinned},
                   lambda st, b: self._apply_state(b) if st == 200 else None)

    @Slot(str)
    def togglePin(self, chat_id: str) -> None:
        row = self.rows.get_row(chat_id) or {}
        self.setPinned(chat_id, row.get("pinnedAt") is None)

    @Slot(str, bool)
    def setArchived(self, chat_id: str, archived: bool) -> None:
        ok_msg = "Chat berhasil diarsipkan" if archived else "Chat dikeluarkan dari arsip"
        fail = "Gagal mengarsipkan chat" if archived else "Gagal mengeluarkan chat dari arsip"

        def done(st: int, body: Any) -> None:
            if st == 200:
                self._apply_state(body)
                self._say(ok_msg, "success")
            else:
                self._say(f"{fail}: {_err(body) or st}", "error")
                self.reload()

        self._call("POST", f"/chats/{enc(chat_id)}/archive", {"archived": archived}, done)

    @Slot(str)
    def toggleArchive(self, chat_id: str) -> None:
        row = self.rows.get_row(chat_id) or {}
        self.setArchived(chat_id, not row.get("archived"))

    @Slot(str, str)
    def setMute(self, chat_id: str, mode: str) -> None:
        """mode: off | 8h | 1w | forever"""
        self._call("POST", f"/chats/{enc(chat_id)}/mute", {"mode": mode},
                   lambda st, b: self._apply_state(b) if st == 200 else None)

    @Slot(str, int)
    def setDisappearing(self, chat_id: str, seconds: int) -> None:
        """Shows at once; a failed request puts the old value back (`set_disappearing_timer`)."""
        row = self.rows.get_row(chat_id)
        previous = (row or {}).get("disappearingSecs", 0)
        if previous == seconds:
            return
        self._patch(chat_id, {"disappearingSecs": seconds}, resort=False)

        def done(st: int, body: Any) -> None:
            if st == 200:
                self._say("Pesan sementara dimatikan" if seconds == 0
                          else f"Pesan sementara aktif: {chatfmt.disappearing_label(seconds)}", "success")
            else:
                self._patch(chat_id, {"disappearingSecs": previous}, resort=False)
                self._say(f"Gagal mengubah pesan sementara: {_err(body) or 'permintaan terputus'}", "error")

        self._call("POST", f"/chats/{enc(chat_id)}/disappearing", {"seconds": seconds}, done)

    @Slot(str)
    def leaveGroup(self, chat_id: str) -> None:
        self._call("POST", f"/groups/{enc(chat_id)}/leave", None, lambda st, b: self.reload() if st == 200 else None)

    @Slot()
    def logout(self) -> None:
        """Ask the auth module to log out; falls back to POST /logout (GPUI `open_logout`)."""
        auth = getattr(self, "_auth_obj", None)
        if auth is not None and hasattr(auth, "logout"):
            auth.logout()
            return

        def done(st: int, body: Any) -> None:
            self._app.set_auth("unauthenticated")
            self._app.navigate("chat")

        self._call("POST", "/logout", None, done)

    def attach_auth(self, auth: Any) -> None:
        self._auth_obj = auth

    # ------------------------------------------------------------------ requests to other modules
    @Slot(str, str)
    def requestForward(self, chat_id: str, message_id: str) -> None:
        self.forwardRequested.emit(chat_id, message_id)

    @Slot(str, "QVariantList")
    def requestForwardMany(self, chat_id: str, message_ids: list) -> None:
        self.forwardManyRequested.emit(chat_id, [str(i) for i in message_ids])

    @Slot(str, str)
    def requestSheet(self, name: str, chat_id: str) -> None:
        if chat_id and chat_id != self._app.activeChatId and name != "starred":
            self._app.openChat(chat_id)
        self.sheetRequested.emit(name, chat_id)

    @Slot(str, "QVariantList", "QVariantList")
    def forwardMessages(self, chat_id: str, message_ids: list, targets: list) -> None:
        """Forward each message (in order) to every target chat (`submit_forward`)."""
        ids, tgt = [str(i) for i in message_ids], [str(t) for t in targets]
        if not ids or not tgt:
            return

        def step(i: int) -> None:
            if i >= len(ids):
                self._say("Pesan diteruskan", "success")
                self.forwardFinished.emit(True)
                return

            def done(st: int, body: Any) -> None:
                if st == 200:
                    step(i + 1)
                else:
                    self._say("Gagal meneruskan pesan", "error")
                    self.forwardFinished.emit(False)

            self._call("POST", f"/chats/{enc(chat_id)}/messages/{enc(ids[i])}/forward", {"targets": tgt}, done)

        step(0)

    # ------------------------------------------------------------------ groups
    @Slot(str)
    def loadGroup(self, jid: str) -> None:
        if not jid or jid in self._group_loading:
            return
        self._group_loading.add(jid)
        self.groupChanged.emit()

        def done(st: int, body: Any) -> None:
            self._group_loading.discard(jid)
            if st == 200 and isinstance(body, dict):
                self._groups[jid] = body
            self.groupChanged.emit()

        self._call("GET", f"/groups/{enc(jid)}", None, done)

    @Slot(str, result="QVariant")
    def getGroup(self, jid: str):
        return self._groups.get(jid)

    @Slot(str, "QVariantList")
    def createGroup(self, name: str, participants: list) -> None:
        name = name.strip()
        if not name:
            self._say("Nama grup wajib diisi", "error")
            self.createGroupFinished.emit(False, "")
            return

        def done(st: int, body: Any) -> None:
            if st == 200:
                self._say("Grup berhasil dibuat", "success")
                self.reload()
                jid = (body or {}).get("group", {}).get("jid", "") if isinstance(body, dict) else ""
                self.createGroupFinished.emit(True, jid)
            else:
                self._say("Gagal membuat grup", "error")
                self.createGroupFinished.emit(False, "")

        self._call("POST", "/groups/create", {"name": name, "participants": [str(p) for p in participants]}, done)

    @Slot(str)
    def joinGroup(self, link: str) -> None:
        link = link.strip()
        if not link:
            self._say("Tautan undangan wajib diisi", "error")
            self.joinGroupFinished.emit(False)
            return

        def done(st: int, body: Any) -> None:
            if st == 200:
                self._say("Berhasil bergabung ke grup", "success")
                self.reload()
            else:
                self._say("Gagal bergabung ke grup", "error")
            self.joinGroupFinished.emit(st == 200)

        self._call("POST", "/groups/join", {"link": link}, done)

    @Slot(str, "QVariantList")
    def addMembers(self, group_id: str, jids: list) -> None:
        jids = [str(j) for j in jids]
        if not jids:
            self._say("Pilih minimal satu kontak", "error")
            self.addMembersFinished.emit(False)
            return

        def done(st: int, body: Any) -> None:
            if st == 200 and isinstance(body, dict):
                self._groups[body.get("jid") or group_id] = body
                self.groupChanged.emit()
                self._say("Anggota ditambahkan", "success")
            else:
                self._say("Gagal menambahkan anggota", "error")
            self.addMembersFinished.emit(st == 200)

        self._call("POST", f"/groups/{enc(group_id)}/participants", {"action": "add", "jids": jids}, done)

    # ------------------------------------------------------------------ info sheet lists
    @Slot(str)
    def loadInfo(self, chat_id: str) -> None:
        """Media / docs / links of a chat (first 30 each) + the group's member list."""
        if not chat_id:
            return
        self._info.update(media=[], docs=[], links=[], loading=True, chat=chat_id)
        self.infoChanged.emit()
        pending = {"n": 3}

        def make(key: str):
            def done(st: int, body: Any) -> None:
                if self._info["chat"] != chat_id:
                    return
                if st == 200 and isinstance(body, list):
                    self._info[key] = body
                pending["n"] -= 1
                if pending["n"] == 0:
                    self._info["loading"] = False
                self.infoChanged.emit()
            return done

        for key, path in (("media", "media"), ("docs", "docs"), ("links", "links")):
            self._call("GET", f"/chats/{enc(chat_id)}/{path}?limit={PAGE_INFO}", None, make(key))
        if chat_id.endswith("@g.us"):
            self.loadGroup(chat_id)

    # ------------------------------------------------------------------ in-chat search
    @Slot(str, str)
    def search(self, chat_id: str, query: str) -> None:
        query = query.strip()
        seq = self._search["seq"] = self._search["seq"] + 1
        self._search["query"] = query
        if not query or not chat_id:
            self._search.update(results=[], busy=False)
            self.searchChanged.emit()
            return
        self._search["busy"] = True
        self.searchChanged.emit()

        def done(st: int, body: Any) -> None:
            if seq != self._search["seq"]:
                return  # a newer query is in flight
            self._search["busy"] = False
            if st == 200 and isinstance(body, list):
                self._search["results"] = sorted(body, key=lambda m: -int(m.get("timestamp") or 0))
            self.searchChanged.emit()

        self._call("GET", f"/chats/{enc(chat_id)}/search?q={enc(query)}&limit=50", None, done)

    @Slot()
    def clearSearch(self) -> None:
        self._search.update(results=[], busy=False, query="", seq=self._search["seq"] + 1)
        self.searchChanged.emit()

    # ------------------------------------------------------------------ starred sheet
    @Slot(bool)
    def loadStarred(self, more: bool = False) -> None:
        if self._starred["busy"]:
            return
        rows = self._starred["rows"]
        before = rows[-1].get("starredAt") if (more and rows) else None
        if more and before is None:
            return
        self._starred["busy"] = True
        self.starredChanged.emit()
        path = f"/starred?limit={STARRED_PAGE}" + (f"&before={before}" if before is not None else "")

        def done(st: int, body: Any) -> None:
            self._starred["busy"] = False
            if st == 200 and isinstance(body, list):
                self._starred["more"] = len(body) >= STARRED_PAGE
                if more:
                    known = {m.get("id") for m in self._starred["rows"]}
                    self._starred["rows"] = self._starred["rows"] + [m for m in body if m.get("id") not in known]
                else:
                    self._starred["rows"] = body
            else:
                self._say("Gagal memuat pesan berbintang", "error")
            self.starredChanged.emit()

        self._call("GET", path, None, done)

    @Slot(str, str)
    def unstar(self, chat_id: str, message_id: str) -> None:
        """Unstar from the starred list: it leaves at once; a failed request reloads the list."""
        self._starred["rows"] = [m for m in self._starred["rows"] if m.get("id") != message_id]
        self.starredChanged.emit()

        def done(st: int, body: Any) -> None:
            if st != 200:
                self._starred["busy"] = False
                self.loadStarred(False)

        self._call("POST", f"/chats/{enc(chat_id)}/messages/{enc(message_id)}/star", {"starred": False}, done)

    @Slot(str, str)
    def openMessage(self, chat_id: str, message_id: str) -> None:
        """Jump to a message (search result / starred list), opening its chat first when it is another one."""
        self.jumpRequested.emit(chat_id, message_id)

    # ------------------------------------------------------------------ typing
    def _typing_entries(self, chat_id: str) -> list[tuple[str, str, bool]]:
        now = time.monotonic()
        entries = self._typing.get(chat_id) or {}
        fresh = sorted(((s, e) for s, e in entries.items() if now - e["at"] < TYPING_TTL), key=lambda x: x[1]["at"])
        out = []
        for sender, e in fresh:
            name = e.get("name")
            if not name:
                row = self.rows.get_row(sender)
                name = (row or {}).get("name") or sender.split("@", 1)[0]
            out.append((sender, name, bool(e.get("audio"))))
        return out

    def _typing_label_for(self, chat_id: str, is_group: bool) -> str:
        return chatfmt.typing_label(self._typing_entries(chat_id), is_group)

    @Slot(str, result="QVariantList")
    def typers(self, chat_id: str) -> list:
        return [{"jid": j, "name": n, "audio": a} for j, n, a in self._typing_entries(chat_id)]

    @Slot(str, result=str)
    def peerPresence(self, chat_id: str) -> str:
        return self._presence.get(chat_id, "")

    def _set_typing(self, chat_id: str, sender: str, name: str | None, composing: bool, audio: bool) -> None:
        sender = sender or chat_id
        if composing:
            self._typing.setdefault(chat_id, {})[sender] = {"name": name, "audio": audio, "at": time.monotonic()}
            if not self._typing_timer.isActive():
                self._typing_timer.start()
        else:
            entries = self._typing.get(chat_id)
            if entries is not None:
                entries.pop(sender, None)
                if not entries:
                    del self._typing[chat_id]
        self._refresh_typing(chat_id)

    def _refresh_typing(self, chat_id: str) -> None:
        row = self.rows.get_row(chat_id)
        if row is not None:
            label = self._typing_label_for(chat_id, bool(row.get("isGroup")))
            if label != row.get("typing"):
                self.rows.patch(chat_id, {"typing": label})
        self.typingChanged.emit(chat_id)

    def _sweep_typing(self) -> None:
        now = time.monotonic()
        for cid in list(self._typing):
            entries = self._typing[cid]
            for s in [s for s, e in entries.items() if now - e["at"] >= TYPING_TTL]:
                del entries[s]
            if not entries:
                del self._typing[cid]
            self._refresh_typing(cid)
        if not self._typing:
            self._typing_timer.stop()

    # ------------------------------------------------------------------ WebSocket
    def _on_event(self, typ: str, payload_json: str) -> None:
        try:
            p = json.loads(payload_json) if payload_json else {}
        except ValueError:
            return
        if not isinstance(p, dict):
            return
        h = {
            "new_message": self._ev_new_message, "chat_update": self._ev_chat_update,
            "chat_state": lambda p: self._apply_state(p), "chat_name_update": self._ev_name,
            "chats_changed": self._ev_chats_changed, "chat_deleted": self._ev_deleted,
            "chat_cleared": self._ev_cleared, "chat_presence": self._ev_typing, "presence": self._ev_presence,
            "message_starred": self._ev_starred, "group_updated": self._ev_group, "auth_success": self._ev_auth,
            "logged_out": lambda p: self._clear(),
        }.get(typ)
        if h:
            h(p)

    def _ev_auth(self, p: dict) -> None:
        self.reload()

    def _ev_new_message(self, m: dict) -> None:
        chat_id = m.get("chatId") or ""
        if not chat_id:
            return
        incoming = m.get("status") == "received"
        extra = m.get("extra") or {}
        mentions_me = incoming and bool(extra.get("mentionsMe"))
        reading = self._reading(chat_id)
        unread_here = incoming and not reading
        if incoming and reading:
            self._call("POST", f"/chats/{enc(chat_id)}/read")
        content = m.get("content") or ""
        ts = int(m.get("timestamp") or 0)
        row = self.rows.get_row(chat_id)
        if row is not None:
            f: dict[str, Any] = {"lastMsg": content, "lastTime": ts}
            if unread_here:
                f["unread"] = row.get("unread", 0) + 1
                f["hasUnreadMention"] = bool(row.get("hasUnreadMention") or mentions_me)
            self._patch(chat_id, f)
        else:
            self._put({"id": chat_id, "name": m.get("senderName") or chat_id, "lastMsg": content, "lastTime": ts,
                       "unread": 1 if unread_here else 0, "hasUnreadMention": unread_here and mentions_me,
                       "isActive": True, "isGroup": "@g.us" in chat_id}, at_top=True)
        self.statsChanged.emit()

    def _ev_chat_update(self, p: dict) -> None:
        cid = p.get("chatId") or ""
        row = self.rows.get_row(cid)
        if row is None:
            return
        f: dict[str, Any] = {}
        if isinstance(p.get("unread"), (int, float)):
            f["unread"] = int(p["unread"])
            if f["unread"] == 0:
                f["hasUnreadMention"] = False
        if isinstance(p.get("lastMsg"), str):
            f["lastMsg"] = p["lastMsg"]
        if isinstance(p.get("disappearingSecs"), (int, float)):
            f["disappearingSecs"] = int(p["disappearingSecs"])
        if f:
            self._patch(cid, f, resort=False)
            self.statsChanged.emit()

    def _ev_name(self, p: dict) -> None:
        if p.get("chatId") and "name" in p:
            self._patch(p["chatId"], {"name": p.get("name") or ""}, resort=False)

    def _ev_chats_changed(self, p: dict) -> None:
        self._reload_timer.start()  # coalesces a burst into one GET /chats
        if p.get("reason") in ("names", "history_sync"):
            self.namesChanged.emit()

    def _ev_deleted(self, p: dict) -> None:
        cid = p.get("chatId") or ""
        self.rows.remove(cid)
        self.statsChanged.emit()
        if cid and cid == self._app.activeChatId:
            self._app.openChat("")

    def _ev_cleared(self, p: dict) -> None:
        cid = p.get("chatId") or ""
        self._patch(cid, {"lastMsg": "", "unread": 0, "hasUnreadMention": False}, resort=False)

    def _ev_typing(self, p: dict) -> None:
        name = (p.get("senderName") or "").strip() or None
        self._set_typing(p.get("chatId") or "", p.get("sender") or "", name,
                         p.get("state") == "composing", p.get("media") == "audio")

    def _ev_presence(self, p: dict) -> None:
        cid = p.get("chatId") or p.get("jid") or ""
        if cid:
            self._presence[cid] = p.get("presence") or ""

    def _ev_starred(self, p: dict) -> None:
        self._starred_version += 1
        self.starredVersionChanged.emit()

    def _ev_group(self, p: dict) -> None:
        jid = p.get("jid") or ""
        if jid and (jid == self._app.activeChatId or jid in self._groups):
            self.loadGroup(jid)

    # ------------------------------------------------------------------ export
    @Slot(str, str)
    def exportChat(self, chat_id: str, chat_name: str) -> None:
        """Save the whole conversation as text in ~/Downloads (`export_chat`)."""
        if self._exporting:
            return
        self._exporting = True
        self.exportingChanged.emit()
        self._say("Mengekspor chat…", "success")
        collected: list[dict] = []

        def page(before: int | None, n: int) -> None:
            path = f"/chats/{enc(chat_id)}/messages?limit=200" + (f"&before={before}" if before else "")

            def done(st: int, body: Any) -> None:
                if st != 200 or not isinstance(body, list):
                    return self._export_done(False, "")
                body.sort(key=lambda m: int(m.get("timestamp") or 0))
                collected[:0] = body
                if len(body) < 200 or not body or n >= 500:
                    return self._write_export(chat_name, collected)
                page(int(body[0].get("timestamp") or 0), n + 1)

            self._call("GET", path, None, done)

        page(None, 0)

    def _write_export(self, chat_name: str, msgs: list[dict]) -> None:
        text = chatfmt.export_chat_text(chat_name, msgs)
        home = Path(os.environ.get("HOME") or os.environ.get("USERPROFILE") or "/tmp")
        d = home / "Downloads"
        d = d if d.is_dir() else Path(os.environ.get("TMPDIR") or "/tmp")
        path = d / chatfmt.safe_file_name(chat_name)
        count = len(msgs)

        def work() -> None:
            try:
                path.write_text(text, encoding="utf-8")
                self._export_cb.emit(True, f"{count} pesan diekspor ke {path}")
            except OSError:
                self._export_cb.emit(False, "")

        threading.Thread(target=work, daemon=True, name="chat-export").start()

    def _export_done(self, ok: bool, msg: str) -> None:
        self._exporting = False
        self.exportingChanged.emit()
        self.exportFinished.emit(ok, msg)
        self._say(msg if ok else "Gagal mengekspor chat", "success" if ok else "error")

    # ------------------------------------------------------------------ formatting helpers for QML
    @Slot(int, result=str)
    def fmtDate(self, ts: int) -> str:
        return chatfmt.format_search_date(ts)

    @Slot(int, result=str)
    def fmtTime(self, ts: int) -> str:
        return chatfmt.format_search_time(ts)

    @Slot("QVariant", result=str)
    def senderLabel(self, msg) -> str:
        return chatfmt.sender_label(dict(msg or {}))

    @Slot("QVariant", result=str)
    def snippetOf(self, msg) -> str:
        return chatfmt.snippet_of(dict(msg or {}))

    @Slot(int, result=str)
    def disappearingLabel(self, seconds: int) -> str:
        return chatfmt.disappearing_label(seconds)

    @Slot(str, result=str)
    def decode(self, content: str) -> str:
        return chatfmt.decode_content(content)


def register(services) -> None:
    chats = Chats(services.api, services.hub, services.app, services.toast, services.backend, services.settings)
    services.expose("Chats", chats)
    services.expose("ChatList", chats.view)
    # `Auth` is loaded before `chats` (registry order): let logout go through it when it has one.
    auth = services.exposed.get("Auth")
    if auth is not None:
        chats.attach_auth(auth)
