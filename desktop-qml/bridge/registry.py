"""Wires services into QML. Feature modules plug in without touching main.py:
`bridge/<name>.py` defining `register(services)` is imported and called at start-up;
it may add QML globals through `services.expose(name, obj)` and keep Python-side
listeners on `services.hub.event`.
Order matters only for modules that depend on another's singleton.
"""
from __future__ import annotations

import importlib
import traceback
from dataclasses import dataclass, field
from typing import Any


FEATURE_MODULES = [
    "auth",       # Auth: login/QR/pairing state, sets AppState auth
    "chats",      # Chats: chat list store, chat actions, groups, sheets data
    "messages",   # Messages: active-conversation messages, history paging, per-message actions
    "composer",   # Composer: send text/media/voice/poll/location/contact, reply/edit state
    "statuses",   # Statuses: status stories
    "channels",   # Channels
    "calls",      # Calls: call state + media transport
    "botadmin",   # BotAdmin: triggers, cron, webhooks, docs, AI assistant
    "system",     # System: settings page backing, history sync, GTK palette
    "platform",   # Platform: notifications, tray, single instance, popover
]


@dataclass
class Services:
    backend: Any
    api: Any
    hub: Any
    settings: Any
    theme: Any
    app: Any  # AppState
    qt_app: Any
    toast: Any = None
    engine: Any = None
    exposed: dict[str, Any] = field(default_factory=dict)

    def expose(self, name: str, obj: Any) -> None:
        """Make `obj` visible to every QML file as the global `name` (a context property).
        NB: do not use qmlRegister*: with PySide6 6.11 + Python 3.14 it breaks nested items."""
        self.engine.rootContext().setContextProperty(name, obj)
        self.exposed[name] = obj


def register_all(services: Services) -> list[str]:
    """Expose core services, then load feature modules. Returns names that failed to load."""
    for name, obj in [
        ("Backend", services.backend), ("Api", services.api), ("Events", services.hub),
        ("Settings", services.settings), ("Theme", services.theme), ("AppState", services.app),
        ("Toast", services.toast),
    ]:
        services.expose(name, obj)
    failed = []
    for mod in FEATURE_MODULES:
        try:
            m = importlib.import_module(f"bridge.{mod}")
        except ModuleNotFoundError as e:
            if e.name == f"bridge.{mod}":
                continue  # not written yet
            traceback.print_exc()
            failed.append(mod)
            continue
        try:
            m.register(services)
        except Exception:
            traceback.print_exc()
            failed.append(mod)
    return failed
