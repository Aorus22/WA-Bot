"""`Auth` — account/session state for the login screen and the startup gate.

Mirrors GPUI `state/auth.rs` + the seeding in `main.rs`:
  * start-up: GET /status -> authenticated (phone) | unauthenticated (+ GET /qr-code for the QR string);
  * WS `qr_code {code}` (ignored while authenticated), `auth_success {phone}`, `logged_out`;
  * "Refresh QR" / "Retry": GET /qr-code (empty code = error "Failed to fetch QR code from backend.");
  * logout: POST /logout, then unauthenticated + navigate to chat (a new `qr_code` event follows).
Additions GPUI lacks (a user needs them): a failed /status keeps the app in `checking` and retries
(GPUI would drop to the login) so the gate can offer a Retry; a WS (re)connect or a moved backend re-checks /status.

The QR is rendered by `image://qr/<urlsafe-b64(text)>` (segno builds the module matrix, painted like GPUI's
`render_qr_matrix`: 256px white square, 16px quiet zone, 12px rounded corners). The backend has no
phone-code pairing endpoint (GPUI/web both show "coming soon" on the Phone tab), so there is none here.

QML: status (checking|unauthenticated|authenticated), phone, qr, qrLoading, qrError, qrSource, qrInvalid,
activeTab (qr|phone), checkFailures, checkError; slots setTab(t) refreshQr() retryCheck() logout().
"""
from __future__ import annotations

import base64
import json
from typing import Any

from PySide6.QtCore import Property, QObject, QRectF, QSize, Qt, QTimer, Signal, Slot
from PySide6.QtGui import QColor, QImage, QPainter, QPainterPath
from PySide6.QtQuick import QQuickImageProvider

try:  # pure-python QR encoder (pyproject dependency); without it the QR shows "Invalid QR code"
    import segno
except ImportError:  # pragma: no cover
    segno = None

QR_FETCH_ERROR = "Failed to fetch QR code from backend."
RETRY_FIRST_MS = 2000
RETRY_MS = 3000
UNAVAILABLE_AFTER = 3  # failed /status checks before the gate offers a Retry screen
TABS = ("qr", "phone")


# -- QR image ------------------------------------------------------------------
def qr_matrix(text: str) -> list[list[bool]] | None:
    """Module matrix (True = dark) at error-correction M like `qrcode::QrCode::new` / web `level="M"`."""
    if segno is None or not text:
        return None
    try:
        q = segno.make(text, error="m", micro=False, boost_error=False)
    except Exception:
        return None
    return [[bool(c) for c in row] for row in q.matrix]


def render_qr(matrix: list[list[bool]], side: int = 256) -> QImage:
    """GPUI geometry scaled to `side`: white square, quiet zone side/16, modules (cell+0.3px), 12px-radius corners."""
    img = QImage(side, side, QImage.Format.Format_ARGB32_Premultiplied)
    img.fill(Qt.GlobalColor.transparent)
    k = side / 256.0
    n = len(matrix)
    quiet = 16.0 * k
    cell = (side - 2 * quiet) / n
    p = QPainter(img)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    clip = QPainterPath()
    clip.addRoundedRect(QRectF(0, 0, side, side), 12 * k, 12 * k)
    p.setClipPath(clip)
    p.fillRect(QRectF(0, 0, side, side), QColor("white"))
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(QColor("black"))
    for y, row in enumerate(matrix):
        for x, dark in enumerate(row):
            if dark:
                p.drawRect(QRectF(quiet + x * cell, quiet + y * cell, cell + 0.3 * k, cell + 0.3 * k))
    p.end()
    return img


def qr_url(text: str) -> str:
    return "image://qr/" + base64.urlsafe_b64encode(text.encode()).decode().rstrip("=") if text else ""


class QrProvider(QQuickImageProvider):
    def __init__(self):
        super().__init__(QQuickImageProvider.ImageType.Image)

    def requestImage(self, id: str, size: QSize, requestedSize: QSize):  # noqa: N802
        raw = id.split("?", 1)[0]
        try:
            text = base64.urlsafe_b64decode(raw + "=" * (-len(raw) % 4)).decode()
        except Exception:
            text = ""
        side = max(requestedSize.width(), requestedSize.height(), 0) or 256
        m = qr_matrix(text)
        img = render_qr(m, side) if m else QImage(side, side, QImage.Format.Format_ARGB32_Premultiplied)
        if not m:
            img.fill(Qt.GlobalColor.transparent)
        size.setWidth(side)
        size.setHeight(side)
        return img


