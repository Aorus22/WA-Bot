"""QML context property `Toast`: `Toast.show(msg, kind)`, `.success(msg)`, `.error(msg)`.
Python code can call `services.toast.show(...)` too. Rendered by shell/ToastHost.qml."""
from __future__ import annotations

from PySide6.QtCore import QObject, Signal, Slot


class ToastBus(QObject):
    shown = Signal(int, str, str)  # id, message, kind (info|success|error)
    dismissed = Signal(int)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._next = 1

    @Slot(str, str, result=int)
    def show(self, message: str, kind: str = "info") -> int:
        i, self._next = self._next, self._next + 1
        self.shown.emit(i, message, kind or "info")
        return i

    @Slot(str, result=int)
    def success(self, message: str) -> int:
        return self.show(message, "success")

    @Slot(str, result=int)
    def error(self, message: str) -> int:
        return self.show(message, "error")

    @Slot(int)
    def dismiss(self, i: int) -> None:
        self.dismissed.emit(i)
