"""The media WebSocket of one call — port of `call_media/net.rs` + `backend-client/src/media.rs`.

`MediaNet` lives on the session's worker thread. It requests a single-use token
(`POST /api/calls/{id}/media-session`), opens `ws(s)://host{ws_path}?token=..&video=jpeg`, hands
incoming audio/pictures to the session and reconnects (with a fresh token) whenever the socket
drops. A dropped media socket never ends the call. Connect problems become a user-visible notice
only after `ERROR_AFTER_FAILURES` consecutive failures (the first ones are normal while a call is
still becoming active); retries slow down after `SLOW_AFTER_FAILURES`.
"""
from __future__ import annotations

import json
import sys
from typing import Callable
from urllib.parse import quote

from PySide6.QtCore import QByteArray, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest
from PySide6.QtWebSockets import QWebSocket

from . import frame as wire

ERROR_AFTER_FAILURES = 3
SLOW_AFTER_FAILURES = 6
CONNECT_TIMEOUT_MS = 10_000
# Never let the socket buffer more than ~8 audio frames: media is real time, stale data is dropped.
MAX_PENDING_BYTES = 8 * wire.AUDIO_FRAME_BYTES + 64 * 1024


def media_ws_url(base_url: str, session: dict) -> str:
    """Absolute ws(s) URL for a media session (JPEG video mode). `ws_path` is normally relative to
    the backend root; an absolute ws:// URL is accepted as is (tests, reverse proxies)."""
    path = str(session.get("ws_path") or "")
    if path.startswith(("ws://", "wss://")):
        root, path = "", path
    else:
        root = base_url.rstrip("/")
        root = root[:-4] if root.endswith("/api") else root
        root = ("wss://" + root[8:]) if root.startswith("https://") else ("ws://" + root[7:]) if root.startswith("http://") else root
        if not path.startswith("/"):
            path = "/" + path
    sep = "&" if "?" in path else "?"
    return f"{root}{path}{sep}token={quote(str(session.get('token', '')), safe='-_.~')}&video=jpeg"


def describe_http(status: int, body: str) -> str:
    """A short, user-facing reason for a failed token request."""
    message = ""
    try:
        parsed = json.loads(body)
        if isinstance(parsed, dict) and isinstance(parsed.get("error"), str):
            message = parsed["error"]
    except ValueError:
        message = body.strip()
    if status == 404:
        message = message or "not found"
        message += " - this backend does not offer call media (needs the call-media backend build)"
    return f"{message or 'request failed'} (HTTP {status})" if status else (message or "backend unreachable")


