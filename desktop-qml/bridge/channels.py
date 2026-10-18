"""`Channels` context property: followed channels, feed with paging, reactions, follow/unfollow/mute.

Mirrors GPUI `views/channels.rs` (and web ChannelsPage):
  GET    /channels                                   -> [Channel]
  GET    /channels/preview?url=                      -> ChannelPreview   (no follow)
  POST   /channels {url}                             follow
  DELETE /channels/{jid}                             unfollow
  POST   /channels/{jid}/mute {muted}
  GET    /channels/{jid}/messages?count=30[&before=serverId]  -> [ChannelMessage], newest first/any order
  POST   /channels/{jid}/messages/{serverId}/react {messageId, emoji}
  WS     channels_changed {reason}, channel_message {channelId, message}, channel_update {channelId, messages}
         (GPUI's ws.rs reads `jid`; the Go backend sends `channelId` — both are accepted)
"""
from __future__ import annotations

import json
from typing import Any
from urllib.parse import quote

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from .listsync import avatar_color, error_text, grouped, initial, ok, post_time, sync_model
from .models import DictListModel

PAGE = 30
QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"]

CHANNEL_ROLES = ["jid", "name", "initial", "color", "avatar", "description", "subscribers", "subscribersLabel",
                 "muted", "verified", "inviteCode"]
POST_ROLES = ["id", "serverId", "type", "timestamp", "timeLabel", "viewsCount", "content", "mediaType",
              "reactions", "channelName"]


def _reactions(raw: Any) -> list[dict]:
    if not isinstance(raw, dict):
        return []
    items = [{"emoji": str(k), "count": int(v or 0)} for k, v in raw.items() if int(v or 0) > 0]
    return sorted(items, key=lambda r: (-r["count"], r["emoji"]))


