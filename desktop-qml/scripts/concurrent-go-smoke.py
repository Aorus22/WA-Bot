#!/usr/bin/env python3
"""Run React, GPUI and QML against a fresh Go backend in a temporary home.

The backend has no account database, credentials, or saved configuration. It
starts an unpaired WhatsApp session to exercise its production HTTP/WS server;
the smoke never scans its QR or sends messages. All temporary state and logs are
removed when the process exits. Requirements match concurrent-smoke.py plus the
current Go backend executable (WABOT_TEST_BACKEND may override its path).
"""
from __future__ import annotations

import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from collections import deque
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent


def _require() -> tuple[list[str], Path]:
    missing = [name for name in ("npm", "firefox", "xvfb-run", "dbus-run-session") if shutil.which(name) is None]
    gpui = Path(os.environ.get("WABOT_GPUI_BIN", REPO / "desktop-gpui" / "target" / "debug" / "wabot"))
    backend = Path(os.environ.get("WABOT_TEST_BACKEND", ROOT / "build" / "pyinstaller" / "wa-bot-backend")).resolve()
    if not gpui.is_file() or not os.access(gpui, os.X_OK):
        missing.append(f"GPUI executable ({gpui})")
    if not (REPO / "web" / "node_modules").is_dir():
        missing.append("web/node_modules")
    if not backend.is_file() or not os.access(backend, os.X_OK):
        missing.append(f"Go backend executable ({backend})")
    return missing, backend


def _free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _stop_group(proc: subprocess.Popen | None) -> None:
    if proc is None or proc.poll() is not None:
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait(timeout=5)


