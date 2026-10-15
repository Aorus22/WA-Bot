"""Statuses / Channels bridges against the mock backend (REST + WS), no WhatsApp account involved."""
from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import pytest
from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend, jpeg_placeholder  # noqa: E402
import mock_channels  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.channels import Channels  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.statuses import Statuses  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402
from mock_common import BUDI, ME, SARI  # noqa: E402


def spin(cond, ms=6000):
    loop, t = QEventLoop(), QTimer()
    t.setInterval(20)
    t.timeout.connect(lambda: loop.quit() if cond() else None)
    QTimer.singleShot(ms, loop.quit)
    t.start()
    loop.exec()
    t.stop()
    return cond()


def settle(ms=150):
    spin(lambda: False, ms)


class Rig:
    def __init__(self):
        self.app = QCoreApplication.instance() or QCoreApplication([])
        self.mb = MockBackend(areas=["common", "status", "channels"]).start()
        self.backend = BackendController(["--url", self.mb.url])
        self.backend.start()
        assert spin(lambda: self.backend.state == "ready")
        self.hub = EventHub(self.backend)
        self.hub.start()
        assert spin(lambda: self.hub.connected)
        self.toasts: list[tuple[str, str]] = []
        self.toast = ToastBus()
        self.toast.shown.connect(lambda i, m, k: self.toasts.append((k, m)))
        self.api = Api(self.backend)
        self.appstate = AppState(self.hub)
        self.svc = Services(backend=self.backend, api=self.api, hub=self.hub, settings=None, theme=None,
                            app=self.appstate, qt_app=self.app, toast=self.toast)

    def close(self):
        self.hub.stop()
        self.mb.stop()


@pytest.fixture()
def rig():
    r = Rig()
    yield r
    r.close()


@pytest.fixture()
def st(rig):
    s = Statuses(rig.svc)
    assert spin(lambda: not s.loading)
    s.rig = rig
    return s


@pytest.fixture()
def ch(rig):
    c = Channels(rig.svc)
    assert spin(lambda: not c.loading)
    c.rig = rig
    return c


def rows(model):
    return list(model.rows())


# =============================================================== statuses
def test_status_list_rings_and_labels(st):
    groups = rows(st.model)
    assert [g["sender"] for g in groups][:2] == [BUDI, SARI]
    assert len(groups) == 4  # own group is separate
    budi = groups[0]
    assert budi["displayName"] == "Budi Santoso" and budi["initial"] == "B" and budi["allViewed"] is False
    assert budi["count"] == 3 and budi["unviewed"] == 3 and budi["timeLabel"].startswith("Today at ")
    assert groups[1]["allViewed"] is True  # Sari's single status was seen
    unnamed = next(g for g in groups if g["sender"].startswith("6284444"))
    assert unnamed["displayName"] == "6284444444444"  # sender local part when the backend has no name
    assert unnamed["timeLabel"].startswith("Yesterday at ")  # 25 h ago
    assert st.own["count"] == 2 and st.own["name"] == "Status Saya"
    assert st.unviewedCount == 3  # Budi, Andi, unnamed
    assert st.error == "" and st.loading is False


def test_open_group_marks_viewed_and_navigates(st):
    st.openGroup(BUDI)
    assert st.viewerOpen and st.viewerCount == 3 and st.viewerIndex == 0
    assert st.viewerGroup["displayName"] == "Budi Santoso" and st.viewerIsOwn is False
    assert st.viewerEntry["id"] == "b1" and st.viewerEntry["type"] == "text"
    assert spin(lambda: ("viewed", "b1") in st.rig.mb.state["status_calls"])
    st.next()
    assert st.viewerIndex == 1 and st.viewerEntry["type"] == "image"
    assert st.viewerEntry["src"].endswith("/statuses/b2/media")
    assert st.viewerEntry["caption"] == "Kopi pagi"
    st.next()
    assert st.viewerEntry["type"] == "video"
    assert spin(lambda: ("viewed", "b3") in st.rig.mb.state["status_calls"])
    assert rows(st.model)[0]["allViewed"] is True  # ring turned grey as soon as every story was seen
    st.prev()
    assert st.viewerIndex == 1
    st.goTo(5)  # past the end closes (GPUI navigate)
    assert not st.viewerOpen
    assert spin(lambda: st.unviewedCount == 2)


