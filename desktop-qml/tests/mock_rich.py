"""Extra typed message cards for rendering screenshots; only used by explicit local test runs."""
from __future__ import annotations

import mock_chats
from mock_common import BUDI


def install(mb) -> None:
    rows = mb.state.setdefault("messages", {}).setdefault(BUDI, [])
    rows.extend([
        mock_chats._msg("rich-poll", BUDI, "", 360, type_="poll", extra={"poll": {
            "question": "Kapan mulai review?",
            "options": [{"name": "Pagi"}, {"name": "Siang"}, {"name": "Sore"}],
            "votes": {"6281111111111@s.whatsapp.net": ["Pagi"], "me": ["Siang"]},
            "multiSelect": False,
        }}),
        mock_chats._msg("rich-location", BUDI, "", 300, type_="location", extra={"location": {
            "latitude": -6.2, "longitude": 106.816666, "name": "Jakarta", "address": "Indonesia",
        }}),
        mock_chats._msg("rich-contact", BUDI, "", 240, type_="contact", extra={"contact": {
            "contacts": [{"displayName": "Sari Wulandari", "vcard": "BEGIN:VCARD\nTEL;TYPE=CELL:+6282222222222\nEND:VCARD"}],
        }}),
        mock_chats._msg("rich-link", BUDI, "Read the source", 180, extra={"linkPreview": {
            "url": "https://example.com/article", "title": "Example article",
            "description": "A link preview used for keyboard accessibility coverage.",
        }}),
    ])
    rows.sort(key=lambda row: row.get("timestamp", 0))

    @mb.route("POST", r"/chats/(?P<chat>[^/]+)/messages/(?P<message>[^/]+)/vote")
    def vote(req):
        options = list((req.json() or {}).get("options") or [])
        mb.state.setdefault("poll_votes", []).append((req.params["chat"], req.params["message"], options))
        return 200, {"ok": True}
