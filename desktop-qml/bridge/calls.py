"""Call signalling, history, and live media orchestration for the QML client."""
from __future__ import annotations

import json
from datetime import datetime
from typing import Any
from urllib.parse import quote

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot
from PySide6.QtGui import QImage

from .models import DictListModel

PAGE = 25
TERMINAL = {"ended", "rejected", "missed", "busy", "failed", "interrupted"}
LOG_ROLES = [
    "id", "meow_call_id", "direction", "call_type", "target", "group_jid", "participants", "source",
    "media_mode", "status", "error_message", "api_key_id", "started_at", "answered_at", "ended_at",
    "duration_ms", "created_at", "displayName", "initial", "missed", "typeLabel", "directionLabel",
    "statusLabel", "durationLabel", "timeLabel", "dateLabel", "startedFullLabel", "endedFullLabel",
]


def _dt(value: Any) -> datetime | None:
    try:
        n = int(value or 0)
        if n > 10_000_000_000:
            n //= 1000
        return datetime.fromtimestamp(n)
    except (TypeError, ValueError, OSError, OverflowError):
        return None


def _full_time_label(value: Any) -> str:
    date = _dt(value)
    if date is None:
        return "—"
    try:
        seconds = int(value or 0)
        if seconds > 10_000_000_000:
            seconds //= 1000
    except (TypeError, ValueError, OverflowError):
        return "—"
    return f"{date:%H:%M} (ts: {seconds})"


def _label(value: Any) -> str:
    return str(value or "").replace("_", " ").title()


def _terminal(status: Any) -> bool:
    return str(status or "").lower() in TERMINAL


def _is_video(call: dict | None) -> bool:
    return bool(call and (call.get("video_enabled") or str(call.get("type") or "").endswith("video")))


