"""Fake desktop services for the platform tests, run on a PRIVATE session bus (never the user's):

  - org.freedesktop.Notifications server (logs every Notify as one JSON line on stdout)
  - org.kde.StatusNotifierWatcher (so a tray icon can register; logs REGISTER)

stdin commands (one per line):  action <id> <key>   |  closed <id> <reason>   |  quit
Needs `dbus_next`; tests skip when it is missing.
"""
import asyncio
import json
import sys

from dbus_next import Variant
from dbus_next.aio import MessageBus
from dbus_next.service import PropertyAccess, ServiceInterface, dbus_property, method, signal


def plain(v):
    return {"sig": v.signature, "value": v.value if not isinstance(v.value, (bytes, Variant)) else str(v.value)}


def emit(obj):
    print(json.dumps(obj), flush=True)


class Notifications(ServiceInterface):
    def __init__(self, with_actions=True):
        super().__init__("org.freedesktop.Notifications")
        self.next_id = 1
        self.with_actions = with_actions

    @method()
    def GetCapabilities(self) -> "as":
        return ["body", "persistence", "icon-static"] + (["actions"] if self.with_actions else [])

    @method()
    def GetServerInformation(self) -> "ssss":
        return ["fake-notifyd", "tests", "1.0", "1.2"]

    @method()
    def Notify(self, app_name: "s", replaces_id: "u", app_icon: "s", summary: "s", body: "s",
               actions: "as", hints: "a{sv}", expire_timeout: "i") -> "u":
        nid = replaces_id or self.next_id
        if not replaces_id:
            self.next_id += 1
        emit({"ev": "notify", "id": nid, "app": app_name, "replaces": replaces_id, "icon": app_icon,
              "summary": summary, "body": body, "actions": actions,
              "hints": {k: plain(v) for k, v in hints.items()}, "expire": expire_timeout})
        return nid

    @method()
    def CloseNotification(self, id: "u"):
        emit({"ev": "close", "id": id})
        self.NotificationClosed(id, 3)

    @signal()
    def NotificationClosed(self, id: "u", reason: "u") -> "uu":
        return [id, reason]

    @signal()
    def ActionInvoked(self, id: "u", key: "s") -> "us":
        return [id, key]


class Watcher(ServiceInterface):
    def __init__(self):
        super().__init__("org.kde.StatusNotifierWatcher")
        self.items = []

    @method()
    def RegisterStatusNotifierItem(self, service: "s"):
        self.items.append(service)
        emit({"ev": "register", "service": service})

    @dbus_property(access=PropertyAccess.READ)
    def RegisteredStatusNotifierItems(self) -> "as":
        return self.items

    @dbus_property(access=PropertyAccess.READ)
    def IsStatusNotifierHostRegistered(self) -> "b":
        return True

    @dbus_property(access=PropertyAccess.READ)
    def ProtocolVersion(self) -> "i":
        return 0


async def main():
    flags = set(sys.argv[1:])
    bus = await MessageBus().connect()
    notif = Notifications(with_actions="--no-actions" not in flags)
    if "--no-notifications" not in flags:
        bus.export("/org/freedesktop/Notifications", notif)
        await bus.request_name("org.freedesktop.Notifications")
    if "--no-watcher" not in flags:
        bus.export("/StatusNotifierWatcher", Watcher())
        await bus.request_name("org.kde.StatusNotifierWatcher")
    emit({"ev": "ready"})
    loop = asyncio.get_running_loop()
    reader = asyncio.StreamReader()
    await loop.connect_read_pipe(lambda: asyncio.StreamReaderProtocol(reader), sys.stdin)
    while True:
        line = (await reader.readline()).decode().split()
        if not line or line[0] == "quit":
            break
        if line[0] == "action":
            notif.ActionInvoked(int(line[1]), line[2])
        elif line[0] == "closed":
            notif.NotificationClosed(int(line[1]), int(line[2]))


asyncio.run(main())