class MediaNet(QObject):
    connectedChanged = Signal(bool)
    notice = Signal(str, str)              # source, message ("" clears)
    audioReceived = Signal(bytes)          # PCM s16le, normally 1920 bytes
    videoReceived = Signal(bytes, int)     # one JPEG, clockwise quarter turns
    keyframeRequested = Signal()

    def __init__(self, call_id: str, base_url: Callable[[], str], retry_delay_ms: int = 1000,
                 parent: QObject | None = None):
        super().__init__(parent)
        self.call_id = call_id
        self._base_url = base_url
        self._retry_ms = retry_delay_ms
        self._failures = 0
        self._running = False
        self._connected = False
        self._nam: QNetworkAccessManager | None = None
        self._sock: QWebSocket | None = None
        self._reply: QNetworkReply | None = None
        self._retry = QTimer(self)
        self._retry.setSingleShot(True)
        self._retry.timeout.connect(self._attempt)
        self._connect_timer = QTimer(self)
        self._connect_timer.setSingleShot(True)
        self._connect_timer.timeout.connect(lambda: self._fail("connection timed out"))
        self.stats = {"sent_audio": 0, "sent_video": 0, "recv_audio": 0, "recv_video": 0, "dropped": 0,
                      "connects": 0, "bad_frames": 0}

    @property
    def connected(self) -> bool:
        return self._connected

    @Slot()
    def start(self) -> None:
        if self._running:
            return
        self._running = True
        self._nam = QNetworkAccessManager(self)
        self._attempt()

    @Slot()
    def stop(self) -> None:
        self._running = False
        self._retry.stop()
        self._connect_timer.stop()
        self._drop_socket(close=True)
        if self._reply is not None:
            self._reply.abort()
            self._reply = None
        self._set_connected(False)

    # -- connecting ---------------------------------------------------------------------
    def _attempt(self) -> None:
        if not self._running:
            return
        base = self._base_url()
        if not base:
            self._fail("backend not ready")
            return
        req = QNetworkRequest(QUrl(f"{base}/calls/{quote(self.call_id, safe='')}/media-session"))
        req.setHeader(QNetworkRequest.KnownHeaders.ContentTypeHeader, "application/json")
        req.setTransferTimeout(10_000)
        reply = self._reply = self._nam.post(req, QByteArray(b"{}"))
        reply.finished.connect(lambda r=reply, b=base: self._on_token(r, b))

    def _on_token(self, reply: QNetworkReply, base: str) -> None:
        if reply is not self._reply:
            reply.deleteLater()
            return
        self._reply = None
        status = reply.attribute(QNetworkRequest.Attribute.HttpStatusCodeAttribute) or 0
        body = bytes(reply.readAll()).decode("utf-8", "replace")
        err = reply.errorString()
        reply.deleteLater()
        if not self._running:
            return
        if not status or status >= 300:
            self._fail(describe_http(int(status), body) if status else (err or "backend unreachable"))
            return
        try:
            session = json.loads(body)
            if not isinstance(session, dict) or not session.get("token"):
                raise ValueError("no token")
        except ValueError:
            self._fail("unexpected media-session response")
            return
        self._open(media_ws_url(base, session))

    def _open(self, url: str) -> None:
        self._drop_socket(close=False)
        sock = self._sock = QWebSocket()
        sock.setParent(self)
        sock.connected.connect(lambda s=sock: self._on_open(s))
        sock.disconnected.connect(lambda s=sock: self._on_closed(s))
        sock.errorOccurred.connect(lambda _e, s=sock: self._on_error(s))
        sock.binaryMessageReceived.connect(self._on_binary)
        self._connect_timer.start(CONNECT_TIMEOUT_MS)
        sock.open(QUrl(url))

    def _on_open(self, sock: QWebSocket) -> None:
        if sock is not self._sock:
            return
        self._connect_timer.stop()
        self._failures = 0
        self.stats["connects"] += 1
        self.notice.emit("network", "")
        self._set_connected(True)

    def _on_error(self, sock: QWebSocket) -> None:
        if sock is not self._sock or self._connected:
            return   # an error after a good connect is followed by disconnected()
        self._fail(sock.errorString() or "connection failed")

    def _on_closed(self, sock: QWebSocket) -> None:
        if sock is not self._sock:
            return
        was = self._connected
        self._set_connected(False)
        if was and self._running:
            self._sock = None
            sock.deleteLater()
            self._retry.start(self._retry_ms)   # fresh token on every reconnect

    def _fail(self, message: str) -> None:
        if not self._running:
            return
        self._connect_timer.stop()
        self._failures += 1
        print(f"[call] media connect failed ({self._failures}): {message}", file=sys.stderr)
        self._drop_socket(close=True)
        if self._failures >= ERROR_AFTER_FAILURES:
            self.notice.emit("network", f"Call audio and video unavailable: {message}")
        delay = self._retry_ms * (3 if self._failures >= SLOW_AFTER_FAILURES else 1)
        self._retry.start(delay)

    def _drop_socket(self, close: bool) -> None:
        sock, self._sock = self._sock, None
        if sock is None:
            return
        try:
            sock.disconnected.disconnect()
            sock.errorOccurred.disconnect()
        except (RuntimeError, TypeError):
            pass
        if close:
            sock.abort()
        sock.deleteLater()

    def _set_connected(self, value: bool) -> None:
        if value != self._connected:
            self._connected = value
            self.connectedChanged.emit(value)

    # -- frames ---------------------------------------------------------------------------
    def _on_binary(self, data: QByteArray | bytes) -> None:
        try:
            f = wire.decode_frame(bytes(data))
        except wire.FrameError:
            self.stats["bad_frames"] += 1
            return
        if f.kind == wire.IN_AUDIO:
            self.stats["recv_audio"] += 1
            self.audioReceived.emit(f.payload)
        elif f.kind == wire.IN_VIDEO:
            self.stats["recv_video"] += 1
            self.videoReceived.emit(f.payload, f.video_metadata()[1])
        elif f.kind == wire.KEYFRAME_REQUEST:
            self.keyframeRequested.emit()   # JPEG mode: nothing to do
        # unknown kinds are ignored

    def _send(self, data: bytes) -> bool:
        s = self._sock
        if not self._connected or s is None:
            return False
        if s.bytesToWrite() > MAX_PENDING_BYTES:
            self.stats["dropped"] += 1   # the network is behind: drop, never wait
            return False
        s.sendBinaryMessage(QByteArray(data))
        return True

    @Slot(bytes)
    def send_audio(self, pcm: bytes) -> None:
        """Exactly 960 samples; anything else is dropped (the server insists)."""
        if len(pcm) == wire.AUDIO_FRAME_BYTES and self._send(wire.outgoing_audio(pcm)):
            self.stats["sent_audio"] += 1

    @Slot(bytes)
    def send_video(self, jpeg: bytes) -> None:
        if len(jpeg) <= wire.MAX_FRAME_BYTES - 1024 and self._send(wire.outgoing_video(jpeg)):
            self.stats["sent_video"] += 1
