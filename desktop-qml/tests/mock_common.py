"""Shared fixture chats/messages so every area's mock agrees on ids. Load via MockBackend(areas=["common", ...]).
Timestamps are unix seconds or milliseconds (the UI accepts both). `NOW` is fixed for stable screenshots."""
from __future__ import annotations

import time

ME = "628123456789@s.whatsapp.net"
BUDI = "6281111111111@s.whatsapp.net"
SARI = "6282222222222@s.whatsapp.net"
TEAM = "120363000000000001@g.us"
NOW = int(time.time())


def chat(id_, name, last_msg, ago, unread=0, group=False, **kw):
    d = {"id": id_, "name": name, "avatar": "", "lastMsg": last_msg, "lastTime": NOW - ago, "unread": unread,
         "hasUnreadMention": False, "isActive": True, "isGroup": group, "archived": False, "pinnedAt": None,
         "muteMode": "off", "mutedUntil": None, "disappearingSecs": 0}
    d.update(kw)
    return d


def msg(id_, chat_id, content, ago, out=False, type_="text", status=None, sender=None, **kw):
    d = {"id": id_, "chatId": chat_id, "from": ME if out else (sender or chat_id), "to": chat_id if out else ME,
         "content": content, "timestamp": (NOW - ago) * 1000, "status": status or ("read" if out else "received"),
         "type": type_, "mediaUrl": None, "senderName": None, "starred": False}
    d.update(kw)
    return d


CHATS = [
    chat(TEAM, "Tim Proyek", "Sari: sudah di-deploy ya", 120, unread=3, group=True, pinnedAt=NOW - 5000),
    chat(BUDI, "Budi Santoso", "Oke, sampai besok", 900, unread=1),
    chat(SARI, "Sari Wulandari", "Terima kasih!", 3600 * 5),
    chat("6283333333333@s.whatsapp.net", "Andi", "📷 Photo", 3600 * 26),
    chat("6284444444444@s.whatsapp.net", "Dewi", "Besok jadi?", 3600 * 50, muteMode="always"),
    chat("120363000000000002@g.us", "Keluarga", "Ibu: jangan lupa makan", 3600 * 80, group=True),
]

MESSAGES = {
    BUDI: [
        msg("b1", BUDI, "Halo Budi, apa kabar?", 3600, out=True),
        msg("b2", BUDI, "Baik! Kamu gimana?", 3500),
        msg("b3", BUDI, "Baik juga. Besok meeting jam 10 ya", 3400, out=True),
        msg("b4", BUDI, "Oke, sampai besok", 900),
    ],
    TEAM: [
        msg("t1", TEAM, "Pagi semua", 7200, sender="6282222222222@s.whatsapp.net", senderName="Sari"),
        msg("t2", TEAM, "Pagi! Build sudah hijau", 7100, out=True),
        msg("t3", TEAM, "sudah di-deploy ya", 120, sender="6282222222222@s.whatsapp.net", senderName="Sari"),
    ],
    SARI: [msg("s1", SARI, "Terima kasih!", 3600 * 5)],
}


def install(mb) -> None:
    mb.state.setdefault("chats", CHATS)
    mb.state.setdefault("messages", MESSAGES)
