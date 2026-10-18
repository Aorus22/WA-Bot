"""Group entry points used by the QML sidebar and info sheet use existing REST contracts."""
from __future__ import annotations

import json
import sys
from pathlib import Path
from urllib.parse import unquote

from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI, TEAM  # noqa: E402
from bridge.api import Api  # noqa: E402
from bridge.appstate import AppState  # noqa: E402
from bridge.backend import BackendController  # noqa: E402
from bridge.chats import Chats  # noqa: E402
from bridge.events import EventHub  # noqa: E402
from bridge.registry import Services  # noqa: E402
from bridge.settings import Settings  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402

_RIGS = []


def spin(cond, ms=6000):
    loop = QEventLoop()
    timer = QTimer(loop)
    timer.setInterval(20)
    timer.timeout.connect(lambda: loop.quit() if cond() else None)
    timeout = QTimer(loop)
    timeout.setSingleShot(True)
    timeout.timeout.connect(loop.quit)
    timeout.start(ms)
    timer.start()
    loop.exec()
    timer.stop()
    timeout.stop()
    return cond()


def settle(ms=200):
    spin(lambda: False, ms)


def test_create_join_and_add_group_members(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    state, toast = AppState(hub), ToastBus()
    services = Services(backend=backend, api=api, hub=hub, settings=Settings(tmp_path / "settings.json"),
                        theme=None, app=state, qt_app=app, toast=toast)
    chats = Chats(api, hub, state, toast, backend, services.settings)
    _RIGS.append((chats, services, backend, hub, mock))
    backend.start()
    created, joined, members = [], [], []
    chats.createGroupFinished.connect(lambda ok, jid: created.append((ok, jid)))
    chats.joinGroupFinished.connect(joined.append)
    chats.addMembersFinished.connect(members.append)
    try:
        assert spin(lambda: backend.state == "ready" and chats.loaded)
        jumps = []
        chats.jumpRequested.connect(lambda chat_id, message_id: jumps.append((chat_id, message_id)))
        chats.search("6281111111111@s.whatsapp.net", "besok")
        assert spin(lambda: not chats.searching and bool(chats.searchResults))
        chats.openMessage("6281111111111@s.whatsapp.net", str(chats.searchResults[0].get("id") or ""))
        assert jumps and jumps[0][0] == "6281111111111@s.whatsapp.net"

        chats.createGroup("Planning", ["6282222222222@s.whatsapp.net", "6281111111111@s.whatsapp.net"])
        assert spin(lambda: created)
        assert created[0][0] is True and created[0][1].endswith("@g.us")
        assert mock.state["groups"][created[0][1]]["name"] == "Planning"
        chats.addMembers(created[0][1], ["6285555555555@s.whatsapp.net"])
        assert spin(lambda: members)
        assert members == [True]
        chats.joinGroup("https://chat.whatsapp.com/mock-invite")
        assert spin(lambda: joined)
        assert joined == [True]
        assert any(path == "/api/groups/create" for _method, path, _body in mock.requests)
        assert any(path == "/api/groups/join" for _method, path, _body in mock.requests)
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_group_settings_member_actions_and_invite_link_use_shared_contracts(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    state, toast = AppState(hub), ToastBus()
    chats = Chats(api, hub, state, toast, backend, Settings(tmp_path / "settings.json"))
    backend.start()
    group_id = TEAM
    member = BUDI
    links = []
    chats.groupInviteLinkReady.connect(lambda jid, link: links.append((jid, link)))
    try:
        assert spin(lambda: backend.state == "ready" and chats.loaded)
        state.openChat(group_id)
        assert spin(lambda: chats.group.get("jid") == group_id)

        chats.updateGroup(group_id, {"locked": True, "announce": True, "joinApproval": True})
        assert spin(lambda: all(mock.state["groups"][group_id][key] is True
                                for key in ("locked", "announce", "joinApproval")))
        assert chats.group["locked"] is True and chats.group["announce"] is True

        chats.updateGroupParticipants(group_id, "promote", [member])
        assert spin(lambda: next(p for p in mock.state["groups"][group_id]["participants"] if p["jid"] == member)["isAdmin"])
        chats.updateGroupParticipants(group_id, "demote", [member])
        assert spin(lambda: not next(p for p in mock.state["groups"][group_id]["participants"] if p["jid"] == member)["isAdmin"])
        chats.updateGroupParticipants(group_id, "remove", [member])
        assert spin(lambda: all(p["jid"] != member for p in mock.state["groups"][group_id]["participants"]))

        chats.getGroupInviteLink(group_id)
        assert spin(lambda: bool(links))
        first_link = links[-1][1]
        assert first_link == mock.state["group_invite_links"][group_id]
        chats.resetGroupInviteLink(group_id)
        assert spin(lambda: mock.state["group_invite_links"][group_id] != first_link)
        assert any(method == "PATCH" and unquote(path) == f"/api/groups/{group_id}" for method, path, _ in mock.requests)
        participant_actions = [json.loads(body)["action"] for method, path, body in mock.requests
                              if method == "POST" and unquote(path) == f"/api/groups/{group_id}/participants"]
        assert participant_actions == ["promote", "demote", "remove"]
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_group_details_and_photo_update_follow_go_contract(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    state, toast = AppState(hub), ToastBus()
    chats = Chats(api, hub, state, toast, backend, Settings(tmp_path / "settings.json"))
    photo = tmp_path / "group-photo.png"
    image_data = b"\x89PNG\r\n\x1a\nmock-group-photo"
    photo.write_bytes(image_data)
    backend.start()
    try:
        assert spin(lambda: backend.state == "ready" and chats.loaded)
        state.openChat(TEAM)
        assert spin(lambda: chats.group.get("jid") == TEAM)

        chats.updateGroup(TEAM, {"name": "Planning Crew", "description": "Weekly planning"})
        assert spin(lambda: mock.state["groups"][TEAM]["name"] == "Planning Crew")
        assert chats.group["description"] == "Weekly planning"
        assert chats.rows.get_row(TEAM)["name"] == "Planning Crew"

        chats.setGroupPhoto(TEAM, photo.as_uri())
        assert spin(lambda: len(mock.state["group_photo_uploads"]) == 1)
        upload = mock.state["group_photo_uploads"][0]
        assert upload["jid"] == TEAM
        assert upload["content_type"].startswith("multipart/form-data; boundary=")
        assert b'filename="group-photo.png"' in upload["body"]
        assert image_data in upload["body"]

        chats.removeGroupPhoto(TEAM)
        assert spin(lambda: len(mock.state["group_photo_uploads"]) == 2)
        assert mock.state["group_photo_uploads"][1]["body"] == b""
        photo_paths = [(method, unquote(path)) for method, path, _body in mock.requests
                       if path.startswith("/api/groups/") and path.endswith("/photo")]
        assert photo_paths == [("POST", f"/api/groups/{TEAM}/photo")] * 2
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()


def test_chat_selection_actions_keep_using_shared_rest_contracts(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["chats"]).start()
    backend = BackendController(["--url", mock.url])
    hub, api = EventHub(backend), Api(backend)
    state, toast = AppState(hub), ToastBus()
    services = Services(backend=backend, api=api, hub=hub, settings=Settings(tmp_path / "settings.json"),
                        theme=None, app=state, qt_app=app, toast=toast)
    chats = Chats(api, hub, state, toast, backend, services.settings)
    backend.start()
    try:
        assert spin(lambda: backend.state == "ready" and chats.loaded)

        chats.markRead("6281111111111@s.whatsapp.net")
        chats.setArchived("6281111111111@s.whatsapp.net", True)
        chats.setPinned("6282222222222@s.whatsapp.net", True)
        chats.setMute("120363000000000001@g.us", "forever")

        assert spin(lambda: mock.state["chats"])
        assert spin(lambda: mock.state["chats"][1]["unread"] == 0
                    and mock.state["chats"][1]["archived"] is True
                    and mock.state["chats"][2]["pinnedAt"] is not None
                    and mock.state["chats"][0]["muteMode"] == "forever")
        paths = {path.split("?", 1)[0] for _method, path, _body in mock.requests}
        assert any(path.endswith("/read") for path in paths)
        assert any(path.endswith("/archive") for path in paths)
        assert any(path.endswith("/pin") for path in paths)
        assert any(path.endswith("/mute") for path in paths)
    finally:
        settle()
        hub.stop()
        backend.shutdown()
        mock.stop()
