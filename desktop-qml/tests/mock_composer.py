"""Composer fixtures for the mock backend: send/reply/edit/media/sticker/poll/location/contact/typing/link-preview.
Load with `MockBackend(areas=["common", "composer"])`. Everything is recorded in `mb.state["composer"]["calls"]`
as {"name", "path", "body"} (JSON bodies parsed; multipart bodies parsed into {"fields", "file"}) so tests can assert.

    c = mb.state["composer"]
    c["fail"]["send-message"] = 500            # make that endpoint fail (name = first column below)
    c["link_previews"][url] = {...} | None     # None -> 404
    c["delay"]["send-media"] = 0.3             # seconds to stall a response
"""
from __future__ import annotations

import email
import time
from email.policy import default as _policy

from mock_common import BUDI, ME, SARI, TEAM

STICKERS = [
    {"id": "f1", "mediaUrl": "/media/sticker-wa-1.webp", "isAnimated": False, "source": "wa"},
    {"id": "f2", "mediaUrl": "/media/sticker-wa-2.webp", "isAnimated": True, "source": "wa"},
    {"id": "f3", "mediaUrl": "/media/sticker-custom-1.webp", "isAnimated": False, "source": "custom"},
]
RECENT = [{"id": "r1", "mediaUrl": "/media/sticker-recent-1.webp", "isAnimated": False, "source": "recent"}]
PARTICIPANTS = [
    {"jid": SARI, "name": "Sari Wulandari", "isAdmin": True, "isSuperAdmin": False},
    {"jid": BUDI, "name": "Budi Santoso", "isAdmin": False},
    {"jid": ME, "name": "Me", "isAdmin": False},
    {"jid": "6285555555555@s.whatsapp.net", "name": "", "isAdmin": False},
]


def parse_multipart(content_type: str, body: bytes) -> dict:
    msg = email.message_from_bytes(b"Content-Type: " + content_type.encode() + b"\r\n\r\n" + body, policy=_policy)
    fields: dict[str, str] = {}
    file: dict = {}
    for part in msg.iter_parts():
        name = part.get_param("name", header="content-disposition")
        fname = part.get_filename()
        payload = part.get_payload(decode=True) or b""
        if fname is not None or name == "file":
            file = {"name": fname, "content_type": part.get_content_type(), "data": payload}
        else:
            fields[name] = payload.decode()
    return {"fields": fields, "file": file}


def install(mb) -> None:
    st = mb.state.setdefault("composer", {})
    st.update({"calls": [], "fail": {}, "delay": {}, "n": 0,
               "link_previews": {"https://example.com/article": {
                   "url": "https://example.com/article", "title": "Example Article",
                   "description": "A short description of the linked page that may run long enough to wrap.",
                   "thumbnailUrl": "/media/thumb.png"}},
               "stickers": STICKERS, "recent": RECENT, "participants": PARTICIPANTS})

    def record(name, req, body=None):
        st["calls"].append({"name": name, "path": req.path, "body": body, "method": req.method})
        d = st["delay"].get(name)
        if d:
            time.sleep(d)

    def failing(name):
        code = st["fail"].get(name)
        return (code, {"error": f"mock failure for {name}"}) if code else None

    def next_id(prefix="srv"):
        st["n"] += 1
        return f"{prefix}-{st['n']}"

    def simple(method, pattern, name, ok=lambda req, body: {"status": "success", "id": next_id()}, json_body=True):
        @mb.route(method, pattern)
        def h(req):
            body = req.json() if (json_body and req.body) else None
            record(name, req, body)
            f = failing(name)
            return f or (200, ok(req, body))

    simple("POST", "/send-message", "send-message")
    simple("POST", "/send-sticker", "send-sticker")
    simple("POST", r"/chats/(?P<chat>[^/]+)/messages/(?P<mid>[^/]+)/reply", "reply", lambda r, b: {"status": "success"})
    simple("POST", r"/chats/(?P<chat>[^/]+)/messages/(?P<mid>[^/]+)/edit", "edit", lambda r, b: {"status": "success"})
    simple("POST", r"/chats/(?P<chat>[^/]+)/poll", "poll")
    simple("POST", r"/chats/(?P<chat>[^/]+)/location", "location")
    simple("POST", r"/chats/(?P<chat>[^/]+)/contact", "contact")
    simple("POST", r"/chats/(?P<chat>[^/]+)/typing", "typing", lambda r, b: {"status": "success"})
    simple("POST", r"/chats/(?P<chat>[^/]+)/presence-subscribe", "presence-subscribe", lambda r, b: {"status": "success"})

    @mb.route("POST", "/send-media")
    def send_media(req):
        parsed = parse_multipart(req.headers.get("Content-Type", ""), req.body)
        record("send-media", req, parsed)
        return failing("send-media") or (200, {"status": "success", "id": next_id("media")})

    @mb.route("GET", "/link-preview")
    def link_preview(req):
        url = req.q("url")
        record("link-preview", req, {"url": url})
        f = failing("link-preview")
        if f:
            return f
        meta = st["link_previews"].get(url)
        return (200, meta) if meta else (404, {"error": "no preview available"})

    @mb.route("GET", "/stickers/favorites")
    def favs(req):
        record("stickers-favorites", req)
        return failing("stickers-favorites") or (200, st["stickers"])

    @mb.route("GET", "/stickers/recent")
    def recent(req):
        record("stickers-recent", req)
        return failing("stickers-recent") or (200, st["recent"])

    @mb.route("GET", r"/groups/(?P<id>[^/]+)")
    def group(req):
        record("group", req)
        return 200, {"jid": req.params["id"], "name": "Tim Proyek", "participants": st["participants"],
                     "participantCount": len(st["participants"]), "ownRole": "member"}

