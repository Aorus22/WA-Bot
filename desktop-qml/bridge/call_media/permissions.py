"""Microphone / camera permission handling.

On Linux (and any platform without a permission prompt) Qt reports `Granted`. On macOS/Windows/
Flatpak-like sandboxes the first use triggers the OS prompt; a denial is surfaced to the user as
a notice with an actionable message instead of a silent dead call."""
from __future__ import annotations

from typing import Callable

from PySide6.QtCore import QCameraPermission, QCoreApplication, QMicrophonePermission, QObject, Qt

DENIED = {
    "microphone": "Microphone access is blocked. Allow it for this app in your system's privacy settings.",
    "camera": "Camera access is blocked. Allow it for this app in your system's privacy settings.",
}


def _permission(kind: str):
    return QMicrophonePermission() if kind == "microphone" else QCameraPermission()


def status(kind: str) -> str:
    """granted | denied | undetermined (granted when the platform has no permission system)."""
    app = QCoreApplication.instance()
    if app is None:
        return "granted"
    try:
        s = app.checkPermission(_permission(kind))
    except Exception:  # noqa: BLE001 - unknown platform/permission plugin: do not block the call
        return "granted"
    if s == Qt.PermissionStatus.Granted:
        return "granted"
    return "denied" if s == Qt.PermissionStatus.Denied else "undetermined"


def ensure(kind: str, context: QObject, callback: Callable[[bool], None]) -> None:
    """Call `callback(granted)` once the permission is known (prompting if needed)."""
    st = status(kind)
    if st != "undetermined":
        callback(st == "granted")
        return
    app = QCoreApplication.instance()
    try:
        app.requestPermission(_permission(kind), context,
                              lambda p: callback(app.checkPermission(p) == Qt.PermissionStatus.Granted))
    except Exception:  # noqa: BLE001
        callback(True)
