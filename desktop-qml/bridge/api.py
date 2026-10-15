"""Async REST access for QML. Business logic stays in the Go backend; this is a
transport: `request(method, path, body, token)` -> `finished(token, status, body)`."""
from __future__ import annotations

import json
from urllib.parse import quote

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
        self.finished.emit(token, int(status), payload)
