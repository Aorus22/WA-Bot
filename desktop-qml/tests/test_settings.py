"""Settings preferences persist to an explicit test path, never the user's config."""
from __future__ import annotations

import sys
import json
from pathlib import Path
from types import SimpleNamespace

from PySide6.QtCore import QCoreApplication, QObject, Signal

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from bridge.settings import Settings  # noqa: E402
from bridge.system import System  # noqa: E402
from bridge.theme import ThemeManager  # noqa: E402
from bridge.toast import ToastBus  # noqa: E402

_INERT_SETTINGS_OBJECTS = []


class Hub(QObject):
    event = Signal(str, str)
    connectedChanged = Signal()
    connected = False


class Backend(QObject):
    restartFinished = Signal(str, str)

    def __init__(self):
        super().__init__()
        self.restart_count = 0

    def restart(self):
        self.restart_count += 1


class FakeApi:
    """Queued REST responses exercise the Settings callbacks without a network worker."""
    def __init__(self):
        self.requests: list[dict] = []
        self.pending: list[dict] = []
        self.fail_next_settings_update = False
        self.server_settings = {
            "ai_server_url": "http://localhost:8981", "read_receipts": "true",
            "contact_name_mode": "profile", "call_tts_provider": "",
            "call_tts_default_voice": "", "call_tts_fish_audio_model": "",
            "call_tts_fish_audio_voice_id": "", "gemini_api_key": "", "call_tts_fish_audio_key": "",
        }

    def fetch(self, method, path, body=None, cb=None):
        request = {"method": method.upper(), "path": path, "body": body, "cb": cb}
        self.requests.append(request)
        if cb is not None:
            self.pending.append(request)

    def settings_response(self):
        values = dict(self.server_settings)
        values["gemini_api_key"] = "••••••••" if values["gemini_api_key"] else ""
        values["call_tts_fish_audio_key"] = "••••••••" if values["call_tts_fish_audio_key"] else ""
        return {"settings": values, "hasGeminiKey": bool(self.server_settings["gemini_api_key"]),
                "hasFishKey": bool(self.server_settings["call_tts_fish_audio_key"])}

    def reply(self, method, path, status=None, body=None):
        method = method.upper()
        index = next(i for i, req in enumerate(self.pending)
                     if req["method"] == method and req["path"] == path)
        request = self.pending.pop(index)
        if status is None and (method, path) == ("GET", "/settings"):
            status, body = 200, self.settings_response()
        elif status is None and (method, path) == ("PUT", "/settings"):
            if self.fail_next_settings_update:
                self.fail_next_settings_update = False
                status, body = 500, {"error": "settings write failed"}
            else:
                self.server_settings.update({key: str(value) for key, value in request["body"].items()})
                status, body = 200, self.settings_response()
        assert status is not None
        request["cb"](status, body)


class SettingsRig:
    def __init__(self, tmp_path):
        self.app = QCoreApplication.instance() or QCoreApplication([])
        self.backend = Backend()
        self.hub = Hub()
        self.api = FakeApi()
        self.settings = Settings(tmp_path / "prefs.json")
        self.theme = ThemeManager(self.settings)
        self.toast = ToastBus()
        self.auth_updates = []
        self.routes = []
        self.app_state = SimpleNamespace(
            set_auth=lambda *args: self.auth_updates.append(args),
            navigate=lambda *args: self.routes.append(args),
        )
        services = SimpleNamespace(
            settings=self.settings, theme=self.theme, toast=self.toast, api=self.api,
            backend=self.backend, hub=self.hub, app=self.app_state, engine=None, qt_app=self.app,
        )
        self.system = System(services, gtk_config=tmp_path / "gtk.ini")
        self.system.refresh()
        self.api.reply("GET", "/settings")
        self.api.reply("GET", "/history-sync/status", 200, {"state": "idle", "current": [], "options": {}})
        self.api.reply("GET", "/history-media/status", 200, {"state": "idle", "current": 0, "options": {}})
        self.api.reply("GET", "/storage/usage", 200, {"chatBytes": 1234, "mediaBytes": 5678})

    def close(self):
        for timer in (self.system._gtk_timer, self.system._sync_timer, self.system._media_timer):
            timer.stop()
        # Keep stopped QObject wrappers alive until pytest exits. Flushing global
        # DeferredDelete events can invalidate objects owned by other fixtures.
        _INERT_SETTINGS_OBJECTS.append(self)


def test_chat_layout_and_format_preferences_persist_in_test_file(tmp_path):
    app = QCoreApplication.instance() or QCoreApplication([])
    settings_path = tmp_path / "settings.json"
    gtk_config = tmp_path / "gtk.ini"
    settings = Settings(settings_path)
    theme = ThemeManager(settings)
    services = SimpleNamespace(settings=settings, theme=theme, toast=ToastBus(), api=None,
                               backend=Backend(), hub=Hub(), app=SimpleNamespace(), engine=None,
                               qt_app=app)
    system = System(services, gtk_config=gtk_config)
    try:
        assert system.chatLayout == "auto"
        system.setChatLayout("pages")
        system.setFormatMessages(False)
        system.setNotificationsEnabled(False)
        saved = Settings(settings_path)
        assert system.chatLayout == "pages"
        assert saved.get_py("chat_layout") == "pages"
        assert saved.get_py("format_messages") is False
        assert saved.get_py("notifications_enabled") is False
        assert settings_path.exists() and gtk_config.parent == tmp_path
    finally:
        system._gtk_timer.stop()
        system._sync_timer.stop()
        system._media_timer.stop()
        _INERT_SETTINGS_OBJECTS.append((system, settings, theme, services))