class Calls(QObject):
    historyChanged = Signal()
    callChanged = Signal()
    mediaChanged = Signal()
    filterChanged = Signal()
    _mediaStarted = Signal(str)

    def __init__(self, services, parent: QObject | None = None):
        super().__init__(parent)
        self._svc = services
        self._api = services.api
        self._hub = services.hub
        self._app = services.app
        self._toast = services.toast
        self._frames = services.call_frames
        self._chats = services.exposed.get("Chats")
        if self._chats is not None:
            self._chats.loadedChanged.connect(self._refresh_contact_names)
            if hasattr(self._chats, "namesChanged"):
                self._chats.namesChanged.connect(self._refresh_contact_names)
        self._history = DictListModel(LOG_ROLES, key="id", parent=self)
        self._logs: list[dict] = []
        self._filter = "all"
        self._query = ""
        self._loading = False
        self._loading_more = False
        self._has_more = False
        self._history_error = ""
        self._history_gen = 0
        self._selected_id = ""
        self._active: dict = {}
        self._active_gen = 0
        self._incoming: dict = {}
        self._video_upgrade: dict = {}
        self._media_handle = None
        self._media_session = None
        self._camera = None
        self._media_call_id = ""
        self._media_notice = ""
        self._media_connected = False
        self._mic_muted = False
        self._speaker_muted = False
        self._frame_rev = {"local": 0, "remote": 0}
        self._timer = QTimer(self)
        self._timer.setInterval(1000)
        self._timer.timeout.connect(self.callChanged)
        self._hub.event.connect(self._on_event)
        self._hub.connectedChanged.connect(self._on_connected)
        self._svc.backend.apiUrlChanged.connect(self._on_backend)
        self._mediaStarted.connect(self._start_media)
        if self._svc.backend.apiUrl:
            QTimer.singleShot(0, self.refresh)

    # -- QML properties ------------------------------------------------------------------
    @Property(QObject, constant=True)
    def history(self) -> QObject:
        return self._history

    @Property(bool, notify=historyChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(bool, notify=historyChanged)
    def loadingMore(self) -> bool:
        return self._loading_more

    @Property(bool, notify=historyChanged)
    def hasMore(self) -> bool:
        return self._has_more

    @Property(str, notify=historyChanged)
    def historyError(self) -> str:
        return self._history_error

    @Property(int, notify=historyChanged)
    def total(self) -> int:
        return len(self._logs)

    @Property(bool, notify=historyChanged)
    def canRetry(self) -> bool:
        return bool(self._history_error)

    @Property("QVariantMap", notify=historyChanged)
    def selectedLog(self) -> dict:
        return next((dict(row) for row in self._logs if row.get("id") == self._selected_id), {})

    @Property("QVariantMap", notify=callChanged)
    def activeCall(self) -> dict:
        return dict(self._active)

    @Property("QVariantMap", notify=callChanged)
    def incomingCall(self) -> dict:
        return dict(self._incoming)

    @Property("QVariantMap", notify=callChanged)
    def videoUpgradeRequested(self) -> dict:
        return dict(self._video_upgrade)

    @Property(bool, notify=callChanged)
    def hasActiveCall(self) -> bool:
        return bool(self._active) and not _terminal(self._active.get("status"))

    @Property(bool, notify=mediaChanged)
    def mediaConnected(self) -> bool:
        return self._media_connected

    @Property(str, notify=mediaChanged)
    def mediaNotice(self) -> str:
        return self._media_notice

    @Property(bool, notify=mediaChanged)
    def micMuted(self) -> bool:
        return self._mic_muted

    @Property(bool, notify=mediaChanged)
    def speakerMuted(self) -> bool:
        return self._speaker_muted

    @Property(str, notify=mediaChanged)
    def remoteFrameUrl(self) -> str:
        return f"image://callframes/remote?rev={self._frame_rev['remote']}" if self._frame_rev["remote"] else ""

    @Property(str, notify=mediaChanged)
    def localFrameUrl(self) -> str:
        return f"image://callframes/local?rev={self._frame_rev['local']}" if self._frame_rev["local"] else ""

    @Property(str, notify=callChanged)
    def elapsedText(self) -> str:
        started = self._active.get("answered_at") or self._active.get("started_at")
        start = _dt(started)
        if not start:
            return "00:00"
        secs = max(0, int((datetime.now() - start).total_seconds()))
        return f"{secs // 60:02d}:{secs % 60:02d}"

    @Property(str, notify=filterChanged)
    def filter(self) -> str:
        return self._filter

    @Property(str, notify=filterChanged)
    def query(self) -> str:
        return self._query

    # -- backend lifecycle ---------------------------------------------------------------
    def _on_backend(self) -> None:
        if self._svc.backend.apiUrl:
            self.refresh()
            self.refreshActive()

    def _on_connected(self) -> None:
        if self._hub.connected:
            self.refreshActive()

    @Slot()
    def refresh(self) -> None:
        self._history_gen += 1
        gen = self._history_gen
        self._loading = True
        self._history_error = ""
        self.historyChanged.emit()
        self._fetch_history(gen, reset=True)

    def _fetch_history(self, gen: int, reset: bool, before: int | None = None) -> None:
        query = f"?limit={PAGE}"
        if before is not None:
            query += f"&before={before}"
        def done(status: int, body: Any) -> None:
            if gen != self._history_gen:
                return
            self._loading = False
            self._loading_more = False
            if not 200 <= status < 300 or not isinstance(body, dict):
                self._history_error = str((body or {}).get("message") or (body or {}).get("error") or "Failed to load calls") if isinstance(body, dict) else "Failed to load calls"
                self._toast.error(self._history_error)
                self.historyChanged.emit()
                return
            rows = [self._row(log) for log in (body.get("logs") or []) if isinstance(log, dict)]
            self._has_more = len(rows) >= PAGE
            if reset:
                self._logs = rows
            else:
                seen = {r["id"] for r in self._logs}
                self._logs.extend(r for r in rows if r["id"] not in seen)
            self._rebuild_history()
        self._api.fetch("GET", "/calls/history" + query, None, done)

    def _row(self, log: dict) -> dict:
        group = str(log.get("group_jid") or "").strip()
        target = str(log.get("target") or "")
        # GPUI's call history uses the raw group JID or the target's local number,
        # even when the chat list has a resolved contact name.
        name = group or target.split("@", 1)[0] or "Unknown"
        date = _dt(log.get("started_at"))
        duration = int(log.get("duration_ms") or 0) // 1000
        call_type = str(log.get("call_type") or "")
        type_label = {
            "audio": "Audio", "video": "Video", "group_audio": "Group audio",
            "group_video": "Group video",
        }.get(call_type, "Unknown")
        return {
            **log, "id": str(log.get("id") or ""), "displayName": name,
            "initial": ("".join(c for c in name if c.isalnum())[:2] or "WA").upper(),
            "missed": str(log.get("status") or "") in {"missed", "rejected", "failed"},
            "typeLabel": type_label,
            "directionLabel": _label(log.get("direction")), "statusLabel": _label(log.get("status")),
            "durationLabel": f"{duration // 60}:{duration % 60:02d}" if duration > 0 else "—",
            "timeLabel": date.strftime("%H:%M") if date else "",
            "dateLabel": self._date_label(date),
            "startedFullLabel": _full_time_label(log.get("started_at")),
            "endedFullLabel": _full_time_label(log.get("ended_at")) if log.get("ended_at") else "",
        }

    @Slot()
    def _refresh_contact_names(self) -> None:
        if self._logs:
            self._logs = [self._row(row) for row in self._logs]
            self._rebuild_history()

    @staticmethod
    def _date_label(date: datetime | None) -> str:
        if date is None:
            return "Earlier"
        age = (datetime.now().date() - date.date()).days
        return "Today" if age <= 0 else "Yesterday" if age == 1 else "Earlier"

    def _rebuild_history(self) -> None:
        q = self._query.strip().lower()
        rows = []
        for log in self._logs:
            if self._filter == "incoming" and log.get("direction") != "incoming":
                continue
            if self._filter == "outgoing" and log.get("direction") != "outgoing":
                continue
            if self._filter == "missed" and not log.get("missed"):
                continue
            if q and q not in str(log.get("displayName") or "").lower() and q not in str(log.get("target") or "").lower() and q not in str(log.get("group_jid") or "").lower():
                continue
            rows.append(log)
        self._history.reset(rows)
        self.historyChanged.emit()

    @Slot(str)
    def selectLog(self, log_id: str) -> None:
        if self._selected_id != log_id:
            self._selected_id = log_id
            self.historyChanged.emit()

    @Slot()
    def clearSelection(self) -> None:
        self.selectLog("")

    @Slot(str)
    def callBack(self, log_id: str) -> None:
        log = next((row for row in self._logs if row.get("id") == log_id), None)
        if not log:
            return
        call_type = str(log.get("call_type") or "")
        video = "video" in call_type
        if call_type.startswith("group_") or log.get("group_jid"):
            self.startGroupCall(str(log.get("group_jid") or log.get("target") or ""),
                                list(log.get("participants") or []),
                                "group_video" if video else "group_audio")
        else:
            self.startCall(str(log.get("target") or ""), "video" if video else "audio")

    @Slot(str)
    def openChat(self, target: str) -> None:
        if target:
            self._app.openChat(target)

    @Slot()
    def loadMore(self) -> None:
        if self._loading or self._loading_more or not self._has_more or not self._logs:
            return
        self._loading_more = True
        self.historyChanged.emit()
        before = int(self._logs[-1].get("started_at") or 0)
        self._fetch_history(self._history_gen, reset=False, before=before)

    @Slot(str)
    def setFilter(self, value: str) -> None:
        if value in ("all", "incoming", "outgoing", "missed") and value != self._filter:
            self._filter = value
            self.filterChanged.emit()
            self._rebuild_history()

    @Slot(str)
    def setQuery(self, value: str) -> None:
        if value != self._query:
            self._query = value or ""
            self.filterChanged.emit()
            self._rebuild_history()

    # -- active call state ---------------------------------------------------------------
    @Slot()
    def refreshActive(self) -> None:
        gen = self._active_gen
        self._api.fetch("GET", "/calls/active", None,
                        lambda status, body: self._active_loaded(status, body, gen))

    def _active_loaded(self, status: int, body: Any, gen: int) -> None:
        # A slow startup refresh must not erase newer REST/WS call state.
        if gen != self._active_gen:
            return
        if status == 200 and isinstance(body, dict):
            self._set_active(body)
        elif status == 200 and body is None:
            self._set_active({})

    def _set_active(self, call: dict) -> None:
        self._active_gen += 1
        if not call:
            self._active = {}
            self._incoming = {}
            self._video_upgrade = {}
            self._stop_media()
            self._timer.stop()
            self.callChanged.emit()
            return
        if _terminal(call.get("status")):
            self._active = {}
            self._incoming = {}
            self._video_upgrade = {}
            self._stop_media()
            self._timer.stop()
            self.callChanged.emit()
            self.refresh()
            return
        self._active = dict(call)
        if call.get("direction") == "incoming" and call.get("status") == "ringing":
            self._incoming = dict(call)
        elif call.get("status") in ("connected", "connecting"):
            self._incoming = {}
        self._timer.start()
        self.callChanged.emit()
        self._sync_media()

    def _on_event(self, typ: str, payload: str) -> None:
        try:
            p = json.loads(payload) if payload else {}
        except ValueError:
            return
        if typ in ("call.incoming", "call.state", "call.peer_accepted", "call.ready"):
            if isinstance(p, dict) and p.get("id"):
                self._set_active(p)
                if typ == "call.peer_accepted":
                    self._video_upgrade = {}
                    self.callChanged.emit()
            return
        if typ == "call.ended":
            state = p.get("state") if isinstance(p, dict) else None
            cid = str((state or {}).get("id") or p.get("id") or "") if isinstance(p, dict) else ""
            if not cid or cid == str(self._active.get("id") or ""):
                self._set_active({})
            self.refresh()
            return
        if typ in ("call.video_state", "call.video_upgrade_requested") and isinstance(p, dict):
            if str(p.get("id") or "") == str(self._active.get("id") or ""):
                self._active = {**self._active, "video_enabled": bool(p.get("video_enabled")),
                                "remote_video_enabled": bool(p.get("remote_video_enabled"))}
                self._video_upgrade = dict(p) if typ == "call.video_upgrade_requested" else {}
                self.callChanged.emit()
                self._sync_media()
            return
        if typ in ("call.group_state", "call.participant_join") and isinstance(p, dict):
            state = p.get("state")
            if isinstance(state, dict):
                self._set_active(state)
            elif p.get("participants") and str(p.get("id") or "") == str(self._active.get("id") or ""):
                self._active = {**self._active, "participants": p["participants"]}
                self.callChanged.emit()

    def _post(self, path: str, body: dict | None = None, done=None) -> None:
        def finished(status: int, payload: Any) -> None:
            if not 200 <= status < 300:
                message = payload.get("error") or payload.get("message") if isinstance(payload, dict) else ""
                self._toast.error(str(message or "Call action failed"))
                if done:
                    done(status, payload)
                return
            if done:
                done(status, payload)
            QTimer.singleShot(150, self.refreshActive)
        self._api.fetch("POST", path, body, finished)

    @Slot(str, str)
    def startCall(self, target: str, call_type: str = "audio") -> None:
        target = (target or "").strip()
        if not target or self.hasActiveCall:
            return
        if call_type not in ("audio", "video"):
            call_type = "audio"
        self._api.fetch("POST", "/calls", {"target": target, "type": call_type},
                        lambda status, body: self._set_active(body) if 200 <= status < 300 and isinstance(body, dict)
                        else self._toast.error(str((body or {}).get("message") or "Failed to start call") if isinstance(body, dict) else "Failed to start call"))

    @Slot(str, "QVariantList", str)
    def startGroupCall(self, group_jid: str, participants: list, call_type: str = "group_audio") -> None:
        if self.hasActiveCall:
            return
        self._api.fetch("POST", "/calls/group", {"group_jid": group_jid, "participants": participants, "type": call_type},
                        lambda status, body: self._set_active(body) if 200 <= status < 300 and isinstance(body, dict)
                        else self._toast.error("Failed to start group call"))

    @Slot(str, "QVariantList")
    def addParticipants(self, call_id: str, targets: list) -> None:
        clean = list(dict.fromkeys(str(target).strip() for target in targets if str(target).strip()))
        if call_id and clean:
            self._post(f"/calls/{quote(call_id, safe='')}/participants", {"targets": clean})

    @Slot(str)
    def answer(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/answer")

    @Slot(str)
    def reject(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/reject")

    @Slot(str)
    def hangup(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/hangup")

    @Slot()
    def hangupActive(self) -> None:
        if self._active.get("id"):
            self.hangup(str(self._active["id"]))

    @Slot(str)
    def startVideo(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/video/start")

    @Slot(str)
    def acceptVideo(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/video/accept")

    @Slot(str)
    def rejectVideo(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/video/reject")

    @Slot(str)
    def stopVideo(self, call_id: str) -> None:
        self._post(f"/calls/{quote(call_id, safe='')}/video/stop")

    @Slot()
    def toggleVideo(self) -> None:
        if not self._active.get("id"):
            return
        self.stopVideo(str(self._active["id"])) if self._active.get("video_enabled") else self.startVideo(str(self._active["id"]))

    # -- media device/session ---------------------------------------------------------------
    def _sync_media(self) -> None:
        call = self._active
        live = bool(call and call.get("status") == "connected" and call.get("media_mode", "live") == "live")
        if not live:
            self._stop_media()
            return
        cid = str(call.get("id") or "")
        if not cid or cid == self._media_call_id:
            if call and call.get("video_enabled") and self._camera is None:
                self._start_camera()
            elif call and not call.get("video_enabled") and self._camera is not None:
                self._stop_camera()
            return
        self._media_call_id = cid
        self._mediaStarted.emit(cid)

    @Slot(str)
    def _start_media(self, call_id: str) -> None:
        if call_id != self._media_call_id:
            return
        self._stop_media_handle_only()
        from .call_media.session import MediaSession, SessionHandle
        session = MediaSession(call_id, lambda: self._svc.backend.apiUrl, devices=True)
        session.notice.connect(self._on_media_notice)
        session.networkConnected.connect(self._on_media_connected)
        session.remoteFrameReady.connect(self._on_remote_frame)
        self._media_session = session
        self._media_handle = SessionHandle(session, self)
        if self._active.get("video_enabled"):
            self._start_camera()

    def _stop_media_handle_only(self) -> None:
        handle = self._media_handle
        self._media_handle = None
        self._media_session = None
        if handle is not None:
            handle.finished.connect(handle.deleteLater)
            handle.close()

    def _stop_camera(self) -> None:
        camera, self._camera = self._camera, None
        if camera is not None:
            camera.stop()
            camera.deleteLater()
        self._frames.set_frame("local", None)
        self._frame_rev["local"] = 0
        self.mediaChanged.emit()

    def _stop_media(self) -> None:
        self._media_call_id = ""
        self._stop_camera()
        self._stop_media_handle_only()
        self._media_connected = False
        self._media_notice = ""
        self._frames.set_frame("remote", None)
        self._frame_rev["remote"] = 0
        self.mediaChanged.emit()

    def _start_camera(self) -> None:
        if self._camera is not None:
            return
        if not self._active.get("video_enabled") or self._media_session is None:
            return
        from .call_media.video import CameraCapture
        camera = self._camera = CameraCapture(self)
        camera.frame.connect(self._on_local_frame)
        camera.notice.connect(self._on_media_notice)
        camera.start()

    @Slot(QImage)
    def _on_local_frame(self, image: QImage) -> None:
        if self._media_session is not None:
            self._media_session.offer_video(image)
        self._frames.set_frame("local", image)
        self._frame_rev["local"] += 1
        self.mediaChanged.emit()

    @Slot()
    def _on_remote_frame(self) -> None:
        if self._media_session is None:
            return
        image = self._media_session.take_remote_frame()
        if image is not None:
            self._frames.set_frame("remote", image)
            self._frame_rev["remote"] += 1
            self.mediaChanged.emit()

    @Slot(str, str)
    def _on_media_notice(self, source: str, message: str) -> None:
        self._media_notice = f"{source}: {message}" if message else ""
        self.mediaChanged.emit()

    @Slot(bool)
    def _on_media_connected(self, connected: bool) -> None:
        self._media_connected = connected
        self.mediaChanged.emit()

    @Slot(bool)
    def setMicMuted(self, muted: bool) -> None:
        self._mic_muted = muted
        if self._media_session is not None:
            self._media_session.setMicMuted.emit(muted)
        self.mediaChanged.emit()

    @Slot()
    def toggleMic(self) -> None:
        self.setMicMuted(not self._mic_muted)

    @Slot(bool)
    def setSpeakerMuted(self, muted: bool) -> None:
        self._speaker_muted = muted
        if self._media_session is not None:
            self._media_session.setSpeakerMuted.emit(muted)
        self.mediaChanged.emit()

    @Slot()
    def toggleSpeaker(self) -> None:
        self.setSpeakerMuted(not self._speaker_muted)

    # -- history helpers -------------------------------------------------------------------
    @Slot("QVariantMap", result=str)
    def logTime(self, log: dict) -> str:
        d = _dt(log.get("started_at"))
        return d.strftime("%H:%M") if d else ""

    @Slot(str, result=str)
    def displayNameFor(self, target: str) -> str:
        target = str(target or "")
        contact = self._chats.get(target) if self._chats is not None else None
        name = str((contact or {}).get("name") or "").strip()
        if name:
            return name
        local = target.split("@", 1)[0]
        if len(local) > 7 and local.isdigit():
            return f"{local[:3]} {local[3:6]} {local[6:]}"
        return local or "Unknown"

    @Slot(str, result=str)
    def statusLabel(self, value: str) -> str:
        return _label(value)


def register(services) -> None:
    frames = getattr(services, "call_frames", None)
    if frames is None:
        return
    services.calls = Calls(services)
    services.expose("Calls", services.calls)