class Channels(QObject):
    loadingChanged = Signal()
    postsLoadingChanged = Signal()
    loadingMoreChanged = Signal()
    hasMoreChanged = Signal()
    selectedChanged = Signal()
    discoveringChanged = Signal()
    previewChanged = Signal()
    errorChanged = Signal()
    postsErrorChanged = Signal()
    # the feed gained a post at the bottom (QML scrolls down when already at the end)
    postAppended = Signal()
    followed = Signal()

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._api = services.api
        self._model = DictListModel(CHANNEL_ROLES, key="jid", parent=self)
        self._posts = DictListModel(POST_ROLES, key="id", parent=self)
        self._selected = ""
        self._loading = True
        self._posts_loading = False
        self._loading_more = False
        self._has_more = False
        self._discovering = False
        self._preview: dict = {}
        self._error = ""
        self._posts_error = ""
        self._seq = 0
        self._posts_seq = 0
        self._loaded_once = False

        self._debounce = QTimer(self)
        self._debounce.setSingleShot(True)
        self._debounce.setInterval(250)
        self._debounce.timeout.connect(self.refresh)
        self._posts_debounce = QTimer(self)
        self._posts_debounce.setSingleShot(True)
        self._posts_debounce.setInterval(400)
        self._posts_debounce.timeout.connect(self._reload_posts)

        services.hub.event.connect(self._on_event)
        services.app.authChanged.connect(self._on_auth)
        services.backend.apiUrlChanged.connect(self._on_backend)
        if services.backend.apiUrl:
            QTimer.singleShot(0, self.refresh)

    # ---------------------------------------------------------------- properties
    @Property(QObject, constant=True)
    def model(self) -> QObject:
        return self._model

    @Property(QObject, constant=True)
    def posts(self) -> QObject:
        return self._posts

    @Property("QVariantList", constant=True)
    def quickReactions(self) -> list:
        return QUICK_REACTIONS

    @Property(bool, notify=loadingChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=errorChanged)
    def error(self) -> str:
        return self._error

    @Property(bool, notify=postsLoadingChanged)
    def postsLoading(self) -> bool:
        return self._posts_loading

    @Property(str, notify=postsErrorChanged)
    def postsError(self) -> str:
        return self._posts_error

    @Property(bool, notify=loadingMoreChanged)
    def loadingMore(self) -> bool:
        return self._loading_more

    @Property(bool, notify=hasMoreChanged)
    def hasMore(self) -> bool:
        return self._has_more

    @Property(str, notify=selectedChanged)
    def selectedJid(self) -> str:
        return self._selected

    @Property("QVariantMap", notify=selectedChanged)
    def selected(self) -> dict:
        return self._model.get_row(self._selected) or {}

    @Property(bool, notify=discoveringChanged)
    def discovering(self) -> bool:
        return self._discovering

    @Property("QVariantMap", notify=previewChanged)
    def preview(self) -> dict:
        return self._preview

    # ---------------------------------------------------------------- channel list
    def _on_auth(self) -> None:
        if self._svc.app.auth == "authenticated":
            self.refresh()

    def _on_backend(self) -> None:
        if self._svc.backend.apiUrl:
            self.refresh()

    def _row(self, c: dict) -> dict:
        name = c.get("name") or ""
        return {
            "jid": str(c.get("jid", "")), "name": name, "initial": initial(name),
            "color": avatar_color(str(c.get("jid", ""))), "avatar": c.get("avatar") or "",
            "description": c.get("description") or "", "subscribers": int(c.get("subscribers") or 0),
            "subscribersLabel": grouped(c.get("subscribers") or 0), "muted": bool(c.get("muted")),
            "verified": bool(c.get("verified")), "inviteCode": c.get("inviteCode") or "",
        }

    @Slot()
    def refresh(self) -> None:
        self._seq += 1
        seq = self._seq
        self._api.fetch("GET", "/channels", None, lambda st, body, s=seq: self._loaded(s, st, body))

    def _loaded(self, seq: int, status: int, body: Any) -> None:
        if seq != self._seq:
            return
        self._set(loading=False)
        if not ok(status) or not isinstance(body, list):
            if not self._loaded_once:
                self._set(error=error_text(status, body, "Could not load channels"))
            return
        self._loaded_once = True
        self._set(error="")
        rows = [self._row(c) for c in body if isinstance(c, dict)]
        had = self._model.get_row(self._selected) is not None
        sync_model(self._model, rows, "jid")
        if self._selected:
            if had and self._model.get_row(self._selected) is None:
                self._clear_selection()  # unfollowed elsewhere
            else:
                self.selectedChanged.emit()

    _SIGNALS = {"loading": "loadingChanged", "error": "errorChanged", "posts_loading": "postsLoadingChanged",
                "posts_error": "postsErrorChanged", "loading_more": "loadingMoreChanged",
                "has_more": "hasMoreChanged", "discovering": "discoveringChanged"}

    def _set(self, **kw) -> None:
        for k, v in kw.items():
            if getattr(self, "_" + k) != v:
                setattr(self, "_" + k, v)
                getattr(self, self._SIGNALS[k]).emit()

    # ---------------------------------------------------------------- feed
    def _post(self, m: dict) -> dict:
        ch = self._model.get_row(self._selected) or {}
        return {
            "id": str(m.get("id", "")), "serverId": int(m.get("serverId") or 0), "type": m.get("type") or "text",
            "timestamp": int(m.get("timestamp") or 0), "timeLabel": post_time(m.get("timestamp") or 0),
            "viewsCount": int(m.get("viewsCount") or 0), "content": m.get("content") or "",
            "mediaType": m.get("mediaType") or "", "reactions": _reactions(m.get("reactions")),
            "channelName": ch.get("name", ""),
        }

    def _clear_selection(self) -> None:
        self._selected = ""
        self._posts_seq += 1
        self._posts.clear()
        self._set(posts_loading=False, loading_more=False, has_more=False, posts_error="")
        self.selectedChanged.emit()

    @Slot(str)
    def select(self, jid: str) -> None:
        if not jid:
            self.clearSelection()
            return
        self._selected = jid
        self._posts.clear()
        self._set(has_more=False, posts_error="", loading_more=False, posts_loading=True)
        self.selectedChanged.emit()
        self._load_posts()

    @Slot()
    def clearSelection(self) -> None:
        if self._selected:
            self._clear_selection()

    def _feed_path(self, before: int | None = None) -> str:
        p = f"/channels/{quote(self._selected, safe='')}/messages?count={PAGE}"
        return p + (f"&before={before}" if before else "")

    def _load_posts(self) -> None:
        self._posts_seq += 1
        seq, jid = self._posts_seq, self._selected

        def done(status: int, body: Any) -> None:
            if seq != self._posts_seq or jid != self._selected:
                return
            self._set(posts_loading=False)
            if not ok(status) or not isinstance(body, list):
                self._set(posts_error=error_text(status, body, "Could not load posts"))
                return
            rows = sorted((self._post(m) for m in body if isinstance(m, dict)), key=lambda r: r["serverId"])
            self._set(has_more=len(body) == PAGE, posts_error="")
            sync_model(self._posts, rows, "id")
            self.postAppended.emit()

        self._api.fetch("GET", self._feed_path(), None, done)

    def _reload_posts(self) -> None:
        if self._selected and not self._posts_loading:
            self._load_posts()

    @Slot()
    def loadMore(self) -> None:
        if self._loading_more or self._posts_loading or not self._has_more or not self._selected or not self._posts.rows():
            return
        oldest = self._posts.rows()[0]["serverId"]
        jid, seq = self._selected, self._posts_seq
        self._set(loading_more=True)

        def done(status: int, body: Any) -> None:
            if jid != self._selected or seq != self._posts_seq:
                return
            self._set(loading_more=False)
            if not ok(status) or not isinstance(body, list):
                self._svc.toast.error(f"Failed to load older posts: {error_text(status, body)}")
                return
            self._set(has_more=len(body) == PAGE)
            fresh = sorted((self._post(m) for m in body if isinstance(m, dict)
                            if not self._posts.contains(str(m.get("id", "")))), key=lambda r: r["serverId"])
            if fresh:
                self._posts.extend(fresh, at=0)

        self._api.fetch("GET", self._feed_path(oldest), None, done)

    # ---------------------------------------------------------------- actions
    @Slot(str, str)
    def react(self, message_id: str, emoji: str) -> None:
        post = self._posts.get_row(message_id)
        if not post or not self._selected:
            return
        jid, server_id = self._selected, post["serverId"]
        self._bump(message_id, emoji, +1)

        def done(status: int, body: Any) -> None:
            if not ok(status):
                self._bump(message_id, emoji, -1)
                self._svc.toast.error(f"Failed to react: {error_text(status, body)}")

        self._api.fetch("POST", f"/channels/{quote(jid, safe='')}/messages/{server_id}/react",
                        {"messageId": message_id, "emoji": emoji}, done)

    def _bump(self, message_id: str, emoji: str, delta: int) -> None:
        post = self._posts.get_row(message_id)
        if not post:
            return
        counts = {r["emoji"]: r["count"] for r in post["reactions"]}
        counts[emoji] = max(counts.get(emoji, 0) + delta, 0)
        self._posts.patch(message_id, {"reactions": _reactions(counts)})

    @Slot()
    def toggleMute(self) -> None:
        ch = self._model.get_row(self._selected)
        if not ch:
            return
        jid, muted = ch["jid"], not ch["muted"]
        self._model.patch(jid, {"muted": muted})  # optimistic, like GPUI
        self.selectedChanged.emit()

        def done(status: int, body: Any) -> None:
            if not ok(status):
                self._model.patch(jid, {"muted": not muted})
                self.selectedChanged.emit()
                self._svc.toast.error(f"Failed to update mute: {error_text(status, body)}")

        self._api.fetch("POST", f"/channels/{quote(jid, safe='')}/mute", {"muted": muted}, done)

    @Slot()
    def unfollow(self) -> None:
        ch = self._model.get_row(self._selected)
        if not ch:
            return
        jid = ch["jid"]
        self._model.remove(jid)  # optimistic like GPUI; restored by refresh() on failure
        self._clear_selection()
        self._svc.toast.success("Unfollowed channel")

        def done(status: int, body: Any) -> None:
            if not ok(status):
                self._svc.toast.error(f"Failed to unfollow: {error_text(status, body)}")
            self.refresh()

        self._api.fetch("DELETE", f"/channels/{quote(jid, safe='')}", None, done)

    # -- discover dialog
    @Slot()
    def clearPreview(self) -> None:
        if self._preview:
            self._preview = {}
            self.previewChanged.emit()

    @Slot(str)
    def previewChannel(self, link: str) -> None:
        link = (link or "").strip()
        if not link or self._discovering:
            return
        self.clearPreview()
        self._set(discovering=True)

        def done(status: int, body: Any) -> None:
            self._set(discovering=False)
            if ok(status) and isinstance(body, dict):
                self._preview = {
                    "jid": body.get("jid", ""), "name": body.get("name", ""),
                    "subscribers": int(body.get("subscribers") or 0),
                    "subscribersLabel": grouped(body.get("subscribers") or 0),
                    "description": body.get("description") or "",
                }
                self.previewChanged.emit()
            else:
                self._svc.toast.error("Invalid channel link")

        self._api.fetch("GET", f"/channels/preview?url={quote(link, safe='')}", None, done)

    @Slot(str)
    def follow(self, link: str) -> None:
        link = (link or "").strip()
        if not link or self._discovering:
            return
        self._set(discovering=True)

        def done(status: int, body: Any) -> None:
            self._set(discovering=False)
            if ok(status):
                self.clearPreview()
                self._svc.toast.success("Channel followed")
                self.followed.emit()
                self.refresh()
            else:
                self._svc.toast.error(f"Failed to follow: {error_text(status, body)}")

        self._api.fetch("POST", "/channels", {"url": link}, done)

    # ---------------------------------------------------------------- WS
    def _on_event(self, typ: str, payload: str) -> None:
        if typ not in ("channels_changed", "channel_message", "channel_update"):
            return
        try:
            p = json.loads(payload) or {}
        except ValueError:
            return
        if typ == "channels_changed":
            self._debounce.start()
            return
        jid = p.get("channelId") or p.get("jid") or ""
        if not jid or jid != self._selected:
            return
        if typ == "channel_message":
            msg = p.get("message")
            if isinstance(msg, dict) and msg.get("id"):
                row = self._post(msg)
                if self._posts.contains(row["id"]):
                    self._posts.patch(row["id"], row)
                else:
                    at = next((i for i, r in enumerate(self._posts.rows()) if r["serverId"] > row["serverId"]),
                              len(self._posts.rows()))
                    self._posts.insert(at, row)
                    if at == len(self._posts.rows()) - 1:
                        self.postAppended.emit()
            else:
                self._posts_debounce.start()
        else:  # channel_update: authoritative reaction/view counts
            msgs = p.get("messages")
            if isinstance(msgs, list):
                for m in msgs:
                    if isinstance(m, dict) and self._posts.contains(str(m.get("id", ""))):
                        self._posts.patch(str(m["id"]), {
                            "viewsCount": int(m.get("viewsCount") or 0), "reactions": _reactions(m.get("reactions"))})
            else:
                self._posts_debounce.start()


def register(services) -> None:
    services.channels = Channels(services)
    services.expose("Channels", services.channels)
