"""Backend lifecycle for the UI: resolve (attach or spawn) off the UI thread,
expose the API URL, and stop only a backend this process started."""
from __future__ import annotations

import os
import threading

from PySide6.QtCore import QObject, QTimer, Property, Signal, Slot

from . import discovery


class BackendController(QObject):
    stateChanged = Signal()
    apiUrlChanged = Signal()
    _resolved = Signal(str, object, str)  # url, OwnedBackend|None, error

    def __init__(self, argv: list[str], parent: QObject | None = None):
        super().__init__(parent)
        self._argv = argv
        self._explicit = discovery.explicit_url(argv, dict(os.environ)) is not None
        self._api_url = ""
        self._state = "resolving"  # resolving | ready | error
        self._error = ""
        self._owned: discovery.OwnedBackend | None = None
        self._busy = False
        self._closed = False
        self._resolved.connect(self._on_resolved)
        # An owned backend that dies is restarted; an attached one is re-resolved
        # by `recheck()` (called when the WebSocket drops).
        self._watch = QTimer(self)
        self._watch.setInterval(2000)
        self._watch.timeout.connect(self._check_owned)
        self._watch.start()

    # -- properties -------------------------------------------------------
    @Property(str, notify=apiUrlChanged)
    def apiUrl(self) -> str:
        return self._api_url

    @Property(str, notify=stateChanged)
    def state(self) -> str:
        return self._state

    @Property(str, notify=stateChanged)
    def error(self) -> str:
        return self._error

    @Property(bool, notify=stateChanged)
    def owner(self) -> bool:
        return self._owned is not None

    def ws_url(self) -> str:
        """Asked again on every WebSocket (re)connect so a restarted backend on a
        new port is followed."""
        return discovery.ws_url(self._api_url) if self._api_url else ""

    def root_url(self) -> str:
        return self._api_url.removesuffix("/api")

    # -- lifecycle --------------------------------------------------------
    @Slot()
    def start(self) -> None:
        if self._busy or self._closed:
            return
        self._busy = True
        self._set_state("resolving", "")
        threading.Thread(target=self._work, name="backend-resolve", daemon=True).start()

    def _work(self) -> None:
        try:
            url, owned, err = discovery.resolve(self._argv)
        except Exception as e:  # never let a worker die silently
            url, owned, err = "", None, str(e)
        self._resolved.emit(url, owned, err or "")

    def _on_resolved(self, url: str, owned, err: str) -> None:
        self._busy = False
        if self._closed:  # shut down while resolving: don't keep what it started
            if owned is not None:
                owned.stop()
            return
        if owned is not None:
            self._owned = owned
        if url != self._api_url:
            self._api_url = url
            self.apiUrlChanged.emit()
        self._set_state("error" if err and url == discovery.DEFAULT_URL else "ready", err)

    def _set_state(self, state: str, error: str) -> None:
        self._state, self._error = state, error
        self.stateChanged.emit()

    def _check_owned(self) -> None:
        if self._owned and self._owned.proc.poll() is not None and not self._busy and not self._closed:
            print("[backend] owned backend exited; restarting")
            discovery.remove_core_if(discovery.core_file(), self._owned.record)
            self._owned = None
            self.start()

    @Slot()
    def recheck(self) -> None:
        """The socket dropped: if the backend moved (and wasn't pinned by CLI/env),
        find it again."""
        if not self._explicit and self._owned is None and not self._busy and not self._closed:
            self.start()

    def shutdown(self) -> None:
        """Stop the backend this process started, if any. Attached ones are left alone."""
        self._closed = True
        self._watch.stop()
        owned, self._owned = self._owned, None
        if owned:
            owned.stop()
