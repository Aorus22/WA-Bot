"""Small multipart/form-data POST helper (QtNetwork, async) for the status/channel modules.
`bridge/api.py` only does JSON; this stays in our own module so the shared transport is untouched."""
from __future__ import annotations

import json
import os
from typing import Any, Callable

from PySide6.QtCore import QFile, QIODevice, QMimeDatabase, QObject, QUrl
from PySide6.QtNetwork import QHttpMultiPart, QHttpPart, QNetworkAccessManager, QNetworkReply, QNetworkRequest


def post_multipart(nam: QNetworkAccessManager, url: str, fields: dict[str, str], file_field: str,
                   file_path: str, cb: Callable[[int, Any], None], owner: QObject | None = None) -> bool:
    """POST `fields` + one file. `cb(status, parsed_json_or_text)`; status 0 = transport failure.
    Returns False (without calling cb) when the file cannot be opened."""
    f = QFile(file_path)
    if not f.open(QIODevice.OpenModeFlag.ReadOnly):
        return False
    multi = QHttpMultiPart(QHttpMultiPart.ContentType.FormDataType)
    for name, value in fields.items():
        part = QHttpPart()
        part.setHeader(QNetworkRequest.KnownHeaders.ContentDispositionHeader, f'form-data; name="{name}"')
        part.setBody(value.encode())
        multi.append(part)
    fp = QHttpPart()
    mime = QMimeDatabase().mimeTypeForFile(file_path).name() or "application/octet-stream"
    fp.setHeader(QNetworkRequest.KnownHeaders.ContentTypeHeader, mime)
    fp.setHeader(QNetworkRequest.KnownHeaders.ContentDispositionHeader,
                 f'form-data; name="{file_field}"; filename="{os.path.basename(file_path)}"')
    fp.setBodyDevice(f)
    f.setParent(multi)
    multi.append(fp)
    reply = nam.post(QNetworkRequest(QUrl(url)), multi)
    multi.setParent(reply)

    def done() -> None:
        status = reply.attribute(QNetworkRequest.Attribute.HttpStatusCodeAttribute) or 0
        payload = bytes(reply.readAll()).decode("utf-8", "replace")
        if reply.error() != QNetworkReply.NetworkError.NoError and not status:
            payload = json.dumps({"error": reply.errorString()})
        reply.deleteLater()
        try:
            parsed: Any = json.loads(payload) if payload else None
        except ValueError:
            parsed = payload
        cb(int(status), parsed)

    reply.finished.connect(done)
    return True
