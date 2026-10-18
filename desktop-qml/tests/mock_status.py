"""Status fixtures for the mock backend (`MockBackend(areas=["common", "status"])`).

Shapes follow backend-client `dto/status.rs`; timestamps are milliseconds like the Go backend.
State lives in `mb.state["statuses"]` (list of groups). Routes: GET /statuses, POST /statuses/text,
POST /statuses/media (multipart), POST /statuses/{id}/viewed, POST /statuses/{id}/reply,
GET /statuses/{id}/media, GET /avatar/{jid}. `mb.state["status_calls"]` records mutations for asserts.
"""
from __future__ import annotations

import functools
import os
import shutil
import struct
import subprocess
import tempfile
import time
import zlib
from email.parser import BytesParser
from email.policy import default as _policy

from mock_common import BUDI, ME, SARI

DAY_MS = 24 * 3600 * 1000


@functools.lru_cache(maxsize=4)
def gradient_png(w=360, h=640, c1=(20, 90, 140), c2=(240, 140, 60)) -> bytes:
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            t = (x / w + y / h) / 2
            row += bytes(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))
        rows.append(bytes(row))

    def chunk(t: bytes, d: bytes) -> bytes:
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b""))


def make_media_files(dirpath: str, seconds: int = 4) -> dict[str, str]:
    """Tiny real mp4 + ogg via ffmpeg when available (video/audio viewer tests). {} when it is not."""
    ff = shutil.which("ffmpeg")
    if not ff:
        return {}
    out = {}
    mp4, ogg = os.path.join(dirpath, "s.mp4"), os.path.join(dirpath, "s.ogg")
    try:
        subprocess.run([ff, "-v", "quiet", "-y", "-f", "lavfi", "-i", f"testsrc=size=320x240:rate=15:duration={seconds}",
                        "-f", "lavfi", "-i", f"sine=frequency=440:duration={seconds}", "-c:v", "libx264", "-pix_fmt",
                        "yuv420p", "-c:a", "aac", "-shortest", mp4], check=True, timeout=60)
        out["video"] = mp4
        subprocess.run([ff, "-v", "quiet", "-y", "-f", "lavfi", "-i", f"sine=frequency=330:duration={seconds}",
                        "-c:a", "libvorbis", ogg], check=True, timeout=60)
        out["audio"] = ogg
    except (subprocess.SubprocessError, OSError):
        pass
    return out


def entry(id_, sender, typ, content, ago_min, viewed=False, name=None, media=None, expires_in_ms=None):
    now = int(time.time() * 1000)
    ts = now - ago_min * 60_000
    return {"id": id_, "sender": sender, "senderName": name, "content": content, "mediaUrl": media, "type": typ,
            "timestamp": ts, "expiresAt": now + expires_in_ms if expires_in_ms is not None else ts + DAY_MS,
            "viewed": viewed}


def group(sender, name, statuses, avatar=None):
    return {"sender": sender, "name": name, "avatar": avatar, "allViewed": all(s["viewed"] for s in statuses),
            "statuses": statuses, "latestTime": max((s["timestamp"] for s in statuses), default=0)}


def fixtures() -> list[dict]:
    andi = "6283333333333@s.whatsapp.net"
    dewi = "6284444444444@s.whatsapp.net"
    return [
        group(ME, "Status Saya", [
            entry("own1", ME, "text", "Lagi ngoding QML", 90, viewed=True),
            entry("own2", ME, "image", "Pemandangan sore", 30, viewed=True),
        ]),
        group(BUDI, "Budi Santoso", [
            entry("b1", BUDI, "text", "Selamat pagi semuanya! Semoga hari ini menyenangkan dan penuh semangat.", 50),
            entry("b2", BUDI, "image", "Kopi pagi", 40),
            entry("b3", BUDI, "video", "Video singkat", 35),
        ]),
        group(SARI, "Sari Wulandari", [entry("s1", SARI, "audio", "", 300, viewed=True)]),
        group(andi, "Andi", [entry("a1", andi, "text", "Status panjang " + "lorem ipsum dolor sit amet " * 12, 600)]),
        # Keep this old story live so the list test can cover the "Yesterday" time label.
        group(dewi, None, [entry("d1", dewi, "text", "Tanpa nama", 60 * 25, expires_in_ms=60 * 60_000)]),
    ]


