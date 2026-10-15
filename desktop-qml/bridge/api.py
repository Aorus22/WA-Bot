"""Async REST access for QML. Business logic stays in the Go backend; this is a
transport: `request(method, path, body, token)` -> `finished(token, status, body)`."""
from __future__ import annotations

import json
from urllib.parse import quote

from typing import Any, Callable

from PySide6.QtCore import QByteArray, QObject, QUrl, Signal, Slot
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest

from .backend import BackendController


class Api(QObject):
    # token, HTTP status (0 = transport failure), response body as JSON text
    finished = Signal(str, int, str)

    def __init__(self, backend: BackendController, parent: QObject | None = None):
        super().__init__(parent)
        self._backend = backend
        self._nam = QNetworkAccessManager(self)
        self._py: dict[str, Callable[[int, Any], None]] = {}

    @Slot(str, result=str)
    def enc(self, value: str) -> str:
        return quote(value, safe="")

    @Slot(str, result=str)
    def mediaUrl(self, value: str) -> str:
        """Absolute URL for a backend-relative media path (same rules as GPUI `media_url`)."""
        if not value:
            return ""
        if value.startswith(("http://", "https://")):
            return value
        api = self._backend.apiUrl
        if value == "/api" or value.startswith("/api/"):
            return api.removesuffix("/api") + value
        return f"{api}{'' if value.startswith('/') else '/'}{value}"

    def fetch(self, method: str, path: str, body: Any = None,
              cb: Callable[[int, Any], None] | None = None) -> None:
        """Python-side call: `cb(status, parsed_json_or_text)`; status 0 = transport failure.
        `body` is a JSON-serialisable object or None."""
        self._seq = getattr(self, "_seq", 0) + 1
        token = f"py:{self._seq}"
        if cb:
            self._py[token] = cb
        self.request(method, path, json.dumps(body) if body is not None else "", token)

    @Slot(str, str, str, str)
    def request(self, method: str, path: str, body: str, token: str) -> None:
        base = self._backend.apiUrl
        if not base:
            self.finished.emit(token, 0, json.dumps({"error": "backend not ready"}))
            return
        req = QNetworkRequest(QUrl(base + path))
        data = QByteArray()
        if body:
            req.setHeader(QNetworkRequest.KnownHeaders.ContentTypeHeader, "application/json")
            data = QByteArray(body.encode())
        verb = method.upper().encode()
        reply = self._nam.sendCustomRequest(req, QByteArray(verb), data)
        reply.finished.connect(lambda r=reply, t=token: self._done(r, t))

    def _done(self, reply: QNetworkReply, token: str) -> None:
        status = reply.attribute(QNetworkRequest.Attribute.HttpStatusCodeAttribute) or 0
        payload = bytes(reply.readAll()).decode("utf-8", "replace")
        if reply.error() != QNetworkReply.NetworkError.NoError and not status:
            payload = json.dumps({"error": reply.errorString()})
        reply.deleteLater()
        cb = self._py.pop(token, None)
        if cb:
            try:
                parsed: Any = json.loads(payload) if payload else None
            except ValueError:
                parsed = payload
            cb(int(status), parsed)
            return
        self.finished.emit(token, int(status), payload)