def test_prev_from_first_closes_and_reloads(st):
    st.openGroup(BUDI)
    st.prev()
    assert not st.viewerOpen
    assert spin(lambda: any(r[1].startswith("/api/statuses") or r[1].startswith("/statuses") for r in
                            st.rig.mb.requests[-3:]))


def test_own_group_has_no_viewed_receipt_and_no_reply(st):
    st.openOwn()
    assert st.viewerOpen and st.viewerIsOwn and st.viewerCount == 2
    settle()
    assert not [c for c in st.rig.mb.state["status_calls"] if c[0] == "viewed"]
    st.reply("hi")
    settle()
    assert not [c for c in st.rig.mb.state["status_calls"] if c[0] == "reply"]


def test_empty_own_row_requests_composer(st):
    st.rig.mb.state["statuses"][0]["statuses"] = []
    st.refresh()
    assert spin(lambda: st.own.get("count") == 0)
    seen = []
    st.composeRequested.connect(lambda: seen.append(1))
    st.openOwn()
    assert seen == [1] and not st.viewerOpen


def test_reply_flow(st):
    st.openGroup(BUDI)
    sent = []
    st.replySent.connect(sent.append)
    st.reply("   ")  # blank is ignored
    st.reply(" Mantap ")
    assert st.replying is True
    assert spin(lambda: sent)
    assert sent == [BUDI]
    assert ("reply", "b1", "Mantap") in st.rig.mb.state["status_calls"]
    assert ("success", "Balasan terkirim") in st.rig.toasts and st.replying is False
    opened = []
    st.rig.appstate.activeChatChanged.connect(lambda: opened.append(st.rig.appstate.activeChatId))
    st.openChat(BUDI)
    assert st.rig.appstate.path == f"chat/{BUDI}" and opened == [BUDI]


def test_reply_failure_toasts(st):
    st.rig.mb.state["reply_fail"] = True
    st.openGroup(BUDI)
    st.reply("x")
    assert spin(lambda: any(k == "error" for k, _ in st.rig.toasts))
    assert st.rig.toasts[-1] == ("error", "Failed to reply: send failed") and st.replying is False


def test_post_text_status(st):
    st.postText("   ", 0xFF128C7E)  # blank never posts
    settle()
    assert not st.rig.mb.state["status_calls"]
    st.postText("Halo dunia", 0xFF6A3080)
    assert st.posting is True
    assert spin(lambda: st.own.get("count") == 3)
    kind, body = st.rig.mb.state["status_calls"][-1]
    assert kind == "text" and body == {"text": "Halo dunia", "background": 0xFF6A3080}
    assert ("success", "Status posted") in st.rig.toasts and st.posting is False


def test_post_media_status_multipart(st, tmp_path):
    f = tmp_path / "pic.png"
    f.write_bytes(jpeg_placeholder())
    st.postMedia(str(f), " Caption saya ")
    assert spin(lambda: st.own.get("count") == 3)
    kind, info = st.rig.mb.state["status_calls"][-1]
    assert kind == "media" and info["type"] == "image" and info["caption"] == "Caption saya"
    assert info["filename"] == "pic.png" and info["size"] == len(jpeg_placeholder())
    mp4 = tmp_path / "clip.mp4"
    mp4.write_bytes(b"not really a video")
    st.postMedia("file://" + str(mp4), "")
    assert spin(lambda: st.own.get("count") == 4)
    assert st.rig.mb.state["status_calls"][-1][1]["type"] == "video"
    st.postMedia(str(tmp_path / "missing.png"), "")
    assert st.rig.toasts[-1][0] == "error" and st.posting is False