# -- state ---------------------------------------------------------------------
class Auth(QObject):
    statusChanged = Signal()
    phoneChanged = Signal()
    qrChanged = Signal()
    qrLoadingChanged = Signal()
    qrErrorChanged = Signal()
    tabChanged = Signal()
    checkChanged = Signal()
    loggingOutChanged = Signal()
    loggedOut = Signal(bool)  # logout finished (ok)

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._s = services
        self._status = "checking"
        self._phone = ""
        self._qr = ""
        self._qr_ok = False  # the current QR string encodes
        self._qr_loading = False
        self._qr_error = ""
        self._tab = "qr"
        self._failures = 0
        self._check_error = ""
        self._in_flight = False
        self._logging_out = False
        self._gen = 0  # invalidates QR fetches that were overtaken by a WS event
        self._retry = QTimer(self)
        self._retry.setSingleShot(True)
        self._retry.timeout.connect(self._check)
        services.hub.event.connect(self.handle_event)
        services.hub.connectedChanged.connect(self._on_ws)
        services.backend.stateChanged.connect(self._on_backend)
        services.backend.apiUrlChanged.connect(self._on_backend)
        self._sync_app()
        self._on_backend()

    # -- properties --------------------------------------------------------------
    @Property(str, notify=statusChanged)
    def status(self) -> str:
        return self._status

    @Property(str, notify=phoneChanged)
    def phone(self) -> str:
        return self._phone

    @Property(str, notify=qrChanged)
    def qr(self) -> str:
        return self._qr

    @Property(str, notify=qrChanged)
    def qrSource(self) -> str:
        return qr_url(self._qr) if self._qr_ok else ""

    @Property(bool, notify=qrChanged)
    def qrInvalid(self) -> bool:
        return bool(self._qr) and not self._qr_ok

    @Property(bool, notify=qrLoadingChanged)
    def qrLoading(self) -> bool:
        return self._qr_loading

    @Property(str, notify=qrErrorChanged)
    def qrError(self) -> str:
        return self._qr_error

    @Property(str, notify=tabChanged)
    def activeTab(self) -> str:
        return self._tab

    @Property(int, notify=checkChanged)
    def checkFailures(self) -> int:
        return self._failures

    @Property(str, notify=checkChanged)
    def checkError(self) -> str:
        return self._check_error

    @Property(bool, notify=loggingOutChanged)
    def loggingOut(self) -> bool:
        return self._logging_out

    # -- state transitions (GPUI AuthState) ---------------------------------------
    def _sync_app(self) -> None:
        self._s.app.set_auth(self._status, self._phone)

    def _set_status(self, status: str) -> None:
        if status == "authenticated":  # GPUI set_status: the QR is no longer needed
            self._set_qr(None)
            self._set_qr_loading(False)
        if status != self._status:
            self._status = status
            self.statusChanged.emit()
        self._sync_app()

    def _set_phone(self, phone: str) -> None:
        if phone != self._phone:
            self._phone = phone
            self.phoneChanged.emit()
        self._sync_app()

    def _set_qr(self, code: str | None) -> None:
        """GPUI set_qr_code: also clears loading and the error."""
        code = code or ""
        changed = code != self._qr
        self._qr = code
        self._qr_ok = qr_matrix(code) is not None
        self._set_qr_loading(False)
        self._set_qr_error("")
        if changed:
            self.qrChanged.emit()

    def _set_qr_loading(self, v: bool) -> None:
        if v != self._qr_loading:
            self._qr_loading = v
            self.qrLoadingChanged.emit()

    def _set_qr_error(self, msg: str) -> None:
        if msg != self._qr_error:
            self._qr_error = msg
            self.qrErrorChanged.emit()
        if msg:
            self._set_qr_loading(False)

    @Slot(str)
    def setTab(self, tab: str) -> None:
        if tab in TABS and tab != self._tab:
            self._tab = tab
            self.tabChanged.emit()

    # -- WS events -----------------------------------------------------------------
    @Slot(str, str)
    def handle_event(self, type_: str, payload_json: str) -> None:
        try:
            payload: Any = json.loads(payload_json) if payload_json else {}
        except ValueError:
            payload = {}
        if not isinstance(payload, dict):
            payload = {}
        if type_ == "qr_code":
            if self._status != "authenticated":
                self._gen += 1
                self._set_status("unauthenticated")
                self._set_qr(str(payload.get("code") or ""))
                self._clear_check()
        elif type_ == "auth_success":
            self._gen += 1
            self._set_status("authenticated")
            self._set_phone(str(payload.get("phone") or ""))
            self._clear_check()
        elif type_ == "logged_out":
            self._gen += 1
            self._set_status("unauthenticated")
            self._set_phone("")
            self._set_qr(None)
            self._clear_check()

    # -- /status check -----------------------------------------------------------------
    def _clear_check(self) -> None:
        self._retry.stop()
        if self._failures or self._check_error:
            self._failures, self._check_error = 0, ""
            self.checkChanged.emit()

    def _on_backend(self) -> None:
        if self._s.backend.state == "ready" and self._s.backend.apiUrl:
            if self._status == "checking":
                if not self._in_flight and not self._retry.isActive():
                    self._check()
            else:
                self._check()  # backend moved/restarted: refresh silently

    def _on_ws(self) -> None:
        if self._s.hub.connected and not self._in_flight and self._s.backend.state == "ready":
            self._check()  # reconnect: events may have been missed (login/logout while offline)

    @Slot()
    def retryCheck(self) -> None:
        """Retry button on the "backend unavailable" screen."""
        if self._s.backend.state == "error":
            self._s.backend.start()
            return
        self._retry.stop()
        if not self._in_flight:
            self._check()

    def _check(self) -> None:
        if self._in_flight:
            return
        self._in_flight = True
        self._s.api.fetch("GET", "/status", None, self._on_status)

    def _on_status(self, code: int, body: Any) -> None:
        self._in_flight = False
        if not (200 <= code < 300) or not isinstance(body, dict):
            msg = ""
            if isinstance(body, dict):
                msg = str(body.get("error") or "")
            self._failures += 1
            self._check_error = msg or (f"Backend answered HTTP {code}." if code else "No response from the backend.")
            self.checkChanged.emit()
            if self._status == "checking":
                self._retry.start(RETRY_FIRST_MS if self._failures == 1 else RETRY_MS)
            return
        self._clear_check()
        if body.get("isLoggedIn"):
            self._set_status("authenticated")
            self._set_phone(str(body.get("phone") or ""))
            return
        self._set_status("unauthenticated")
        self._set_phone("")
        self._fetch_qr(initial=True)

    # -- QR ------------------------------------------------------------------------------
    @Slot()
    def refreshQr(self) -> None:
        """"Refresh QR" / "Retry": GET /qr-code (GPUI `trigger_qr_refresh`)."""
        self._fetch_qr(initial=False)

    def _fetch_qr(self, initial: bool) -> None:
        if not initial:
            self._set_qr_loading(True)
        gen = self._gen

        def done(code: int, body: Any) -> None:
            if gen != self._gen or self._status == "authenticated":
                return  # a WS event already said something newer
            qr = str(body.get("code") or "") if isinstance(body, dict) and 200 <= code < 300 else ""
            if qr:
                self._set_qr(qr)
            elif not initial:
                self._set_qr_error(QR_FETCH_ERROR)
        self._s.api.fetch("GET", "/qr-code", None, done)

    # -- logout --------------------------------------------------------------------------
    @Slot()
    def logout(self) -> None:
        """POST /logout, then (like GPUI) become unauthenticated and go to the chat route; the backend
        broadcasts `logged_out` and then a fresh `qr_code`. The confirmation dialog belongs to the caller."""
        if self._logging_out:
            return
        self._logging_out = True
        self.loggingOutChanged.emit()

        def done(code: int, body: Any) -> None:
            self._logging_out = False
            self.loggingOutChanged.emit()
            ok = 200 <= code < 300
            if ok:
                self._gen += 1
                self._set_status("unauthenticated")
                self._set_phone("")
                self._set_qr(None)
                self._s.app.navigate("chat")
            else:
                err = str(body.get("error")) if isinstance(body, dict) and body.get("error") else f"HTTP {code}"
                if self._s.toast is not None:
                    self._s.toast.error(f"Logout failed: {err}")
            self.loggedOut.emit(ok)
        self._s.api.fetch("POST", "/logout", None, done)


def register(services) -> None:
    services.engine.addImageProvider("qr", QrProvider())
    services.expose("Auth", Auth(services, services.app))
