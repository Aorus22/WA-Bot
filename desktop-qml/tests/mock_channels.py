"""Channel fixtures for the mock backend (`MockBackend(areas=["common", "channels"])`).

Shapes follow backend-client `dto/channel.rs`. `mb.state["channels"]` = followed channels,
`mb.state["channel_posts"][jid]` = posts (serverId ascending). One channel has 75 posts for paging.
`mb.state["channel_calls"]` records mutations. WS payloads match the Go backend (`channelId`).
"""
from __future__ import annotations

import time
from urllib.parse import unquote

TECH = "120363111111111111@newsletter"
NEWS = "120363222222222222@newsletter"
LONG = "120363333333333333@newsletter"
QUIET = "120363444444444444@newsletter"

REACTIONS = [{"👍": 120, "❤️": 40}, {"😂": 7}, {}, {"🙏": 3, "👍": 1, "😮": 12}]


def channels() -> list[dict]:
    return [
        {"jid": TECH, "name": "Tech News Indonesia", "description": "Kabar teknologi harian", "subscribers": 1250431,
         "inviteCode": "TechABC", "avatar": "", "muted": False, "verified": True},
        {"jid": NEWS, "name": "Berita Pagi", "description": "", "subscribers": 8200, "inviteCode": "NewsXYZ",
         "avatar": "", "muted": True, "verified": False},
        {"jid": LONG, "name": "Channel dengan nama yang sangat panjang sekali sehingga harus dipotong", "description": "x",
         "subscribers": 12, "inviteCode": "Long", "avatar": "", "muted": False, "verified": True},
        {"jid": QUIET, "name": "Kosong", "description": "", "subscribers": 0, "inviteCode": "", "avatar": "",
         "muted": False, "verified": False},
    ]


def post(i, ago_min, content=None, media=None, views=None, reactions=None):
    now = int(time.time() * 1000)
    return {"id": f"p{i}", "serverId": i, "type": "text" if not media else media, "timestamp": now - ago_min * 60_000,
            "viewsCount": views if views is not None else 100 + i * 7, "reactions": reactions,
            "content": content if content is not None else f"Post nomor {i}: pembaruan singkat dari channel.",
            "mediaType": media or ""}


def posts() -> dict[str, list[dict]]:
    long_feed = [post(i, (76 - i) * 90, reactions=REACTIONS[i % 4] or None) for i in range(1, 76)]
    long_feed[-1]["content"] = "Terbaru! Baca selengkapnya di https://example.com/artikel/panjang?id=42 sekarang juga."
    return {
        TECH: [
            post(1, 60 * 30, "Selamat datang di Tech News", reactions={"👍": 5}),
            post(2, 60 * 26, "Rilis QML baru: kini dengan parity penuh.\nBaris kedua untuk menguji pembungkusan teks.",
                 reactions={"👍": 120, "❤️": 40, "😂": 2}),
            post(3, 60 * 5, "", media="image", views=9800),
            post(4, 90, "Video demo", media="video", views=500),
            post(5, 3, "Teks pendek.", views=12),
        ],
        NEWS: [post(1, 120, "Berita pagi ini: cuaca cerah.")],
        LONG: long_feed,
        QUIET: [],
    }


def install(mb) -> None:
    mb.state.setdefault("channels", channels())
    mb.state.setdefault("channel_posts", posts())
    mb.state.setdefault("channel_calls", [])
    calls = mb.state["channel_calls"]

    def chan(jid):
        return next((c for c in mb.state["channels"] if c["jid"] == jid), None)

    @mb.route("GET", "/channels")
    def list_(req):
        if mb.state.get("channels_fail"):
            return 502, {"error": "boom"}
        return 200, mb.state["channels"]

    @mb.route("GET", "/channels/preview")
    def preview(req):
        url = req.q("url", "")
        if "whatsapp.com/channel/" not in url:
            return 502, {"error": "invalid channel link"}
        code = url.rsplit("/", 1)[-1]
        return 200, {"jid": f"120363{abs(hash(code)) % 10**12:012d}@newsletter", "name": f"Preview {code}",
                     "subscribers": 4321, "description": "Deskripsi channel dari pratinjau."}

    @mb.route("POST", "/channels")
    def follow(req):
        url = (req.json() or {}).get("url", "")
        if "whatsapp.com/channel/" not in url:
            return 502, {"error": "invalid channel link"}
        code = url.rsplit("/", 1)[-1]
        jid = f"120363{abs(hash(code)) % 10**12:012d}@newsletter"
        calls.append(("follow", url))
        if not chan(jid):
            mb.state["channels"].append({"jid": jid, "name": f"Preview {code}", "description": "", "subscribers": 4321,
                                         "inviteCode": code, "avatar": "", "muted": False, "verified": False})
            mb.state["channel_posts"].setdefault(jid, [])
        mb.broadcast("channels_changed", {"reason": "followed"})
        return 200, {"status": "success", "channel": chan(jid)}

    @mb.route("DELETE", r"/channels/(?P<id>[^/]+)")
    def unfollow(req):
        jid = unquote(req.params["id"])
        calls.append(("unfollow", jid))
        if mb.state.get("unfollow_fail"):
            return 502, {"error": "cannot unfollow"}
        mb.state["channels"] = [c for c in mb.state["channels"] if c["jid"] != jid]
        mb.broadcast("channels_changed", {"reason": "unfollowed"})
        return 200, {"status": "success"}

    @mb.route("POST", r"/channels/(?P<id>[^/]+)/mute")
    def mute(req):
        c = chan(unquote(req.params["id"]))
        if not c:
            return 404, {"error": "unknown channel"}
        if mb.state.get("mute_fail"):
            return 502, {"error": "mute failed"}
        c["muted"] = bool((req.json() or {}).get("muted"))
        calls.append(("mute", c["jid"], c["muted"]))
        return 200, {"status": "success"}

    @mb.route("GET", r"/channels/(?P<id>[^/]+)/messages")
    def messages(req):
        jid = unquote(req.params["id"])
        if mb.state.get("channel_posts_fail"):
            return 502, {"error": "feed down"}
        feed = mb.state["channel_posts"].get(jid, [])
        count = int(req.q("count", "30"))
        before = req.q("before")
        if before:
            feed = [p for p in feed if p["serverId"] < int(before)]
        return 200, feed[-count:][::-1]  # newest first, like the backend; the UI sorts ascending

    @mb.route("POST", r"/channels/(?P<id>[^/]+)/messages/(?P<sid>\d+)/react")
    def react(req):
        jid, sid = unquote(req.params["id"]), int(req.params["sid"])
        body = req.json() or {}
        if mb.state.get("react_fail"):
            return 502, {"error": "react failed"}
        calls.append(("react", jid, sid, body.get("messageId"), body.get("emoji")))
        for p in mb.state["channel_posts"].get(jid, []):
            if p["serverId"] == sid:
                r = dict(p.get("reactions") or {})
                r[body["emoji"]] = r.get(body["emoji"], 0) + 1
                p["reactions"] = r
                mb.broadcast("channel_update", {"channelId": jid, "messages": [p]})
        return 200, {"status": "success"}