def test_ws_status_new_refreshes_and_viewer_keeps_place(st):
    st.openGroup(BUDI)
    st.next()
    assert st.viewerIndex == 1
    g = next(g for g in st.rig.mb.state["statuses"] if g["sender"] == BUDI)
    from mock_status import entry
    g["statuses"].append(entry("b4", BUDI, "text", "baru", 0))
    st.rig.mb.broadcast("status_new", {"sender": BUDI, "id": "b4"})
    assert spin(lambda: st.viewerCount == 4)
    assert st.viewerIndex == 1 and st.viewerEntry["id"] == "b2"
    new_sender = "6285555555555@s.whatsapp.net"
    st.rig.mb.state["statuses"].append({"sender": new_sender, "name": "Rina", "avatar": None, "allViewed": False,
                                        "latestTime": 0, "statuses": [entry("r1", new_sender, "text", "hai", 1)]})
    st.rig.mb.broadcast("status_new", {"sender": new_sender, "id": "r1"})
    assert spin(lambda: any(r["sender"] == new_sender for r in rows(st.model)))


def test_expiry_prunes_and_closes_viewer(st):
    from mock_status import entry, group
    peer = "6286666666666@s.whatsapp.net"
    st.rig.mb.state["statuses"].append(group(peer, "Cepat", [entry("x1", peer, "text", "bye", 1, expires_in_ms=1200)]))
    st.refresh()
    assert spin(lambda: any(r["sender"] == peer for r in rows(st.model)))
    st.openGroup(peer)
    assert st.viewerOpen
    time.sleep(1.4)
    st._prune()  # the 30 s timer does this in the app
    assert not any(r["sender"] == peer for r in rows(st.model))
    assert not st.viewerOpen  # nothing left to show


def test_expired_entries_are_dropped_on_load(st):
    from mock_status import entry, group
    peer = "6287777777777@s.whatsapp.net"
    st.rig.mb.state["statuses"].append(group(peer, "Lama", [entry("old", peer, "text", "x", 1)]))
    st.rig.mb.state["statuses"][-1]["statuses"][0]["expiresAt"] = int(time.time() * 1000) + 600
    st.refresh()
    assert spin(lambda: any(r["sender"] == peer for r in rows(st.model)))
    time.sleep(0.7)
    st._prune()
    assert not any(r["sender"] == peer for r in rows(st.model))


def test_status_error_state(rig):
    rig.mb.state["statuses_fail"] = True
    s = Statuses(rig.svc)
    assert spin(lambda: s.error != "")
    assert s.error == "boom" and s.loading is False and s.model.count == 0
    rig.mb.state["statuses_fail"] = False
    s.refresh()
    assert spin(lambda: s.error == "" and s.model.count == 4)


def test_status_media_src_uses_media_url_first(st):
    g = next(g for g in st.rig.mb.state["statuses"] if g["sender"] == BUDI)
    g["statuses"][1]["mediaUrl"] = "/media/status_cache_b2.jpg"
    st.refresh()
    assert spin(lambda: st.model.count == 4)
    st.openGroup(BUDI)
    st.next()
    assert spin(lambda: st.viewerEntry["src"].endswith("/media/status_cache_b2.jpg"))


# =============================================================== channels
def test_channels_list(ch):
    r = rows(ch.model)
    assert [c["jid"] for c in r] == [c["jid"] for c in mock_channels.channels()]
    assert r[0]["subscribersLabel"] == "1,250,431" and r[0]["verified"] is True and r[0]["initial"] == "T"
    assert r[1]["muted"] is True
    assert ch.error == "" and ch.selectedJid == ""


