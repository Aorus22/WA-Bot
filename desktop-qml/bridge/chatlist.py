"""Row shape + filtered view of the chat list (GPUI `ChatView::render` filter/sort block)."""
from __future__ import annotations

from typing import Any

from PySide6.QtCore import Property, QModelIndex, QObject, QSortFilterProxyModel, Signal, Slot

from . import chatfmt

# REST `/chats` fields (backend-client dto::Chat) + fields derived for the delegates.
REST_ROLES = ["id", "name", "avatar", "lastMsg", "lastTime", "unread", "hasUnreadMention", "isActive", "isGroup",
              "archived", "pinnedAt", "muteMode", "mutedUntil", "disappearingSecs"]
DERIVED_ROLES = ["typing", "snippet", "timeLabel", "muted", "pinned"]
ROLES = REST_ROLES + DERIVED_ROLES


def normalize(d: dict[str, Any]) -> dict[str, Any]:
    """A chat row with every REST field present (serde defaults of `dto::Chat`)."""
    return {
        "id": d.get("id") or "",
        "name": d.get("name") or "",
        "avatar": d.get("avatar") or "",
        "lastMsg": d.get("lastMsg") or "",
        "lastTime": int(d.get("lastTime") or 0),
        "unread": int(d.get("unread") or 0),
        "hasUnreadMention": bool(d.get("hasUnreadMention")),
        "isActive": bool(d.get("isActive")),
        "isGroup": bool(d.get("isGroup")),
        "archived": bool(d.get("archived")),
        "pinnedAt": d.get("pinnedAt"),
        "muteMode": d.get("muteMode") or "off",
        "mutedUntil": d.get("mutedUntil"),
        "disappearingSecs": int(d.get("disappearingSecs") or 0),
    }


def derive(row: dict[str, Any]) -> dict[str, Any]:
    return {
        "snippet": chatfmt.list_snippet(row.get("lastMsg")),
        "timeLabel": chatfmt.format_chat_time(row.get("lastTime")),
        "muted": row.get("muteMode") not in ("", "off", None),
        "pinned": row.get("pinnedAt") is not None,
    }


def sort_key(row: dict[str, Any]):
    """Pinned first (most recently pinned first), then newest last message first."""
    if row.get("pinnedAt") is not None:
        return (0, -chatfmt.to_secs(row["pinnedAt"]), 0)
    return (1, -chatfmt.to_secs(row.get("lastTime")), 0)


class ChatFilterModel(QSortFilterProxyModel):
    """What the sidebar shows: archived vs. active, search text, and the All/Unread/Groups/Contacts filter
    (the chips exist in GPUI state; its UI does not render them). Keeps the source order (already sorted)."""
    changed = Signal()
    statsChanged = Signal()

    def __init__(self, source, parent: QObject | None = None):
        super().__init__(parent)
        self._archived = False
        self._query = ""
        self._kind = "all"
        self._pinned = 0
        self.setSourceModel(source)
        for sig in (self.rowsInserted, self.rowsRemoved, self.modelReset, self.layoutChanged, self.dataChanged):
            sig.connect(self._recount)
        self._recount()

    def filterAcceptsRow(self, row: int, parent: QModelIndex) -> bool:  # noqa: N802
        c = self.sourceModel().rows()[row]
        if bool(c.get("archived")) != self._archived:
            return False
        kind = self._kind
        if kind == "unread" and not c.get("unread"):
            return False
        if kind == "groups" and not c.get("isGroup"):
            return False
        if kind == "contacts" and c.get("isGroup"):
            return False
        q = self._query.strip().lower()
        if q:
            return (q in (c.get("name") or "").lower() or q in (c.get("id") or "").lower()
                    or q in (c.get("lastMsg") or "").lower())
        return True

    def _recount(self, *_a) -> None:
        src = self.sourceModel().rows()
        n = 0
        for i in range(self.rowCount()):
            r = src[self.mapToSource(self.index(i, 0)).row()]
            if r.get("pinned"):
                n += 1
        if n != self._pinned:
            self._pinned = n
        self.statsChanged.emit()

    def _set(self, name: str, value) -> None:
        if getattr(self, name) == value:
            return
        setattr(self, name, value)
        self.invalidateFilter()
        self.changed.emit()
        self._recount()

    @Property(bool, notify=changed)
    def archivedMode(self) -> bool:
        return self._archived

    @archivedMode.setter
    def archivedMode(self, v: bool) -> None:
        self._set("_archived", bool(v))

    @Property(str, notify=changed)
    def query(self) -> str:
        return self._query

    @query.setter
    def query(self, v: str) -> None:
        self._set("_query", v or "")

    @Property(str, notify=changed)
    def kind(self) -> str:
        return self._kind

    @kind.setter
    def kind(self, v: str) -> None:
        self._set("_kind", v or "all")

    @Property(int, notify=statsChanged)
    def count(self) -> int:
        return self.rowCount()

    @Property(int, notify=statsChanged)
    def pinnedCount(self) -> int:
        return self._pinned

    @Slot(int, result="QVariant")
    def get(self, i: int):
        if 0 <= i < self.rowCount():
            return self.sourceModel().rows()[self.mapToSource(self.index(i, 0)).row()]
        return None
