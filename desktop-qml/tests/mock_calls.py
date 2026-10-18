"""Mock call history and signalling endpoints; these fixtures never contact WhatsApp."""
from __future__ import annotations

import time
from copy import deepcopy
from urllib.parse import quote


def _log(id_: str, target: str, direction: str, status: str, call_type: str, ago: int, duration: int = 0):
    now = int(time.time())
    return {
        "id": id_, "meow_call_id": f"meow-{id_}", "direction": direction,
        "call_type": call_type, "target": target, "group_jid": None,
        "participants": None, "source": "ui", "media_mode": "live", "status": status,
        "error_message": None, "api_key_id": None, "started_at": now - ago,
        "answered_at": now - ago + 2 if duration else None,
        "ended_at": now - ago + duration if duration else None,
        "duration_ms": duration * 1000 if duration else None, "created_at": now - ago,
    }


def install(mb) -> None:
    mb.state.setdefault("calls_history", [
        _log("c1", "6281111111111@s.whatsapp.net", "incoming", "missed", "audio", 600),
        _log("c2", "6282222222222@s.whatsapp.net", "outgoing", "ended", "video", 3600, 145),
        _log("c3", "6281111111111@s.whatsapp.net", "outgoing", "rejected", "audio", 86400),
    ])
    mb.state.setdefault("active_call", None)
    mb.state.setdefault("call_actions", [])
    mb.state.setdefault("media_sessions", {})
    mb.state.setdefault("media_session_by_call", {})
    mb.state.setdefault("media_session_count", 0)

    @mb.route("GET", "/calls/history")
    def history(req):
        rows = deepcopy(mb.state["calls_history"])
        direction = req.q("direction")
        status = req.q("status")
        target = req.q("target")
        before = int(req.q("before", "0") or 0)
        if direction:
            rows = [r for r in rows if r["direction"] == direction]
        if status:
            rows = [r for r in rows if r["status"] == status]
        if target:
            rows = [r for r in rows if target.lower() in r["target"].lower()]
        if before:
            rows = [r for r in rows if int(r["started_at"]) < before]
        rows.sort(key=lambda r: int(r["started_at"]), reverse=True)
        limit = max(1, min(int(req.q("limit", "25") or 25), 100))
        return 200, {"logs": rows[:limit]}

    @mb.route("GET", "/calls/active")
    def active(_req):
        return 200, deepcopy(mb.state.get("active_call"))

    @mb.route("POST", r"/calls/(?P<id>[^/]+)/media-session")
    def media_session(req):
        call = mb.state.get("active_call")
        if not call or call.get("id") != req.params["id"]:
            return 409, {"error": "call_not_active", "message": "call is not active"}
        existing = mb.state["media_session_by_call"].get(req.params["id"])
        if existing and existing in mb.state["media_sessions"]:
            token = existing
        else:
            mb.state["media_session_count"] += 1
            token = f"mock-media-{mb.state['media_session_count']}"
            mb.state["media_sessions"][token] = req.params["id"]
            mb.state["media_session_by_call"][req.params["id"]] = token
        return 200, {
            "token": token,
            "call_id": req.params["id"],
            "expires_at": "2099-01-01T00:00:00Z",
            "ws_path": f"/ws/calls/{quote(req.params['id'], safe='')}/media",
        }

    @mb.route("POST", "/calls")
    def create(req):
        body = req.json() or {}
        call = {
            "id": "mock-call-1", "status": "ringing", "type": body.get("type", "audio"),
            "direction": "outgoing", "source": "ui", "media_mode": "tts",
            "target": body.get("target", ""), "group_jid": None, "participants": None,
            "started_at": int(time.time()), "answered_at": None,
            "video_enabled": body.get("type") == "video", "remote_video_enabled": False,
        }
        mb.state["active_call"] = call
        mb.state["call_actions"].append(("create", body))
        return 200, deepcopy(call)

    @mb.route("POST", "/calls/group")
    def create_group(req):
        body = req.json() or {}
        call = {
            "id": "mock-group-call-1", "status": "ringing", "type": body.get("type", "group_audio"),
            "direction": "outgoing", "source": "ui", "media_mode": "tts",
            "target": body.get("group_jid", ""), "group_jid": body.get("group_jid"),
            "participants": body.get("participants", []), "started_at": int(time.time()),
            "answered_at": None, "video_enabled": body.get("type") == "group_video",
            "remote_video_enabled": False,
        }
        mb.state["active_call"] = call
        mb.state["call_actions"].append(("create_group", body))
        return 200, deepcopy(call)

    @mb.route("POST", r"/calls/(?P<id>[^/]+)/(?P<action>answer|reject|hangup)")
    def action(req):
        action_name, call = req.params["action"], mb.state.get("active_call")
        mb.state["call_actions"].append((action_name, req.params["id"]))
        if call and call.get("id") == req.params["id"]:
            if action_name == "answer":
                call.update(status="connected", answered_at=int(time.time()))
                mb.broadcast("call.state", deepcopy(call))
            else:
                call.update(status="rejected" if action_name == "reject" else "ended")
                mb.state["calls_history"].insert(0, {
                    **call, "call_type": call.get("type", "audio"), "ended_at": int(time.time()),
                    "duration_ms": None, "created_at": call.get("started_at", int(time.time())),
                })
                mb.broadcast("call.ended", {"id": call["id"], "state": deepcopy(call)})
                mb.state["active_call"] = None
        return 200, {"status": "ok"}

    @mb.route("POST", r"/calls/(?P<id>[^/]+)/participants")
    def add_participants(req):
        body = req.json() or {}
        mb.state["call_actions"].append(("participants", req.params["id"], body.get("targets", [])))
        call = mb.state.get("active_call")
        if call and call.get("id") == req.params["id"]:
            current = list(call.get("participants") or [])
            call["participants"] = list(dict.fromkeys([*current, *(body.get("targets") or [])]))
            mb.broadcast("call.group_state", {"id": call["id"], "participants": call["participants"]})
        return 200, {"status": "participants_added"}

    @mb.route("POST", r"/calls/(?P<id>[^/]+)/video/(?P<video_action>start|accept|reject|stop)")
    def video_action(req):
        action_name, call = req.params["video_action"], mb.state.get("active_call")
        mb.state["call_actions"].append((f"video_{action_name}", req.params["id"]))
        if call and call.get("id") == req.params["id"]:
            call["video_enabled"] = action_name in ("start", "accept")
            call["remote_video_enabled"] = action_name == "accept"
            mb.broadcast("call.video_state", deepcopy(call))
        return 200, {"status": "ok"}
