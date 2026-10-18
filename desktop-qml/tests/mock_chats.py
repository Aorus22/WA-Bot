"""Chat-list / chat-shell fixtures for tests/mock_backend.py: a realistic chat list (pinned, archived, groups,
muted, long names, unread/mentions), groups with participants, contacts and the chat/group REST endpoints
(`/chats/{id}/{read,pin,archive,mute,disappearing,search,media,docs,links,presence-subscribe}`, `/starred`,
`/chats/{id}/messages/{id}/{star,forward}`, `/groups/*`, `/contacts`, `/avatar/{jid}`).
Load: MockBackend(areas=["chats"])  (it installs `mock_common` first and then replaces the chat list).
Message senders use `from: "me"` like the real backend."""
from __future__ import annotations

import time

from mock_backend import jpeg_placeholder
import mock_common as common

NOW = common.NOW
ME = "me"
BUDI, SARI, TEAM = common.BUDI, common.SARI, common.TEAM
ANDI = "6283333333333@s.whatsapp.net"
DEWI = "6284444444444@s.whatsapp.net"
KELUARGA = "120363000000000002@g.us"
RINA = "6285555555555@s.whatsapp.net"
LONG = "6286666666666@s.whatsapp.net"
ARCH1 = "6287777777777@s.whatsapp.net"
ARCH2 = "120363000000000003@g.us"
NEWBIE = "6289999999999@s.whatsapp.net"
PNG_AVATAR_CHAT = SARI  # `/avatar/<jid>` answers with a picture for this chat only


def chat(id_, name, last_msg, ago, unread=0, group=False, **kw):
    return common.chat(id_, name, last_msg, ago, unread=unread, group=group, **kw)


def _chats():
    return [
        chat(TEAM, "Tim Proyek", "Sari: sudah di-deploy ya", 120, unread=3, group=True, pinnedAt=NOW - 5000,
             hasUnreadMention=True),
        chat(BUDI, "Budi Santoso", "Oke, sampai besok", 900, unread=1, pinnedAt=NOW - 9000),
        chat(SARI, "Sari Wulandari", "Terima kasih!", 3600 * 5, avatar="/media/sari.png"),
        chat(ANDI, "Andi", "📷 Photo", 3600 * 26),
        chat(DEWI, "Dewi", "Besok jadi?", 3600 * 50, muteMode="forever", unread=12),
        chat(KELUARGA, "Keluarga Besar", "Ibu: jangan lupa makan", 3600 * 80, group=True, unread=120),
        chat(RINA, "Rina Marlina", "{{md:KipTZW51IGhhcmkgaW5pKiog8J+Ygg==}}", 3600 * 100, disappearingSecs=86400),
        chat(LONG, "Nama Kontak Yang Sangat Sangat Panjang Sekali Untuk Menguji Pemotongan Teks", "Pesan terakhir yang juga sangat panjang sekali untuk menguji elipsis pada baris pratinjau pesan", 3600 * 130),
        chat(ARCH1, "Pak Hendra", "Siap pak", 3600 * 300, archived=True),
        chat(ARCH2, "Arsip Lama", "Admin: grup ditutup", 3600 * 500, group=True, archived=True),
    ]


def _msg(id_, chat_id, content, ago, out=False, type_="text", **kw):
    d = {"id": id_, "chatId": chat_id, "from": ME if out else chat_id, "to": chat_id if out else ME,
         "content": content, "timestamp": (NOW - ago) * 1000, "status": "read" if out else "received",
         "type": type_, "mediaUrl": None, "senderName": None, "starred": False}
    d.update(kw)
    return d


def _messages():
    team_sari = {"from": SARI, "senderName": "Sari"}
    return {
        BUDI: [_msg("b1", BUDI, "Halo Budi, apa kabar?", 3600, out=True), _msg("b2", BUDI, "Baik! Kamu gimana?", 3500),
               _msg("b3", BUDI, "Baik juga. Besok meeting jam 10 ya", 3400, out=True, starred=True),
               _msg("b4", BUDI, "Oke, sampai besok", 900),
               _msg("bm1", BUDI, "", 800, type_="image", mediaUrl="/media/b-photo-1.png"),
               _msg("bm2", BUDI, "laporan.pdf", 700, type_="document", mediaUrl="/media/laporan.pdf"),
               _msg("bl1", BUDI, "Cek https://example.com/artikel dan https://wa.me/628111", 600)],
        TEAM: [_msg("t1", TEAM, "Pagi semua", 7200, **team_sari), _msg("t2", TEAM, "Pagi! Build sudah hijau", 7100, out=True),
               _msg("t3", TEAM, "sudah di-deploy ya", 120, **team_sari)],
        SARI: [_msg("s1", SARI, "Terima kasih!", 3600 * 5)],
    }


