#!/usr/bin/env python3
"""Run React, GPUI, and QML at once against one isolated mock backend.

This checks that the three frontend processes can independently establish their
REST and JSON WebSocket connections. It never starts the Go backend or uses an
account/configuration directory. Requirements: web/node_modules, the GPUI debug
binary, Firefox, Xvfb, and the desktop-qml virtual environment.
"""
from __future__ import annotations

import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
sys.path.insert(0, str(ROOT / "tests"))
from mock_backend import MockBackend  # noqa: E402


def _require() -> list[str]:
    missing = [name for name in ("npm", "firefox", "xvfb-run", "dbus-run-session") if shutil.which(name) is None]
    gpui = Path(os.environ.get("WABOT_GPUI_BIN", REPO / "desktop-gpui" / "target" / "debug" / "wabot"))
    if not gpui.is_file() or not os.access(gpui, os.X_OK):
        missing.append(f"GPUI executable ({gpui})")
    if not (REPO / "web" / "node_modules").is_dir():
        missing.append("web/node_modules")
    return missing


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


def _tail(path: Path) -> str:
    try:
        return path.read_text(errors="replace")[-1800:]
    except OSError:
        return ""


def main() -> int:
    missing = _require()
    if missing:
        print("concurrent smoke requires: " + ", ".join(missing), file=sys.stderr)
        return 2

    gpui_bin = Path(os.environ.get("WABOT_GPUI_BIN", REPO / "desktop-gpui" / "target" / "debug" / "wabot"))
    backend = MockBackend(areas=["chats", "rich", "calls", "composer", "bot", "status", "channels"]).start()
    procs: list[subprocess.Popen] = []
    with tempfile.TemporaryDirectory(prefix="wa-bot-three-client-") as raw_temp:
        temp = Path(raw_temp)
        logs = {name: temp / f"{name}.log" for name in ("vite", "firefox", "gpui", "qml")}
        handles = {name: path.open("w") for name, path in logs.items()}
        try:
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

            gpui_data = temp / "gpui"
            qml_data = temp / "qml"
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
            status_count = chat_count = ws_count = 0
            while time.monotonic() < deadline:
                with backend._lock:
                    paths = [(method, path.split("?", 1)[0]) for method, path, _body in backend.requests]
                    status_count = sum((method, path) == ("GET", "/api/status") for method, path in paths)
                    chat_count = sum((method, path) == ("GET", "/api/chats") for method, path in paths)
                    ws_count = len(backend.clients)
                if status_count >= 3 and chat_count >= 3 and ws_count >= 3:
                    break
                if any(proc.poll() is not None for proc in procs):
                    break
                time.sleep(0.1)

            alive = all(proc.poll() is None for proc in procs)
            print(f"concurrent mock clients: WebSocket={ws_count}, GET /status={status_count}, GET /chats={chat_count}")
            print(f"processes alive at observation: React/Firefox={firefox.poll() is None}, GPUI={gpui.poll() is None}, QML={qml.poll() is None}")
            if status_count < 3 or chat_count < 3 or ws_count < 3 or not alive:
                for name, path in logs.items():
                    handles[name].flush()
                    print(f"--- {name} ---\n{_tail(path)}", file=sys.stderr)
                return 1
            print("three frontend processes connected concurrently to one isolated mock backend")
            return 0
        finally:
            for proc in reversed(procs):
                _stop_group(proc)
            for handle in handles.values():
                handle.close()
            backend.stop()


if __name__ == "__main__":
    raise SystemExit(main())
