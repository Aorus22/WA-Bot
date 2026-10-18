"""`System` (QML context property): backing for the Settings page and desktop integration of
preferences. Port of desktop-gpui `views/settings.rs`, `state/settings.rs`, `theme/manager.rs`
(`sync_gtk`), `window_material.rs` and the settings-related parts of `layout_pref.rs`.

* server-owned settings    GET/PUT /api/settings   (AI keys, TTS, read receipts, contact names)
* history sync / media     /api/history-sync[/status|/cancel], /api/history-media[...], /api/storage/usage,
                           polled while running (1.5 s / 1 s like GPUI) and fed by the WS `history_sync` event
* UI preferences           `Settings` keys (theme preset/mode, transparency, blur, font scales,
                           notifications, message formatting, chat layout); sliders preview live through
                           `Theme.setUiScale(v, false)` and persist on release
* desktop (GTK) palette    registered as preset `gtk`, re-read when the GTK config changes (2 s poll)
* backend restart          `BackendController.restart()` (owned: handle; adopted: stop_listener by port)
* window effects           translucent window (Main.qml) + best-effort X11 blur hint; see `apply_window_effects`
"""
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, QProcess, QTimer, Signal, Slot, qVersion

from . import gtk_theme, history_text as ht
from .theme import GTK_PRESET_NAME, is_dark

ROOT = Path(__file__).resolve().parent.parent

SYNC_POLL_MS = 1500
MEDIA_POLL_MS = 1000
GTK_POLL_MS = 2000
RESTART_WAIT_STEPS, RESTART_WAIT_MS = 30, 500

# GPUI `open_edit_modal`: (title, description, is_secret, presets)
EDIT_FIELDS: dict[str, dict[str, Any]] = {
    "gemini_key": {"title": "Gemini API Key", "secret": True, "presets": [],
                   "description": "Enter your Google Gemini API key. Stored securely in backend database."},
    "ai_url": {"title": "AI Server URL", "secret": False,
               "description": "HTTP endpoint for external AI services.",
               "presets": [["Default Local", "http://localhost:8981"]]},
    "tts_voice": {"title": "Default Voice", "secret": False,
                  "description": "Voice name used for incoming call speech synthesis.",
                  "presets": [["US Jenny (Edge)", "en-US-JennyNeural"], ["US Guy (Edge)", "en-US-GuyNeural"],
                              ["ID Gadis (Edge)", "id-ID-GadisNeural"], ["ID Ardi (Edge)", "id-ID-ArdiNeural"]]},
    "fish_key": {"title": "Fish Audio API Key", "secret": True, "presets": [],
                 "description": "API key for Fish Audio TTS provider. Leave blank to keep existing key."},
    "fish_model": {"title": "Fish Audio Model", "secret": False,
                   "description": "Model identifier on Fish Audio platform.",
                   "presets": [["Speech 1.5", "fishaudio/fish-speech-1.5"]]},
    "fish_voice_id": {"title": "Fish Audio Voice ID", "secret": False, "presets": [],
                      "description": "Custom reference Voice ID from your Fish Audio library."},
}


def _reason(status: int, body: Any) -> str:
    """What the backend said (its `error` field), like GPUI's `ClientError::Api { message }`."""
    if isinstance(body, dict) and body.get("error"):
        return str(body["error"])
    if isinstance(body, str):
        return body.strip()
    return ""


def app_version() -> str:
    try:
        m = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "pyproject.toml").read_text(), re.M)
        return m.group(1) if m else "dev"
    except OSError:
        return "dev"