def _group(jid, name, members, desc=None, admins=()):
    return {"jid": jid, "name": name, "description": desc, "owner": ME, "locked": False, "announce": False,
            "joinApproval": False, "memberAddMode": "all_member_add", "ownRole": "admin",
            "participantCount": len(members), "updatedAt": NOW,
            "participants": [{"jid": j, "name": n, "isAdmin": j in admins, "isSuperAdmin": j == ME} for j, n in members]}


def install(mb) -> None:
    common.install(mb)
    st = mb.state
    st["chats"] = _chats()
    st["messages"] = _messages()
    st["groups"] = {
        TEAM: _group(TEAM, "Tim Proyek", [(ME, "You"), (BUDI, "Budi Santoso"), (SARI, "Sari Wulandari"), (ANDI, "Andi")],
                     "Koordinasi proyek WA Bot. Harap tetap sopan.", admins=(ME, SARI)),
        KELUARGA: _group(KELUARGA, "Keluarga Besar", [(ME, "You"), (DEWI, "Dewi"), (RINA, "Rina Marlina")]),
    }
    st["group_invite_links"] = {jid: f"https://chat.whatsapp.com/mock-{index}" for index, jid in enumerate(st["groups"], 1)}
    st["group_photo_uploads"] = []
    st["contacts"] = [{"id": c["id"], "name": c["name"], "jid": c["id"], "avatar": ""} for c in st["chats"] if not c["isGroup"]]
    st["starred_extra"] = []
    st["message_pins"] = {}
    st["message_receipts"] = {}
    st["message_receipts"] = {}

    def find(chat_id):
        return next((c for c in st["chats"] if c["id"] == chat_id), None)

    def state_of(c):
        return {"chatId": c["id"], "archived": c["archived"], "pinnedAt": c["pinnedAt"], "muteMode": c["muteMode"],
                "mutedUntil": c["mutedUntil"]}

    @mb.route("GET", "/chats")
    def chats(req):
        return 200, st["chats"]

    @mb.route("GET", r"/chats/(?P<id>[^/]+)/messages")
    def messages(req):
        rows = list(st["messages"].get(req.params["id"], []))
        before = int(req.q("before", "0") or 0)
        after = int(req.q("after", "0") or 0)
        if before:
            rows = [m for m in rows if int(m.get("timestamp") or 0) < before]
            rows.sort(key=lambda m: int(m.get("timestamp") or 0), reverse=True)
        elif after:
            rows = [m for m in rows if int(m.get("timestamp") or 0) > after]
            rows.sort(key=lambda m: int(m.get("timestamp") or 0))
        else:
            rows.sort(key=lambda m: int(m.get("timestamp") or 0), reverse=True)
        limit = max(1, min(int(req.q("limit", "100") or 100), 500))
        return 200, rows[:limit]

    @mb.route("GET", r"/chats/(?P<chat>[^/]+)/messages/(?P<message>[^/]+)/receipts")
    def message_receipts(req):
        info = st["message_receipts"].get((req.params["chat"], req.params["message"]), {})
        receipts = list(info.get("receipts") or [])
        recipients = list(info.get("recipients") or [])
        return 200, {"receipts": receipts, "total": int(info.get("total", len(recipients))),
                     "recipients": recipients}

    @mb.route("GET", r"/chats/(?P<chat>[^/]+)/messages/(?P<message>[^/]+)/receipts")
    def message_receipts(req):
        key = (req.params["chat"], req.params["message"])
        info = st["message_receipts"].get(key, {})
        receipts = list(info.get("receipts") or [])
        recipients = list(info.get("recipients") or [])
        return 200, {"receipts": receipts, "total": int(info.get("total", len(recipients))),
                     "recipients": recipients}

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/read")
    def read(req):
        c = find(req.params["id"])
        if c:
            c["unread"] = 0
            c["hasUnreadMention"] = False
            mb.broadcast("chat_update", {"chatId": c["id"], "unread": 0})
        return 200, {"status": "ok"}

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/presence-subscribe")
    def presence(req):
        return 200, {"status": "ok"}

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/pin")
    def pin(req):
        c = find(req.params["id"])
        if not c:
            return 404, {"error": "chat not found"}
        c["pinnedAt"] = int(time.time()) if req.json().get("pinned") else None
        mb.broadcast("chat_state", state_of(c))
        return 200, state_of(c)

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/archive")
    def archive(req):
        c = find(req.params["id"])
        if not c:
            return 404, {"error": "chat not found"}
        c["archived"] = bool(req.json().get("archived"))
        return 200, state_of(c)

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/mute")
    def mute(req):
        c = find(req.params["id"])
        if not c:
            return 404, {"error": "chat not found"}
        mode = req.json().get("mode", "off")
        c["muteMode"] = "off" if mode == "off" else "forever" if mode == "forever" else "until"
        c["mutedUntil"] = None if mode in ("off", "forever") else (NOW + (8 * 3600 if mode == "8h" else 7 * 86400)) * 1000
        return 200, state_of(c)

    @mb.route("POST", r"/chats/(?P<id>[^/]+)/disappearing")
    def disappearing(req):
        c = find(req.params["id"])
        if not c:
            return 404, {"error": "chat not found"}
        c["disappearingSecs"] = int(req.json().get("seconds", 0))
        return 200, {"status": "ok", "seconds": c["disappearingSecs"]}

    @mb.route("GET", r"/chats/(?P<id>[^/]+)/search")
    def search(req):
        q = (req.q("q") or "").lower()
        rows = [m for m in st["messages"].get(req.params["id"], []) if q and q in m["content"].lower()]
        return 200, rows

    @mb.route("GET", r"/chats/(?P<id>[^/]+)/pins")
    def message_pins(req):
        return 200, st["message_pins"].get(req.params["id"], [])

    def typed(kind):
        def h(req):
            msgs = st["messages"].get(req.params["id"], [])
            if kind == "media":
                return 200, [m for m in msgs if m["type"] in ("image", "video")]
            if kind == "docs":
                return 200, [m for m in msgs if m["type"] == "document"]
            return 200, [m for m in msgs if "http" in m["content"]]
        return h

    for kind in ("media", "docs", "links"):
        mb.route("GET", rf"/chats/(?P<id>[^/]+)/{kind}")(typed(kind))

    @mb.route("GET", "/starred")
    def starred(req):
        rows = []
        for cid, msgs in st["messages"].items():
            c = find(cid)
            for m in msgs:
                if m.get("starred"):
                    rows.append({**m, "chatName": c["name"] if c else None, "starredAt": m["timestamp"] + 1000})
        rows.sort(key=lambda m: -m["starredAt"])
        before = req.q("before")
        if before:
            rows = [m for m in rows if m["starredAt"] < int(before)]
        return 200, rows

    @mb.route("POST", r"/chats/(?P<cid>[^/]+)/messages/(?P<mid>[^/]+)/star")
    def star(req):
        for m in st["messages"].get(req.params["cid"], []):
            if m["id"] == req.params["mid"]:
                m["starred"] = bool(req.json().get("starred"))
        return 200, {"status": "ok", "starred": bool(req.json().get("starred"))}

    @mb.route("POST", r"/chats/(?P<cid>[^/]+)/messages/(?P<mid>[^/]+)/pin")
    def pin_message(req):
        message = next((m for m in st["messages"].get(req.params["cid"], [])
                        if m["id"] == req.params["mid"]), None)
        if message is None:
            return 404, {"error": "message not found"}
        body = req.json() or {}
        pins = st["message_pins"].setdefault(req.params["cid"], [])
        pins[:] = [pin for pin in pins if pin.get("messageId") != req.params["mid"]]
        if body.get("pinned"):
            now = int(time.time() * 1000)
            pins.insert(0, {"messageId": req.params["mid"], "pinnedBy": "me", "pinnedAt": now,
                            "expiresAt": now + int(body.get("duration") or 0) * 1000, "message": dict(message)})
        st.setdefault("message_pin_calls", []).append((req.params["cid"], req.params["mid"], body))
        return 200, {"status": "ok", "pinned": bool(body.get("pinned"))}

    @mb.route("POST", r"/chats/(?P<cid>[^/]+)/messages/(?P<mid>[^/]+)/react")
    def react_message(req):
        body = req.json() or {}
        st.setdefault("message_reactions", []).append((req.params["cid"], req.params["mid"], body))
        return 200, {"status": "ok"}

    @mb.route("POST", r"/chats/(?P<cid>[^/]+)/messages/(?P<mid>[^/]+)/delete")
    def delete_message(req):
        message = next((m for m in st["messages"].get(req.params["cid"], [])
                        if m["id"] == req.params["mid"]), None)
        if message is None:
            return 404, {"error": "message not found"}
        message["revokedAt"] = int(time.time() * 1000)
        st.setdefault("message_deletions", []).append((req.params["cid"], req.params["mid"]))
        return 200, {"status": "ok", "revokedAt": message["revokedAt"]}

    @mb.route("POST", r"/chats/(?P<cid>[^/]+)/messages/(?P<mid>[^/]+)/forward")
    def forward(req):
        st.setdefault("forwards", []).append((req.params["cid"], req.params["mid"], req.json().get("targets")))
        return 200, {"status": "ok"}

    @mb.route("GET", "/contacts")
    def contacts(req):
        return 200, st["contacts"]

    @mb.route("GET", r"/groups/(?P<id>[^/]+)")
    def group(req):
        g = st["groups"].get(req.params["id"])
        return (200, g) if g else (404, {"error": "group not found"})

    @mb.route("PATCH", r"/groups/(?P<id>[^/]+)")
    def update_group(req):
        g = st["groups"].get(req.params["id"])
        if not g:
            return 404, {"error": "group not found"}
        g.update(req.json() or {})
        return 200, g

    @mb.route("POST", r"/groups/(?P<id>[^/]+)/photo")
    def group_photo(req):
        g = st["groups"].get(req.params["id"])
        if not g:
            return 404, {"error": "group not found"}
        st["group_photo_uploads"].append({
            "jid": req.params["id"],
            "content_type": req.headers.get("Content-Type", ""),
            "body": req.body,
        })
        return 200, g

    @mb.route("POST", r"/groups/(?P<id>[^/]+)/participants")
    def participants(req):
        g = st["groups"].get(req.params["id"])
        if not g:
            return 404, {"error": "group not found"}
        body = req.json()
        action = body.get("action")
        targets = set(body.get("jids", []))
        if action == "add":
            known = {p["jid"] for p in g["participants"]}
            for jid in body.get("jids", []):
                if jid in known:
                    continue
                c = find(jid)
                g["participants"].append({"jid": jid, "name": c["name"] if c else None, "isAdmin": False})
        elif action == "remove":
            g["participants"] = [p for p in g["participants"] if p["jid"] not in targets or p.get("isSuperAdmin")]
        elif action in ("promote", "demote"):
            for participant in g["participants"]:
                if participant["jid"] in targets and not participant.get("isSuperAdmin"):
                    participant["isAdmin"] = action == "promote"
        g["participantCount"] = len(g["participants"])
        return 200, g

    @mb.route("GET", r"/groups/(?P<id>[^/]+)/invite-link")
    def get_group_invite(req):
        link = st["group_invite_links"].get(req.params["id"])
        return (200, {"status": "success", "link": link}) if link else (404, {"error": "group not found"})

    @mb.route("POST", r"/groups/(?P<id>[^/]+)/invite-link")
    def reset_group_invite(req):
        if req.params["id"] not in st["groups"]:
            return 404, {"error": "group not found"}
        st["group_invite_links"][req.params["id"]] = f"https://chat.whatsapp.com/reset-{int(time.time() * 1000)}"
        return 200, {"status": "success", "link": st["group_invite_links"][req.params["id"]]}

    @mb.route("POST", "/groups/create")
    def create(req):
        body = req.json()
        jid = f"1203630000000{len(st['chats']) + 10:05d}@g.us"
        st["groups"][jid] = _group(jid, body["name"], [(ME, "You")] + [(p, p) for p in body.get("participants", [])])
        st["chats"].insert(0, chat(jid, body["name"], "", 0, group=True))
        mb.broadcast("chats_changed", {"reason": "group_created"})
        return 200, {"status": "ok", "group": st["groups"][jid]}

    @mb.route("POST", "/groups/join")
    def join(req):
        link = (req.json() or {}).get("link", "")
        if "chat.whatsapp.com" not in link:
            return 400, {"error": "invalid invite link"}
        return 200, {"status": "ok", "jid": "120363000000000099@g.us"}

    @mb.route("POST", r"/groups/(?P<id>[^/]+)/leave")
    def leave(req):
        st["chats"] = [c for c in st["chats"] if c["id"] != req.params["id"]]
        return 200, {"status": "ok"}

    @mb.route("POST", "/logout")
    def logout(req):
        st["logged_out"] = True
        st["logged_in"] = False
        mb.broadcast("logged_out")
        return 200, {"status": "ok"}

    @mb.route("GET", r"/avatar/(?P<id>.+)")
    def avatar(req):
        if req.params["id"] == PNG_AVATAR_CHAT:
            return 200, jpeg_placeholder((200, 90, 120), 96), "image/png"
        return 200, jpeg_placeholder((90, 120, 150), 64), "image/png"

    @mb.route("GET", r"/media/(?P<name>.+)")
    def media(req):
        failures = mb.state.setdefault("media_failures", {})
        remaining = int(failures.get(req.params["name"], 0))
        if remaining > 0:
            failures[req.params["name"]] = remaining - 1
            return 503, {"error": "temporary media failure"}
        return 200, jpeg_placeholder((60, 130, 200), 96), "image/png"
