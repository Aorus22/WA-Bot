"""Private D-Bus session + fake desktop services for tests. Never touches the user's bus."""
from __future__ import annotations

import json
import queue
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def available() -> bool:
    if not shutil.which("dbus-daemon"):
        return False
    try:
        import dbus_next  # noqa: F401
    except ImportError:
        return False
    return True


class PrivateBus:
    def __init__(self):
        self.proc = subprocess.Popen(
            ["dbus-daemon", "--session", "--nofork", "--print-address=1"],
            stdout=subprocess.PIPE, text=True)
        self.address = self.proc.stdout.readline().strip()

    def stop(self):
        self.proc.terminate()
        try:
            self.proc.wait(3)
        except subprocess.TimeoutExpired:
            self.proc.kill()


class FakeSession:
    """Runs fake_session.py on `bus_address`; `events` collects its JSON lines."""

    def __init__(self, bus_address: str, *flags: str):
        import os
        env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=bus_address)
        self.proc = subprocess.Popen([sys.executable, str(HERE / "fake_session.py"), *flags],
                                     env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.events: list[dict] = []
        self._lock = threading.Lock()
        threading.Thread(target=self._read, daemon=True).start()
        assert self.wait(lambda e: e["ev"] == "ready", 10), "fake session did not start"

    def _read(self):
        for line in self.proc.stdout:
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            with self._lock:
                self.events.append(ev)

    def find(self, pred):
        with self._lock:
            return [e for e in self.events if pred(e)]

    def wait(self, pred, timeout=5.0, pump=None):
        """Wait until an event matches; `pump()` (e.g. Qt processEvents) runs while waiting."""
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            hit = self.find(pred)
            if hit:
                return hit[-1]
            if pump:
                pump()
            time.sleep(0.02)
        return None

    def send(self, line: str):
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()

    def stop(self):
        try:
            self.send("quit")
        except OSError:
            pass
        try:
            self.proc.wait(3)
        except subprocess.TimeoutExpired:
            self.proc.kill()