class System(QObject):
    serverChanged = Signal()
    historyChanged = Signal()
    mediaChanged = Signal()
    storageChanged = Signal()
    prefsChanged = Signal()
    presetsChanged = Signal()
    restartingChanged = Signal()
    historyImported = Signal()  # a sync/download ended: chat list and names may have changed

    def __init__(self, services, gtk_config: Path | None = None, parent: QObject | None = None):
        super().__init__(parent)
        self._s = services
        self._api = services.api
        self._settings = services.settings
        self._theme = services.theme
        self._toast = services.toast
        self._gtk_config = gtk_config
        self._visited = False

        # server settings
        self._loaded = False
        self._loading = False
        self._has_gemini = False
        self._has_fish = False
        self._gemini_input = ""
        self._ai_url = "http://localhost:8981"
        self._read_receipts = True
        self._contact_mode = "profile"
        self._tts_provider = ""
        self._tts_voice = "en-US-JennyNeural"
        self._fish_input = ""
        self._fish_model = "fishaudio/fish-speech-1.5"
        self._fish_voice_id = ""
        self._saving_ai = self._saving_tts = self._saving_receipts = False

        # history sync
        self._sync_status: dict | None = None
        self._sync_active = False
        self._depth = ht.DEFAULT_DEPTH
        self._retry_finished = False
        self._sync_timer = self._timer(SYNC_POLL_MS, self._poll_sync)
        # history media
        self._media_status: dict | None = None
        self._media_active = False
        self._media_kinds = ht.default_kinds()
        self._media_retry = False
        self._media_ask_phone = False
        self._media_polls = 0
        self._media_timer = self._timer(MEDIA_POLL_MS, self._poll_media)
        self._usage: dict | None = None

        self._restarting = False
        self._presets_rev = 0

        # live preferences: anything that changes a pref (here or elsewhere) refreshes bindings
        self._settings.changed.connect(lambda _k: self.prefsChanged.emit())
        self._theme.changed.connect(self.prefsChanged)
        self._s.backend.restartFinished.connect(self._on_backend_restarted)
        self._s.hub.event.connect(self._on_event)
        self._s.hub.connectedChanged.connect(self._on_connected)

        # desktop (GTK) palette
        self._gtk_key = gtk_theme.current_key(gtk_config)
        self._sync_gtk_preset(first=True)
        self._gtk_timer = self._timer(GTK_POLL_MS, self._poll_gtk, single=False)
        self._gtk_timer.start()

        # window effects follow the preferences (and the window, once QML creates it)
        self._window = None
        self._settings.changed.connect(self._on_pref_for_window)
        engine = getattr(services, "engine", None)
        if engine is not None:
            engine.objectCreated.connect(self._on_object_created)

    def _timer(self, ms: int, fn, single: bool = True) -> QTimer:
        t = QTimer(self)
        t.setInterval(ms)
        t.setSingleShot(single)
        t.timeout.connect(fn)
        return t

    # ===================================================================== server settings
    haveLoaded = Property(bool, lambda s: s._loaded, notify=serverChanged)
    hasGeminiKey = Property(bool, lambda s: s._has_gemini, notify=serverChanged)
    hasFishKey = Property(bool, lambda s: s._has_fish, notify=serverChanged)
    aiServerUrl = Property(str, lambda s: s._ai_url, notify=serverChanged)
    readReceipts = Property(bool, lambda s: s._read_receipts, notify=serverChanged)
    contactNameMode = Property(str, lambda s: s._contact_mode, notify=serverChanged)
    ttsProvider = Property(str, lambda s: s._tts_provider, notify=serverChanged)
    ttsDefaultVoice = Property(str, lambda s: s._tts_voice, notify=serverChanged)
    fishAudioModel = Property(str, lambda s: s._fish_model, notify=serverChanged)
    fishAudioVoiceId = Property(str, lambda s: s._fish_voice_id, notify=serverChanged)
    savingAi = Property(bool, lambda s: s._saving_ai, notify=serverChanged)
    savingTts = Property(bool, lambda s: s._saving_tts, notify=serverChanged)
    savingReceipts = Property(bool, lambda s: s._saving_receipts, notify=serverChanged)

    @Slot()
    def refresh(self) -> None:
        """Load everything the page shows (called when the Settings page is opened)."""
        self._visited = True
        self.loadSettings()
        self.loadHistoryStatus()
        self.loadMediaStatus()
        self.loadStorageUsage()

    @Slot()
    def loadSettings(self) -> None:
        if self._loading:
            return
        self._loading = True

        def done(status: int, body: Any) -> None:
            self._loading = False
            if status == 200 and isinstance(body, dict):
                self._apply_settings(body)

        self._api.fetch("GET", "/settings", None, done)

    def _apply_settings(self, resp: dict) -> None:
        s = resp.get("settings") or {}
        self._has_gemini = bool(resp.get("hasGeminiKey"))
        self._has_fish = bool(resp.get("hasFishKey"))
        self._ai_url = s.get("ai_server_url", "http://localhost:8981")
        self._read_receipts = s.get("read_receipts", "true") != "false"
        self._contact_mode = "saved" if s.get("contact_name_mode") == "saved" else "profile"
        self._tts_provider = s.get("call_tts_provider", "")
        self._tts_voice = s.get("call_tts_default_voice", "")
        self._fish_model = s.get("call_tts_fish_audio_model", "")
        self._fish_voice_id = s.get("call_tts_fish_audio_voice_id", "")
        self._loaded = True
        self.serverChanged.emit()

    @Slot(str, result="QVariantMap")
    def editSpec(self, field: str) -> dict:
        """Title/description/initial value/secret/presets of the edit modal for `field`."""
        spec = dict(EDIT_FIELDS[field])
        spec["field"] = field
        spec["value"] = {
            "gemini_key": self._gemini_input, "ai_url": self._ai_url, "tts_voice": self._tts_voice,
            "fish_key": self._fish_input, "fish_model": self._fish_model, "fish_voice_id": self._fish_voice_id,
        }[field]
        return spec

    @Slot(str, str)
    def commitEdit(self, field: str, value: str) -> None:
        """Modal 'Done': store the value (unsaved until the section's Save button)."""
        attr = {"gemini_key": "_gemini_input", "ai_url": "_ai_url", "tts_voice": "_tts_voice",
                "fish_key": "_fish_input", "fish_model": "_fish_model", "fish_voice_id": "_fish_voice_id"}[field]
        setattr(self, attr, value)
        self.serverChanged.emit()

    @Slot(str)
    def setTtsProvider(self, provider: str) -> None:
        if provider in ("", "edge", "fish"):
            self._tts_provider = provider
            self.serverChanged.emit()

    def ai_payload(self) -> dict[str, str]:
        data = {"ai_server_url": self._ai_url.strip()}
        if self._gemini_input.strip():
            data["gemini_api_key"] = self._gemini_input.strip()
        return data

    def tts_payload(self) -> dict[str, str]:
        data = {
            "call_tts_provider": self._tts_provider,
            "call_tts_default_voice": self._tts_voice.strip(),
            "call_tts_fish_audio_model": self._fish_model.strip(),
            "call_tts_fish_audio_voice_id": self._fish_voice_id.strip(),
        }
        if self._fish_input.strip():
            data["call_tts_fish_audio_key"] = self._fish_input.strip()
        return data

    @Slot()
    def saveAi(self) -> None:
        if self._saving_ai:
            return
        self._saving_ai = True
        self.serverChanged.emit()

        def done(status: int, body: Any) -> None:
            self._saving_ai = False
            if status == 200 and isinstance(body, dict):
                self._has_gemini = bool(body.get("hasGeminiKey", self._has_gemini))
                self._gemini_input = ""
                self._toast.success("AI configuration saved")
            else:
                self._toast.error("Failed to save AI configuration")
            self.serverChanged.emit()

        self._api.fetch("PUT", "/settings", self.ai_payload(), done)

    @Slot()
    def saveTts(self) -> None:
        if self._saving_tts:
            return
        self._saving_tts = True
        self.serverChanged.emit()

        def done(status: int, body: Any) -> None:
            self._saving_tts = False
            if status == 200 and isinstance(body, dict):
                self._has_fish = bool(body.get("hasFishKey", self._has_fish))
                self._fish_input = ""
                self._toast.success("Call TTS settings saved")
            else:
                self._toast.error("Failed to save TTS settings")
            self.serverChanged.emit()

        self._api.fetch("PUT", "/settings", self.tts_payload(), done)

    @Slot()
    def toggleReadReceipts(self) -> None:
        if self._saving_receipts:
            return
        self._saving_receipts = True
        new = not self._read_receipts
        self._read_receipts = new  # optimistic, like GPUI
        self.serverChanged.emit()

        def done(status: int, body: Any) -> None:
            self._saving_receipts = False
            if status == 200 and isinstance(body, dict):
                self._read_receipts = (body.get("settings") or {}).get("read_receipts", "true") != "false"
                self._toast.success("Read receipts enabled" if self._read_receipts else "Read receipts disabled")
            else:
                self._toast.error("Failed to update read receipts")  # GPUI keeps the optimistic value here
            self.serverChanged.emit()

        self._api.fetch("PUT", "/settings", {"read_receipts": "true" if new else "false"}, done)

    @Slot(str)
    def setContactNameMode(self, mode: str) -> None:
        if mode not in ("saved", "profile") or self._contact_mode == mode:
            return
        previous, self._contact_mode = self._contact_mode, mode
        self.serverChanged.emit()

        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._contact_mode = "saved" if (body.get("settings") or {}).get("contact_name_mode") == "saved" else "profile"
            else:
                self._contact_mode = previous
                self._toast.error("Failed to update contact names")
            self.serverChanged.emit()

        self._api.fetch("PUT", "/settings", {"contact_name_mode": mode}, done)

    # ===================================================================== account
    @Slot()
    def logout(self) -> None:
        """Confirmed logout: POST /logout, then (whatever it said, like GPUI) back to the login."""
        def done(status: int, body: Any) -> None:
            self._s.app.set_auth("unauthenticated")
            self._s.app.navigate("chat")

        self._api.fetch("POST", "/logout", None, done)

    @Slot()
    def login(self) -> None:
        self._s.app.set_auth("unauthenticated")

    restartingBackend = Property(bool, lambda s: s._restarting, notify=restartingChanged)

    @Slot()
    def restartBackend(self) -> None:
        if self._restarting:
            return
        self._restarting = True
        self.restartingChanged.emit()
        self._s.backend.restart()

    def _on_backend_restarted(self, url: str, err: str) -> None:
        if not self._restarting:
            return
        if err:
            self._toast.error(f"Backend restart failed: {err}")
            self._restart_done()
            return
        self._wait_status(0)

    def _wait_status(self, attempt: int) -> None:
        """The session reconnects on its own; wait (<= 15 s) for the API to say so."""
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                logged = bool(body.get("isLoggedIn"))
                self._s.app.set_auth("authenticated" if logged else "unauthenticated", body.get("phone") or None)
            elif attempt + 1 < RESTART_WAIT_STEPS:
                QTimer.singleShot(RESTART_WAIT_MS, lambda: self._wait_status(attempt + 1))
                return
            self._toast.success("Backend restarted")
            self._restart_done()
            self.refresh()
            self.historyImported.emit()  # chats_stale

        self._api.fetch("GET", "/status", None, done)

    def _restart_done(self) -> None:
        self._restarting = False
        self.restartingChanged.emit()

    def _on_connected(self) -> None:
        if self._visited and self._s.hub.connected:
            self.refresh()

    # ===================================================================== history sync
    historyCard = Property("QVariantMap", lambda s: s._history_card(), notify=historyChanged)

    def _history_card(self) -> dict:
        card = ht.sync_card(self._sync_status, self._sync_active)
        card["depths"] = [{"id": d["id"], "label": d["label"], "selected": d["id"] == self._depth}
                          for d in ht.SYNC_DEPTHS]
        card["hint"] = ht.depth(self._depth)["hint"]
        card["retryFinished"] = self._retry_finished
        return card

    @Slot()
    def loadHistoryStatus(self) -> None:
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._apply_sync(body)

        self._api.fetch("GET", "/history-sync/status", None, done)

    def _apply_sync(self, st: dict) -> None:
        was = self._sync_active
        now = ht.is_active(st.get("state"))
        self._sync_active, self._sync_status = now, st
        if now:
            if not self._sync_timer.isActive():
                self._sync_timer.start()
        else:
            self._sync_timer.stop()
        if was and not now:
            state, added = st.get("state"), st.get("messagesAdded") or 0
            if state == "failed":
                self._toast.error("History sync failed")
            elif state == "cancelled":
                self._toast.show(f"History sync stopped: {added} messages added", "info")
            else:
                self._toast.success(f"History sync added {added} messages")
            self.historyImported.emit()
        self.historyChanged.emit()

    def _poll_sync(self) -> None:
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._apply_sync(body)
            elif self._sync_active:
                self._sync_timer.start()  # transient failure: keep following

        self._api.fetch("GET", "/history-sync/status", None, done)

    @Slot(str)
    def setHistoryDepth(self, depth_id: str) -> None:
        if not self._sync_active:
            self._depth = ht.depth(depth_id)["id"]
            self.historyChanged.emit()

    @Slot()
    def toggleRetryFinished(self) -> None:
        if not self._sync_active:
            self._retry_finished = not self._retry_finished
            self.historyChanged.emit()

    @Slot(bool)
    def startHistorySync(self, resume: bool) -> None:
        if self._sync_active:
            return
        self._sync_active = True
        body = ht.sync_request(self._depth, self._retry_finished, resume)
        self.historyChanged.emit()

        def done(status: int, resp: Any) -> None:
            if status in (200, 201, 202) and isinstance(resp, dict):
                self._sync_active = ht.is_active(resp.get("state"))
                self._sync_status = resp
                if self._sync_active and not self._sync_timer.isActive():
                    self._sync_timer.start()
                self._toast.success("History sync resumed" if resume else "History sync started")
            elif status == 409 and isinstance(resp, dict):
                self._sync_active = True  # follow the one that is running instead of failing
                self._sync_status = resp
                if not self._sync_timer.isActive():
                    self._sync_timer.start()
                self._toast.show("A sync is already running. Stop it first to start a different depth.", "info")
            else:
                self._sync_active = False
                reason = _reason(status, resp)
                what = "resume" if resume else "start"
                self._toast.error(f"Failed to {what} history sync: {reason}" if reason else f"Failed to {what} history sync")
                self.loadHistoryStatus()
            self.historyChanged.emit()

        self._api.fetch("POST", "/history-sync", body, done)

    @Slot()
    def cancelHistorySync(self) -> None:
        if not self._sync_active:
            return

        def done(status: int, body: Any) -> None:
            if status in (200, 202) and isinstance(body, dict):
                self._sync_status = body
                if not self._sync_timer.isActive():
                    self._sync_timer.start()
            else:
                self._toast.error("Could not stop the history sync")
            self.historyChanged.emit()

        self._api.fetch("POST", "/history-sync/cancel", None, done)

    def _on_event(self, typ: str, payload: str) -> None:
        if typ != "history_sync":
            return
        import json
        try:
            st = json.loads(payload)
        except ValueError:
            return
        if isinstance(st, dict) and "state" in st:
            self._apply_sync(st)

    # ===================================================================== history media + storage
    mediaCard = Property("QVariantMap", lambda s: s._media_card(), notify=mediaChanged)
    storageText = Property(str, lambda s: ht.usage_text(s._usage) or "", notify=storageChanged)

    def _media_card(self) -> dict:
        return ht.media_card(self._media_status, self._media_kinds, self._media_retry,
                             self._media_ask_phone, self._usage, self._media_active)

    @Slot()
    def loadStorageUsage(self) -> None:
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._usage = body
                self.storageChanged.emit()
                self.mediaChanged.emit()

        self._api.fetch("GET", "/storage/usage", None, done)

    @Slot()
    def loadMediaStatus(self) -> None:
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._apply_media(body)

        self._api.fetch("GET", "/history-media/status", None, done)

    def _apply_media(self, st: dict) -> None:
        was = self._media_active
        now = ht.is_active(st.get("state"))
        self._media_active, self._media_status = now, st
        if now:
            self._media_polls += 1
            if self._media_polls % 5 == 0:
                self.loadStorageUsage()
            if not self._media_timer.isActive():
                self._media_timer.start()
        else:
            self._media_timer.stop()
        if was and not now:
            self.loadStorageUsage()
            state, done = st.get("state"), st.get("done") or 0
            if state == "failed":
                self._toast.error("Media download failed")
            elif state == "cancelled":
                self._toast.show(f"Media download stopped: {done} files downloaded", "info")
            else:
                self._toast.success(f"Downloaded {done} files")
            self.historyImported.emit()
        self.mediaChanged.emit()

    def _poll_media(self) -> None:
        def done(status: int, body: Any) -> None:
            if status == 200 and isinstance(body, dict):
                self._apply_media(body)
            elif self._media_active:
                self._media_timer.start()

        self._api.fetch("GET", "/history-media/status", None, done)

    @Slot(str)
    def toggleMediaKind(self, kind: str) -> None:
        if self._media_active:
            return
        if kind in self._media_kinds:
            self._media_kinds.remove(kind)
        else:
            self._media_kinds.append(kind)
        self.mediaChanged.emit()

    @Slot()
    def toggleAskPhone(self) -> None:
        if not self._media_active:
            self._media_ask_phone = not self._media_ask_phone
            self.mediaChanged.emit()

    @Slot()
    def toggleRetryUnavailable(self) -> None:
        if not self._media_active:
            self._media_retry = not self._media_retry
            self.mediaChanged.emit()

    @Slot(bool)
    def startHistoryMedia(self, resume: bool) -> None:
        if self._media_active:
            return
        if not resume and not self._media_kinds:
            return
        self._media_active = True
        body = ht.media_request(self._media_kinds, self._media_retry, self._media_ask_phone, resume)
        self.mediaChanged.emit()

        def done(status: int, resp: Any) -> None:
            if status in (200, 201, 202) and isinstance(resp, dict):
                self._media_active = ht.is_active(resp.get("state"))
                self._media_status = resp
                if self._media_active and not self._media_timer.isActive():
                    self._media_timer.start()
                self._toast.success("Media download resumed" if resume else "Media download started")
            elif status == 409 and isinstance(resp, dict):
                self._media_active = True
                self._media_status = resp
                if not self._media_timer.isActive():
                    self._media_timer.start()
                self._toast.show("A media download is already running.", "info")
            else:
                self._media_active = False
                reason = _reason(status, resp)
                self._toast.error(f"Failed to start the media download: {reason}" if reason
                                  else "Failed to start the media download")
                self.loadMediaStatus()
            self.mediaChanged.emit()

        self._api.fetch("POST", "/history-media", body, done)

    @Slot()
    def cancelHistoryMedia(self) -> None:
        if not self._media_active:
            return

        def done(status: int, body: Any) -> None:
            if status in (200, 202) and isinstance(body, dict):
                self._media_status = body
                if not self._media_timer.isActive():
                    self._media_timer.start()
            else:
                self._toast.error("Could not stop the media download")
            self.mediaChanged.emit()

        self._api.fetch("POST", "/history-media/cancel", None, done)

    # ===================================================================== UI preferences
    def _pref(self, key: str, default: Any = None) -> Any:
        return self._settings.get_py(key, default)

    themeModeFilter = Property(str, lambda s: {"dark": "dark", "light": "light"}.get(s._pref("theme_mode"), "all"),
                               notify=prefsChanged)
    notificationsEnabled = Property(bool, lambda s: bool(s._pref("notifications_enabled", True)), notify=prefsChanged)
    formatMessages = Property(bool, lambda s: bool(s._pref("format_messages", True)), notify=prefsChanged)
    chatLayout = Property(str, lambda s: s._pref("chat_layout") if s._pref("chat_layout") in ("auto", "pages", "sidebar") else "auto",
                          notify=prefsChanged)
    windowBlurEnabled = Property(bool, lambda s: bool(s._pref("window_blur_enabled", True)), notify=prefsChanged)
    windowTransparency = Property(int, lambda s: int(s._pref("window_transparency", 28)), notify=prefsChanged)
    uiFontScale = Property(int, lambda s: int(s._pref("ui_font_scale", 100)), notify=prefsChanged)
    chatFontScale = Property(int, lambda s: int(s._pref("chat_font_scale", 100)), notify=prefsChanged)
    activePreset = Property(str, lambda s: s._theme.presetName, notify=prefsChanged)

    @Slot(bool)
    def setNotificationsEnabled(self, on: bool) -> None:
        self._settings.set("notifications_enabled", bool(on))

    @Slot(bool)
    def setFormatMessages(self, on: bool) -> None:
        self._settings.set("format_messages", bool(on))

    @Slot(str)
    def setChatLayout(self, layout: str) -> None:
        if layout in ("auto", "pages", "sidebar"):
            self._settings.set("chat_layout", layout)

    @Slot(bool)
    def setWindowBlurEnabled(self, on: bool) -> None:
        self._settings.set("window_blur_enabled", bool(on))

    @Slot(str)
    def applyPreset(self, name: str) -> None:
        self._theme.setPreset(name)

    @Slot(str)
    def setThemeModeFilter(self, mode: str) -> None:
        """GPUI `set_theme_mode_filter`: switch to the sibling `-dark`/`-light` preset when the active
        one doesn't match, then persist the mode ("all" is stored as "system")."""
        if mode not in ("all", "dark", "light"):
            return
        cur = self._theme.find(self._theme.presetName)
        cur_dark = is_dark(cur["colors"]["background"]) if cur else True
        if (mode == "dark" and not cur_dark) or (mode == "light" and cur_dark):
            base = self._theme.presetName
            while base.endswith("-dark"):
                base = base[:-5]
            while base.endswith("-light"):
                base = base[:-6]
            target = f"{base}-{mode}"
            if self._theme.find(target):
                self._theme.setPreset(target)
        self._settings.set("theme_mode", mode if mode != "all" else "system")

    presetsRevision = Property(int, lambda s: s._presets_rev, notify=presetsChanged)

    @Slot(str, str, result="QVariantList")
    def presetList(self, mode: str, query: str = "") -> list:
        """Swatch data for the picker. The desktop (GTK) preset comes first and ignores the
        dark/light filter (it follows whatever the desktop uses); `query` filters by label/name."""
        q = query.strip().lower()
        rows = []
        for p in self._theme.presets():
            c = p["colors"]
            dark = is_dark(c["background"])
            is_gtk = p["name"] == GTK_PRESET_NAME
            if not is_gtk and ((mode == "dark" and not dark) or (mode == "light" and dark)):
                continue
            if q and q not in p["label"].lower() and q not in p["name"].lower():
                continue
            rows.append({
                "name": p["name"], "label": p["label"], "dark": dark, "gtk": is_gtk,
                "bg": c["background"], "fg": c["foreground"], "mutedFg": c["mutedForeground"],
                "primary": c["primary"], "accent": c["accent"], "destructive": c["destructive"],
            })
        return rows

    # ===================================================================== desktop (GTK) palette
    def _sync_gtk_preset(self, first: bool = False) -> None:
        p = gtk_theme.preset(self._gtk_config)
        if p:
            self._theme.register_preset(p)
        else:
            self._theme.unregister_preset(GTK_PRESET_NAME)
        self._presets_rev += 1
        if not first:
            self.presetsChanged.emit()

    def _poll_gtk(self) -> None:
        """Follow the desktop theme live: re-read the GTK config when it changed on disk (the desktop
        switched theme or dark/light); restyles the app when the desktop theme is the active one."""
        key = gtk_theme.current_key(self._gtk_config)
        if key != self._gtk_key:
            self._gtk_key = key
            self._sync_gtk_preset()

    # ===================================================================== window effects
    @Property(str, constant=True)
    def platformFramework(self) -> str:
        import PySide6
        return f"Qt {qVersion()} Quick (QML) + PySide6 {PySide6.__version__}"

    @Property(str, constant=True)
    def appVersion(self) -> str:
        return app_version()

    def _on_object_created(self, obj, _url) -> None:
        from PySide6.QtGui import QWindow
        if isinstance(obj, QWindow) and self._window is None:
            self._window = obj
            self.apply_window_effects()

    def _on_pref_for_window(self, key: str) -> None:
        if key in ("window_blur_enabled", "window_transparency"):
            self.apply_window_effects()

    def apply_window_effects(self) -> str:
        """Translucency is the window's own (Main.qml: `color: transparent` while transparency > 0, content
        painted with `Theme.glass`). Qt has no public backdrop-blur API, so blur is best effort:

          X11 (xcb)   sets/clears `_KDE_NET_WM_BLUR_BEHIND_REGION` (honoured by KWin and picom/compton
                      configs that read it). Needs `xprop`.
          Wayland     not possible from Qt/PySide: the compositor protocols (KDE blur,
                      `ext-background-effect-v1` on COSMIC) have no public Qt binding, so the window is just
                      translucent. GNOME has no blur protocol at all.

        Returns what was done: "x11", "unsupported" or "none"."""
        win = self._window
        if win is None:
            return "none"
        from PySide6.QtGui import QGuiApplication
        if QGuiApplication.platformName() != "xcb":
            return "unsupported"
        import shutil
        xprop = shutil.which("xprop")
        if not xprop or not win.winId():
            return "none"
        want = bool(self._pref("window_blur_enabled", True)) and int(self._pref("window_transparency", 0)) > 0
        args = ["-id", str(int(win.winId()))]
        args += (["-f", "_KDE_NET_WM_BLUR_BEHIND_REGION", "32c", "-set", "_KDE_NET_WM_BLUR_BEHIND_REGION", "0"]
                 if want else ["-remove", "_KDE_NET_WM_BLUR_BEHIND_REGION"])
        QProcess.startDetached(xprop, args)
        return "x11"


def register(services) -> None:
    services.system = System(services)
    services.expose("System", services.system)