def parse_multipart(req) -> tuple[dict, dict]:
    raw = b"Content-Type: " + req.headers["Content-Type"].encode() + b"\r\n\r\n" + req.body
    msg = BytesParser(policy=_policy).parsebytes(raw)
    fields, files = {}, {}
    for part in msg.iter_parts():
        name = part.get_param("name", header="content-disposition")
        fn = part.get_filename()
        if fn:
            files[name] = (fn, part.get_payload(decode=True))
        else:
            fields[name] = part.get_content()
    return fields, files


def install(mb) -> None:
    mb.state.setdefault("statuses", fixtures())
    mb.state.setdefault("status_calls", [])
    tmp = tempfile.mkdtemp(prefix="mock-status-")
    media = make_media_files(tmp)
    png = gradient_png()
    calls = mb.state["status_calls"]

    def find(id_):
        for g in mb.state["statuses"]:
            for e in g["statuses"]:
                if e["id"] == id_:
                    return g, e
        return None, None

    @mb.route("GET", "/statuses")
    def list_(req):
        if mb.state.get("statuses_fail"):
            return 502, {"error": "boom"}
        now = int(time.time() * 1000)
        out = []
        for g in mb.state["statuses"]:
            live = [e for e in g["statuses"] if e["expiresAt"] > now]
            if not live and g["name"] != "Status Saya":
                continue
            out.append({**g, "statuses": live, "allViewed": all(e["viewed"] for e in live),
                        "latestTime": max((e["timestamp"] for e in live), default=0)})
        return 200, out

    def own_group():
        g = next((g for g in mb.state["statuses"] if g["name"] == "Status Saya"), None)
        if g is None:
            g = group(ME, "Status Saya", [])
            mb.state["statuses"].insert(0, g)
        return g

    @mb.route("POST", "/statuses/text")
    def post_text(req):
        body = req.json()
        if not str(body.get("text", "")).strip():
            return 400, {"error": "text is required"}
        calls.append(("text", body))
        sid = f"own{int(time.time() * 1000)}"
        g = own_group()
        g["statuses"].append(entry(sid, ME, "text", body["text"], 0, viewed=True))
        mb.broadcast("status_new", {"sender": ME, "id": sid})
        return 200, {"status": "success", "id": sid}

    @mb.route("POST", "/statuses/media")
    def post_media(req):
        fields, files = parse_multipart(req)
        if "file" not in files:
            return 400, {"error": "file is required"}
        calls.append(("media", {"type": fields.get("type"), "caption": fields.get("caption"),
                                "filename": files["file"][0], "size": len(files["file"][1])}))
        sid = f"own{int(time.time() * 1000)}"
        g = own_group()
        g["statuses"].append(entry(sid, ME, fields.get("type", "image"), fields.get("caption") or "[Media]", 0, viewed=True))
        mb.broadcast("status_new", {"sender": ME, "id": sid})
        return 200, {"status": "success", "id": sid}

    @mb.route("POST", r"/statuses/(?P<id>[^/]+)/viewed")
    def viewed(req):
        g, e = find(req.params["id"])
        if e is None:
            return 404, {"error": "status not found"}
        calls.append(("viewed", req.params["id"]))
        e["viewed"] = True
        return 200, {"status": "success"}

    @mb.route("POST", r"/statuses/(?P<id>[^/]+)/reply")
    def reply(req):
        g, e = find(req.params["id"])
        if e is None:
            return 404, {"error": "status not found"}
        if g["name"] == "Status Saya":
            return 400, {"error": "cannot reply to your own status"}
        text = (req.json() or {}).get("text", "")
        if not text.strip():
            return 400, {"error": "text is required"}
        if mb.state.get("reply_fail"):
            return 502, {"error": "send failed"}
        calls.append(("reply", req.params["id"], text))
        return 200, {"status": "success", "id": "reply1", "chatId": g["sender"]}

    @mb.route("GET", r"/statuses/(?P<id>[^/]+)/media")
    def media_(req):
        g, e = find(req.params["id"])
        if e is None:
            return 404, {"error": "status not found"}
        if e["type"] == "video" and "video" in media:
            return 200, open(media["video"], "rb").read(), "video/mp4"
        if e["type"] == "audio" and "audio" in media:
            return 200, open(media["audio"], "rb").read(), "audio/ogg"
        if e["type"] in ("video", "audio"):
            return 410, {"error": "status media no longer available"}
        return 200, png, "image/png"

    @mb.route("GET", r"/avatar/(?P<jid>.+)")
    def avatar(req):
        return 200, gradient_png(64, 64), "image/png"