def test_select_channel_feed_sorted_with_reactions(ch):
    ch.select(mock_channels.TECH)
    assert ch.postsLoading is True
    assert spin(lambda: ch.posts.count == 5)
    p = rows(ch.posts)
    assert [x["serverId"] for x in p] == [1, 2, 3, 4, 5] and ch.hasMore is False
    assert p[1]["reactions"][0] == {"emoji": "👍", "count": 120}
    assert p[2]["mediaType"] == "image" and p[2]["content"] == ""
    assert p[4]["timeLabel"].startswith("Today at ") and p[0]["channelName"] == "Tech News Indonesia"
    assert ch.selected["name"] == "Tech News Indonesia"


def test_paging_older_posts(ch):
    ch.select(mock_channels.LONG)
    assert spin(lambda: ch.posts.count == 30)
    assert ch.hasMore is True
    assert rows(ch.posts)[0]["serverId"] == 46 and rows(ch.posts)[-1]["serverId"] == 75
    ch.loadMore()
    assert ch.loadingMore is True
    assert spin(lambda: ch.posts.count == 60)
    ids = [x["serverId"] for x in rows(ch.posts)]
    assert ids == sorted(ids) and ids[0] == 16 and ch.hasMore is True
    ch.loadMore()
    assert spin(lambda: ch.posts.count == 75)
    assert ch.hasMore is False and not ch.loadingMore
    ch.loadMore()  # nothing more: no request storm
    n = len(ch.rig.mb.requests)
    settle(100)
    assert len(ch.rig.mb.requests) == n + (0 if not ch.hasMore else 1)


def test_react_optimistic_and_failure_revert(ch):
    ch.select(mock_channels.TECH)
    assert spin(lambda: ch.posts.count == 5)
    ch.react("p5", "🙏")
    assert ch.posts.get_row("p5")["reactions"] == [{"emoji": "🙏", "count": 1}]  # before the server answers
    assert spin(lambda: ("react", mock_channels.TECH, 5, "p5", "🙏") in ch.rig.mb.state["channel_calls"])
    ch.react("p2", "👍")
    assert ch.posts.get_row("p2")["reactions"][0] == {"emoji": "👍", "count": 121}
    ch.rig.mb.state["react_fail"] = True
    ch.react("p1", "😂")
    assert spin(lambda: any(k == "error" for k, _ in ch.rig.toasts))
    assert {"emoji": "😂", "count": 1} not in ch.posts.get_row("p1")["reactions"]
    assert ch.rig.toasts[-1] == ("error", "Failed to react: react failed")


def test_mute_toggle_and_failure(ch):
    ch.select(mock_channels.TECH)
    ch.toggleMute()
    assert ch.selected["muted"] is True
    assert spin(lambda: ("mute", mock_channels.TECH, True) in ch.rig.mb.state["channel_calls"])
    ch.toggleMute()
    assert ch.selected["muted"] is False
    ch.rig.mb.state["mute_fail"] = True
    ch.toggleMute()
    assert spin(lambda: ch.selected["muted"] is False and any(k == "error" for k, _ in ch.rig.toasts))


def test_unfollow(ch):
    ch.select(mock_channels.NEWS)
    ch.unfollow()
    assert ch.model.count == 3 and ch.selectedJid == "" and ch.posts.count == 0
    assert ("success", "Unfollowed channel") in ch.rig.toasts
    assert spin(lambda: ("unfollow", mock_channels.NEWS) in ch.rig.mb.state["channel_calls"])
    settle()
    assert ch.model.count == 3


def test_unfollow_failure_restores_list(ch):
    ch.rig.mb.state["unfollow_fail"] = True
    ch.select(mock_channels.NEWS)
    ch.unfollow()
    assert ch.model.count == 3
    assert spin(lambda: ch.model.count == 4)  # refreshed from the backend
    assert ch.rig.toasts[-1] == ("error", "Failed to unfollow: cannot unfollow")