def test_server_settings_save_masking_and_optimistic_transitions(tmp_path):
    r = SettingsRig(tmp_path)
    try:
        system = r.system
        system.commitEdit("gemini_key", "  gemini-secret  ")
        system.commitEdit("ai_url", " https://ai.example/v1 ")
        system.saveAi()
        assert r.api.requests[-1]["method"] == "PUT" and r.api.requests[-1]["path"] == "/settings"
        assert r.api.requests[-1]["body"] == {"ai_server_url": "https://ai.example/v1", "gemini_api_key": "gemini-secret"}
        r.api.reply("PUT", "/settings")
        assert r.api.server_settings["gemini_api_key"] == "gemini-secret"
        assert r.api.server_settings["ai_server_url"] == "https://ai.example/v1"
        assert system.hasGeminiKey and system.editSpec("gemini_key")["value"] == ""

        system.setTtsProvider("edge")
        system.commitEdit("tts_voice", "id-ID-GadisNeural")
        system.saveTts()
        r.api.reply("PUT", "/settings")
        assert r.api.server_settings["call_tts_provider"] == "edge"
        assert r.api.server_settings["call_tts_default_voice"] == "id-ID-GadisNeural"

        system.toggleReadReceipts()
        r.api.reply("PUT", "/settings")
        assert not system.readReceipts
        assert r.api.server_settings["read_receipts"] == "false"

        system.setContactNameMode("saved")
        r.api.reply("PUT", "/settings")
        assert system.contactNameMode == "saved"
        assert r.api.server_settings["contact_name_mode"] == "saved"

        r.api.fail_next_settings_update = True
        system.setContactNameMode("profile")
        r.api.reply("PUT", "/settings")
        assert system.contactNameMode == "saved"  # failed write rolls back
    finally:
        r.close()


def test_history_sync_and_media_jobs_follow_rest_status_contracts(tmp_path):
    r = SettingsRig(tmp_path)
    imported = []
    r.system.historyImported.connect(lambda: imported.append(True))
    try:
        system = r.system
        system.setHistoryDepth("days30")
        system.toggleRetryFinished()
        system.startHistorySync(False)
        request = r.api.requests[-1]
        assert request["method"] == "POST" and request["path"] == "/history-sync"
        assert request["body"]["sinceDays"] == 30 and request["body"]["retryFinished"] is True
        running = {"state": "running", "chatsTotal": 3, "chatsProcessed": 0, "current": [], "options": {}}
        r.api.reply("POST", "/history-sync", 202, running)
        assert system._sync_status["state"] == "running"
        system.cancelHistorySync()
        cancelling = {**running, "state": "cancelling"}
        r.api.reply("POST", "/history-sync/cancel", 202, cancelling)
        assert system._sync_status["state"] == "cancelling"

        completed = {**cancelling, "state": "completed", "messagesAdded": 17,
                     "chatsProcessed": 3, "resumable": False}
        system._on_event("history_sync", json.dumps(completed))
        assert not system._sync_active and system._sync_status["state"] == "completed"
        assert imported

        system.toggleAskPhone()
        system.toggleRetryUnavailable()
        system.startHistoryMedia(False)
        request = r.api.requests[-1]
        assert request["method"] == "POST" and request["path"] == "/history-media"
        assert request["body"]["askPhone"] and request["body"]["retryUnavailable"]
        assert request["body"]["types"] == system._media_kinds
        media_running = {"state": "running", "total": 4, "done": 0, "remaining": 4, "options": request["body"]}
        r.api.reply("POST", "/history-media", 202, media_running)
        assert system._media_status["state"] == "running"
        system.cancelHistoryMedia()
        r.api.reply("POST", "/history-media/cancel", 202, {**media_running, "state": "cancelling"})
        assert system._media_status["state"] == "cancelling"
    finally:
        r.close()


def test_logout_and_backend_restart_follow_api_callbacks(tmp_path):
    r = SettingsRig(tmp_path)
    try:
        r.system.logout()
        request = r.api.requests[-1]
        assert (request["method"], request["path"], request["body"]) == ("POST", "/logout", None)
        r.api.reply("POST", "/logout", 200, {"status": "ok"})
        assert r.auth_updates == [("unauthenticated",)] and r.routes == [("chat",)]

        r.system.restartBackend()
        assert r.backend.restart_count == 1 and r.system.restartingBackend
        r.backend.restartFinished.emit("http://127.0.0.1:31415/api", "")
        assert r.api.requests[-1]["method"] == "GET" and r.api.requests[-1]["path"] == "/status"
        r.api.reply("GET", "/status", 200, {"isLoggedIn": True, "phone": "628123456789"})
        assert not r.system.restartingBackend
        assert r.auth_updates[-1] == ("authenticated", "628123456789")
    finally:
        r.close()
