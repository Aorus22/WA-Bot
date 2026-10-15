"""Multipart `POST /send-media` for the composer (GPUI `backend-client/src/multipart.rs`).

Form fields, same names as GPUI: secret, target, message (caption), type, file, and for voice notes
ptt/seconds. The file part streams from disk (QFile) or from memory (QBuffer); nothing blocks the UI thread
beyond the event-loop-driven reads Qt itself does for a network upload."""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable

from PySide6.QtCore import QBuffer, QByteArray, QFile, QIODevice, QMimeDatabase, QObject, QUrl
from PySide6.QtNetwork import QHttpMultiPart, QHttpPart, QNetworkAccessManager, QNetworkReply, QNetworkRequest

from .composer_util import audio_mime

SECRET = "default-secret"  # GPUI falls back to this when API_SECRET is unset


def guess_mime(file_name: str) -> str:
    m = audio_mime(file_name)
    if m:
        return m
    name = QMimeDatabase().mimeTypeForFile(file_name, QMimeDatabase.MatchMode.MatchExtension).name()
    return name or "application/octet-stream"


def _quote(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\r", "").replace("\n", "")


class Uploader(QObject):
    def __init__(self, backend, parent: QObject | None = None):
        super().__init__(parent)
        self._backend = backend
        self._nam = QNetworkAccessManager(self)

    def _text_part(self, name: str, value: str) -> QHttpPart:
        p = QHttpPart()
        p.setHeader(QNetworkRequest.KnownHeaders.ContentDispositionHeader, f'form-data; name="{name}"')
        p.setBody(value.encode())
        return p

    def send_media(self, target: str, mtype: str, file_name: str, *, path: str | None = None,
                   data: bytes | None = None, caption: str = "", mime: str | None = None,
                   ptt: bool | None = None, seconds: float | None = None,
                   cb: Callable[[int, Any], None]) -> bool:
        """Upload a file (by `path`, or in-memory `data`). `cb(status, parsed_json)`; status 0 = transport
        failure / unreadable file. Returns False (and does not call cb) when the file cannot be opened."""
        base = self._backend.apiUrl
        if not base:
            cb(0, {"error": "backend not ready"})
            return True
        multi = QHttpMultiPart(QHttpMultiPart.ContentType.FormDataType)
        for name, value in (("secret", SECRET), ("target", target), ("message", caption), ("type", mtype)):
            multi.append(self._text_part(name, value))
        if ptt is not None:
            multi.append(self._text_part("ptt", "true" if ptt else "false"))
        if seconds is not None:
            # the backend parses an unsigned integer; a fractional value would be ignored
            multi.append(self._text_part("seconds", str(max(1, round(seconds)))))
        part = QHttpPart()
        part.setHeader(QNetworkRequest.KnownHeaders.ContentDispositionHeader,
                       f'form-data; name="file"; filename="{_quote(file_name)}"')
        part.setHeader(QNetworkRequest.KnownHeaders.ContentTypeHeader, mime or guess_mime(file_name))
        if data is not None:
            dev: QIODevice = QBuffer(multi)
            dev.setData(QByteArray(data))
            dev.open(QIODevice.OpenModeFlag.ReadOnly)
        else:
            dev = QFile(path or "", multi)
            if not dev.open(QIODevice.OpenModeFlag.ReadOnly):
                multi.deleteLater()
                return False
        part.setBodyDevice(dev)
        multi.append(part)
        reply = self._nam.post(QNetworkRequest(QUrl(base + "/send-media")), multi)
        multi.setParent(reply)  # lives (with its devices) until the reply is gone
        reply.finished.connect(lambda r=reply: self._done(r, cb))
        return True

    @staticmethod
    def _done(reply: QNetworkReply, cb: Callable[[int, Any], None]) -> None:
        status = int(reply.attribute(QNetworkRequest.Attribute.HttpStatusCodeAttribute) or 0)
        payload = bytes(reply.readAll()).decode("utf-8", "replace")
        if reply.error() != QNetworkReply.NetworkError.NoError and not status:
            payload = json.dumps({"error": reply.errorString()})
        reply.deleteLater()
        try:
            parsed: Any = json.loads(payload) if payload else None
        except ValueError:
            parsed = payload
        cb(status, parsed)


def file_name_of(path: str) -> str:
    return Path(path).name or "file"


def url_to_path(url: str) -> str:
    """`file:///x/y` (or an already-plain path) -> local path."""
    u = QUrl(url)
    return u.toLocalFile() if u.isLocalFile() else url