class GoBackend:
    def __init__(self, binary: Path, home: Path):
        self.binary = binary
        self.home = home
        self.home.mkdir(mode=0o700, parents=True)
        self.lines: deque[str] = deque(maxlen=10000)
        self.lock = threading.Lock()
        self.port = 0
        self.process: subprocess.Popen | None = None

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}/api"

    def start(self) -> bool:
        # Go stores the session DB and account folders relative to cwd. Keeping
        # cwd and HOME under this temp root protects the user's real account.
        env = {
            key: os.environ[key]
            for key in ("PATH", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR")
            if key in os.environ
        }
        env.update(
            HOME=str(self.home),
            TMPDIR=str(self.home),
            TZ="Asia/Jakarta",
            PORT=":0",
            ALLOWED_ORIGINS="*",
            DB_PATH=str(self.home / "database"),
            MEDIA_PATH=str(self.home / "media"),
        )
        self.process = subprocess.Popen(
            [str(self.binary)],
            cwd=self.home,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            start_new_session=True,
        )

        def read_output() -> None:
            assert self.process and self.process.stdout
            for line in self.process.stdout:
                with self.lock:
                    self.lines.append(line.rstrip())
                    if not self.port:
                        match = re.search(r"BACKEND_PORT:(\d+)", line)
                        if match:
                            self.port = int(match.group(1))

        threading.Thread(target=read_output, name="go-smoke-log", daemon=True).start()
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline and self.process.poll() is None:
            if self.port:
                try:
                    with urllib.request.urlopen(f"{self.url}/status", timeout=1) as response:
                        if response.status == 200:
                            return True
                except Exception:
                    pass
            time.sleep(0.1)
        return False

    def counts(self) -> tuple[int, int, int]:
        with self.lock:
            lines = tuple(self.lines)
        statuses = sum("[GET] /api/status " in line for line in lines)
        chats = sum("[GET] /api/chats " in line for line in lines)
        websockets = sum("Client connected:" in line for line in lines)
        return statuses, chats, websockets

    def tail(self) -> str:
        with self.lock:
            return "\n".join(tuple(self.lines)[-60:])


def _tail(path: Path) -> str:
    try:
        return path.read_text(errors="replace")[-1800:]
    except OSError:
        return ""


def main() -> int:
    missing, backend_binary = _require()
    if missing:
        print("concurrent Go smoke requires: " + ", ".join(missing), file=sys.stderr)
        return 2
    gpui_bin = Path(os.environ.get("WABOT_GPUI_BIN", REPO / "desktop-gpui" / "target" / "debug" / "wabot"))
    procs: list[subprocess.Popen] = []
    with tempfile.TemporaryDirectory(prefix="wa-bot-three-go-client-") as raw_temp:
        temp = Path(raw_temp)
        logs = {name: temp / f"{name}.log" for name in ("vite", "firefox", "gpui", "qml")}
        handles = {name: path.open("w") for name, path in logs.items()}
        backend = GoBackend(backend_binary, temp / "backend")
        try:
            ready = backend.start()
            if backend.process is not None:
                procs.append(backend.process)
            if not ready:
                print("temporary Go backend did not become ready:\n" + backend.tail(), file=sys.stderr)
                return 1
            assert backend.process is not None

            vite_port = _free_port()
            vite_env = os.environ.copy()
            vite_env.update(
                VITE_API_URL=backend.url,
                VITE_WS_URL=backend.url.replace("http://", "ws://").replace("/api", "/ws"),
            )
            vite = subprocess.Popen(
                ["npm", "run", "dev", "--", "--host", "127.0.0.1", "--port", str(vite_port), "--strictPort"],
                cwd=REPO / "web", env=vite_env, stdout=handles["vite"], stderr=subprocess.STDOUT,
                start_new_session=True, text=True,
            )
            procs.append(vite)
            vite_url = f"http://127.0.0.1:{vite_port}/"
            ready = False
            for _ in range(250):
                if vite.poll() is not None:
                    break
                try:
                    with urllib.request.urlopen(vite_url, timeout=1) as response:
                        ready = response.status == 200
                    if ready:
                        break
                except Exception:
                    time.sleep(0.1)
            if not ready:
                print("React/Vite did not become ready:\n" + _tail(logs["vite"]), file=sys.stderr)
                return 1

            profile = temp / "firefox-profile"
            profile.mkdir()
            firefox_env = os.environ.copy()
            firefox_env["MOZ_HEADLESS"] = "1"
            firefox = subprocess.Popen(
                ["firefox", "--headless", "--new-instance", "--no-remote", "--profile", str(profile), vite_url],
                env=firefox_env, stdout=handles["firefox"], stderr=subprocess.STDOUT,
                start_new_session=True, text=True,
            )
            procs.append(firefox)

            gpui_data, qml_data = temp / "gpui", temp / "qml"
            for base in (gpui_data, qml_data):
                for name in ("config", "data", "cache", "runtime", "wabot"):
                    path = base / name
                    path.mkdir(parents=True, mode=0o700)
                    if name == "runtime":
                        path.chmod(0o700)

            gpui_env = os.environ.copy()
            gpui_env.pop("WAYLAND_DISPLAY", None)
            gpui_env.update(
                XDG_CONFIG_HOME=str(gpui_data / "config"), XDG_DATA_HOME=str(gpui_data / "data"),
                XDG_CACHE_HOME=str(gpui_data / "cache"), XDG_RUNTIME_DIR=str(gpui_data / "runtime"),
                WABOT_HOME=str(gpui_data / "wabot"), LIBGL_ALWAYS_SOFTWARE="1",
                MESA_LOADER_DRIVER_OVERRIDE="llvmpipe",
                __EGL_VENDOR_LIBRARY_FILENAMES="/usr/share/glvnd/egl_vendor.d/50_mesa.json",
                GTK_USE_PORTAL="0", NO_AT_BRIDGE="1",
            )
            gpui = subprocess.Popen(
                ["dbus-run-session", "--", "xvfb-run", "-a", "-s", "-screen 0 1100x720x24",
                 "env", "-u", "WAYLAND_DISPLAY", "LIBGL_ALWAYS_SOFTWARE=1",
                 "MESA_LOADER_DRIVER_OVERRIDE=llvmpipe",
                 "__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json",
                 str(gpui_bin), "--url", backend.url],
                env=gpui_env, stdout=handles["gpui"], stderr=subprocess.STDOUT,
                start_new_session=True, text=True,
            )
            procs.append(gpui)

            qml_env = os.environ.copy()
            qml_env.update(
                QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software", QSG_RHI_BACKEND="software",
                XDG_CONFIG_HOME=str(qml_data / "config"), XDG_DATA_HOME=str(qml_data / "data"),
                XDG_CACHE_HOME=str(qml_data / "cache"), XDG_RUNTIME_DIR=str(qml_data / "runtime"),
                WABOT_HOME=str(qml_data / "wabot"),
            )
            qml = subprocess.Popen(
                [sys.executable, str(ROOT / "main.py"), "--url", backend.url], cwd=ROOT,
                env=qml_env, stdout=handles["qml"], stderr=subprocess.STDOUT,
                start_new_session=True, text=True,
            )
            procs.append(qml)

            deadline = time.monotonic() + 30
            counts = (0, 0, 0)
            while time.monotonic() < deadline:
                counts = backend.counts()
                if min(counts) >= 3:
                    break
                if any(proc.poll() is not None for proc in procs):
                    break
                time.sleep(0.1)
            alive = all(proc.poll() is None for proc in procs)
            print(f"concurrent Go clients: GET /status={counts[0]}, GET /chats={counts[1]}, WebSocket clients={counts[2]}")
            print(f"processes alive at observation: Go={backend.process.poll() is None}, React/Firefox={firefox.poll() is None}, GPUI={gpui.poll() is None}, QML={qml.poll() is None}")
            if min(counts) < 3 or not alive:
                print("--- Go backend ---\n" + backend.tail(), file=sys.stderr)
                for name, path in logs.items():
                    handles[name].flush()
                    print(f"--- {name} ---\n{_tail(path)}", file=sys.stderr)
                return 1
            print("React, GPUI, and QML connected concurrently to one temporary Go backend")
            return 0
        finally:
            for proc in reversed(procs):
                _stop_group(proc)
            for handle in handles.values():
                handle.close()


if __name__ == "__main__":
    raise SystemExit(main())
