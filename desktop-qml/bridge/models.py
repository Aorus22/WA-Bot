"""Generic dict-backed list model so QML delegates use plain role names.

    m = DictListModel(["id", "name", "unread"], key="id")
    m.reset(rows); m.upsert(row); m.patch(key, {"unread": 0}); m.remove(key)
Rows are dicts; missing roles read as None. `get(i)`/`find(key)` expose rows to QML.
Mutations only emit the narrowest signal that is true, so ListView keeps its position.
"""
from __future__ import annotations

from typing import Any, Callable, Iterable

from PySide6.QtCore import (
    Property, QAbstractListModel, QByteArray, QModelIndex, Qt, Signal, Slot,
)


class DictListModel(QAbstractListModel):
    countChanged = Signal()

    def __init__(self, roles: Iterable[str], key: str = "id", parent=None):
        super().__init__(parent)
        self._names = list(roles)
        self._key = key
        self._rows: list[dict[str, Any]] = []
        self._index: dict[Any, int] = {}
        self._role_ids = {Qt.ItemDataRole.UserRole + 1 + i: n for i, n in enumerate(self._names)}

    # --- QAbstractListModel ---------------------------------------------
    def roleNames(self) -> dict[int, QByteArray]:
        return {rid: QByteArray(n.encode()) for rid, n in self._role_ids.items()}

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:  # noqa: B008
        return 0 if parent.isValid() else len(self._rows)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not 0 <= index.row() < len(self._rows):
            return None
        name = self._role_ids.get(role)
        return self._rows[index.row()].get(name) if name else None

    @Property(int, notify=countChanged)
    def count(self) -> int:
        return len(self._rows)

    # --- python API -------------------------------------------------------
    def rows(self) -> list[dict[str, Any]]:
        return self._rows

    def _reindex(self, start: int = 0) -> None:
        for i in range(start, len(self._rows)):
            self._index[self._rows[i].get(self._key)] = i

    def reset(self, rows: Iterable[dict[str, Any]]) -> None:
        self.beginResetModel()
        self._rows = [dict(r) for r in rows]
        self._index = {}
        self._reindex()
        self.endResetModel()
        self.countChanged.emit()

    def clear(self) -> None:
        if self._rows:
            self.reset([])

    def contains(self, key: Any) -> bool:
        return key in self._index

    def row_of(self, key: Any) -> int:
        return self._index.get(key, -1)

    def get_row(self, key: Any) -> dict[str, Any] | None:
        i = self._index.get(key)
        return self._rows[i] if i is not None else None

    def append(self, row: dict[str, Any]) -> None:
        self.insert(len(self._rows), row)

    def prepend(self, row: dict[str, Any]) -> None:
        self.insert(0, row)

    def insert(self, at: int, row: dict[str, Any]) -> None:
        at = max(0, min(at, len(self._rows)))
        self.beginInsertRows(QModelIndex(), at, at)
        self._rows.insert(at, dict(row))
        self.endInsertRows()
        self._reindex(at)
        self.countChanged.emit()

    def extend(self, rows: list[dict[str, Any]], at: int | None = None) -> None:
        if not rows:
            return
        at = len(self._rows) if at is None else max(0, min(at, len(self._rows)))
        self.beginInsertRows(QModelIndex(), at, at + len(rows) - 1)
        self._rows[at:at] = [dict(r) for r in rows]
        self.endInsertRows()
        self._reindex(at)
        self.countChanged.emit()

    def upsert(self, row: dict[str, Any], at: int | None = None) -> None:
        i = self._index.get(row.get(self._key))
        if i is None:
            self.insert(len(self._rows) if at is None else at, row)
        else:
            self._rows[i].update(row)
            self._emit(i)

    def patch(self, key: Any, fields: dict[str, Any]) -> bool:
        i = self._index.get(key)
        if i is None:
            return False
        self._rows[i].update(fields)
        self._emit(i, list(fields))
        return True

    def _emit(self, i: int, names: list[str] | None = None) -> None:
        idx = self.index(i)
        roles = [rid for rid, n in self._role_ids.items() if names is None or n in names]
        self.dataChanged.emit(idx, idx, roles)

    def remove(self, key: Any) -> bool:
        i = self._index.get(key)
        if i is None:
            return False
        self.beginRemoveRows(QModelIndex(), i, i)
        del self._rows[i]
        self._index.pop(key, None)
        self.endRemoveRows()
        self._reindex(i)
        self.countChanged.emit()
        return True

    def remove_where(self, pred: Callable[[dict[str, Any]], bool]) -> None:
        self.reset([r for r in self._rows if not pred(r)])

    def sort_by(self, keyfn: Callable[[dict[str, Any]], Any], reverse: bool = False) -> None:
        new = sorted(self._rows, key=keyfn, reverse=reverse)
        if [r.get(self._key) for r in new] == [r.get(self._key) for r in self._rows]:
            return
        self.layoutAboutToBeChanged.emit()
        old_idx = {id(r): i for i, r in enumerate(self._rows)}
        persistent = self.persistentIndexList()
        self._rows = new
        self._index = {}
        self._reindex()
        new_idx = {id(r): i for i, r in enumerate(self._rows)}
        old_by_pos = {v: k for k, v in old_idx.items()}
        for p in persistent:
            rid = old_by_pos.get(p.row())
            if rid is not None:
                self.changePersistentIndex(p, self.index(new_idx[rid]))
        self.layoutChanged.emit()

    # --- QML API ----------------------------------------------------------
    @Slot(int, result="QVariant")
    def get(self, i: int):
        return self._rows[i] if 0 <= i < len(self._rows) else None

    @Slot("QVariant", result=int)
    def indexOfKey(self, key) -> int:
        return self._index.get(key, -1)

    @Slot("QVariant", result="QVariant")
    def find(self, key):
        return self.get_row(key)