def test_preview_and_follow(ch):
    ch.previewChannel("not a link")
    assert spin(lambda: ch.rig.toasts and ch.rig.toasts[-1] == ("error", "Invalid channel link"))
    assert ch.preview == {} and ch.discovering is False
    ch.previewChannel("https://whatsapp.com/channel/AbCd123")
    assert ch.discovering is True
    assert spin(lambda: ch.preview.get("name") == "Preview AbCd123")
    assert ch.preview["subscribers"] == 4321 and ch.preview["subscribersLabel"] == "4,321"
    ch.clearPreview()
    assert ch.preview == {}
    done = []
    ch.followed.connect(lambda: done.append(1))
    ch.follow("https://whatsapp.com/channel/AbCd123")
    assert spin(lambda: done and ch.model.count == 5)
    assert ("success", "Channel followed") in ch.rig.toasts
    ch.follow("garbage")
    assert spin(lambda: ch.rig.toasts[-1][0] == "error" and "Failed to follow" in ch.rig.toasts[-1][1])


def test_ws_channel_events(ch):
    mb = ch.rig.mb
    ch.select(mock_channels.TECH)
    assert spin(lambda: ch.posts.count == 5)
    appended = []
    ch.postAppended.connect(lambda: appended.append(1))
    new = mock_channels.post(6, 0, "Breaking!", views=1)
    mb.broadcast("channel_message", {"channelId": mock_channels.TECH, "message": new})
    assert spin(lambda: ch.posts.count == 6 and appended)
    assert rows(ch.posts)[-1]["content"] == "Breaking!"
    mb.broadcast("channel_message", {"channelId": mock_channels.TECH, "message": new})  # duplicate: no second row
    settle()
    assert ch.posts.count == 6
    mb.broadcast("channel_message", {"channelId": mock_channels.NEWS, "message": mock_channels.post(9, 0, "elsewhere")})
    settle()
    assert ch.posts.count == 6
    mb.broadcast("channel_update", {"channelId": mock_channels.TECH,
                                    "messages": [{**new, "viewsCount": 777, "reactions": {"❤️": 9}}]})
    assert spin(lambda: ch.posts.get_row("p6")["viewsCount"] == 777)
    assert ch.posts.get_row("p6")["reactions"] == [{"emoji": "❤️", "count": 9}]
    mb.state["channels"] = [c for c in mb.state["channels"] if c["jid"] != mock_channels.TECH]
    mb.broadcast("channels_changed", {"reason": "unfollowed"})
    assert spin(lambda: ch.model.count == 3 and ch.selectedJid == "")  # selected channel vanished
    mb.state["channels"][0]["subscribers"] = 5
    mb.broadcast("channels_changed", {"reason": "x"})
    assert spin(lambda: rows(ch.model)[0]["subscribers"] == 5)


def test_empty_feed_and_errors(ch):
    ch.select(mock_channels.QUIET)
    assert spin(lambda: not ch.postsLoading)
    assert ch.posts.count == 0 and ch.postsError == ""
    ch.rig.mb.state["channel_posts_fail"] = True
    ch.select(mock_channels.TECH)
    assert spin(lambda: ch.postsError == "feed down")
    assert ch.postsLoading is False and ch.posts.count == 0


def test_channels_error_state(rig):
    rig.mb.state["channels_fail"] = True
    c = Channels(rig.svc)
    assert spin(lambda: c.error == "boom")
    assert c.loading is False and c.model.count == 0
    rig.mb.state["channels_fail"] = False
    c.refresh()
    assert spin(lambda: c.error == "" and c.model.count == 4)


def test_switching_channel_discards_stale_feed(ch):
    ch.select(mock_channels.LONG)
    ch.select(mock_channels.NEWS)  # answer of the first request must not leak into the second feed
    assert spin(lambda: ch.posts.count == 1)
    settle(300)
    assert ch.posts.count == 1 and rows(ch.posts)[0]["content"].startswith("Berita pagi")
