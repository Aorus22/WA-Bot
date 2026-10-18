#!/usr/bin/env python3
"""Start a frozen Linux bundle against the local mock with isolated user paths."""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))
from mock_backend import MockBackend  # noqa: E402


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: smoke-bundle.py /path/to/frozen/wa-bot", file=sys.stderr)
        return 2
    executable = Path(sys.argv[1]).resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        print(f"bundle executable missing or not executable: {executable}", file=sys.stderr)
        return 2
    backend = executable.parent / "_internal" / "wa-bot-backend"
    if not backend.is_file() or not os.access(backend, os.X_OK):
        print(f"bundled backend missing or not executable: {backend}", file=sys.stderr)
        return 2

    mock: MockBackend | None = None
    app: subprocess.Popen | None = None
    with tempfile.TemporaryDirectory(prefix="wa-bot-qml-bundle-") as temp:
        isolated = Path(temp)
        runtime = isolated / "runtime"
        for name in ("config", "cache", "data", "wabot", "runtime"):
            (isolated / name).mkdir(mode=0o700)

        env = os.environ.copy()
        env.update(
            QT_QPA_PLATFORM="offscreen",
            QT_QUICK_BACKEND="software",
            QSG_RHI_BACKEND="software",
            WABOT_HOME=str(isolated / "wabot"),
            XDG_CONFIG_HOME=str(isolated / "config"),
            XDG_CACHE_HOME=str(isolated / "cache"),
            XDG_DATA_HOME=str(isolated / "data"),
            XDG_RUNTIME_DIR=str(runtime),
        )
        try:
            codec = subprocess.run(
                [str(executable), "--self-test-h264"],
                cwd=executable.parent,
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
            )
        except subprocess.TimeoutExpired:
            print("bundle smoke failed: H.264 self-test timed out", file=sys.stderr)
            return 1
        if codec.returncode != 0:
            print(codec.stdout, file=sys.stderr, end="")
            print(codec.stderr, file=sys.stderr, end="")
            print(f"bundle smoke failed: H.264 self-test exited {codec.returncode}", file=sys.stderr)
            return 1

        mock = MockBackend(areas=["chats", "rich", "calls", "composer", "bot"]).start()
        app = subprocess.Popen(
            [str(executable), "--url", mock.url],
            cwd=executable.parent,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        started = time.monotonic()
        try:
            deadline = started + 20
            status_seen = False
            chats_seen = False
            ws_seen = False
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    break
                with mock._lock:
                    paths = {
                        (method, path.split("?", 1)[0])
                        for method, path, _body in mock.requests
                    }
                    status_seen = ("GET", "/api/status") in paths
                    chats_seen = ("GET", "/api/chats") in paths
                    ws_seen = bool(mock.clients)
                if status_seen and chats_seen and ws_seen:
                    break
                time.sleep(0.05)

            if app.poll() is None and status_seen and chats_seen and ws_seen:
                time.sleep(1)
            lived = time.monotonic() - started
            app.terminate()
            try:
                output, _ = app.communicate(timeout=4)
            except subprocess.TimeoutExpired:
                app.kill()
                output, _ = app.communicate(timeout=4)

            bad_markers = (
                "[qml]",
                "QQmlApplicationEngine failed",
                "module \"Qt",
                "Cannot load library",
                "Traceback (most recent call last):",
            )
            failures = [marker for marker in bad_markers if marker in output]
            if (app.returncode not in (-15, -9) or not status_seen or not chats_seen
                    or not ws_seen or lived < 1 or failures):
                print(output, file=sys.stderr, end="")
                print(
                    f"bundle smoke failed: exit={app.returncode}, status_seen={status_seen}, "
                    f"chats_seen={chats_seen}, ws_seen={ws_seen}, lived={lived:.1f}s, errors={failures}",
                    file=sys.stderr,
                )
                return 1

            print(codec.stdout.strip())
            print(f"bundle smoke passed: {executable.name} loaded QML, REST auth/chat and the mock WebSocket")
            return 0
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=4)
            if mock is not None:
                mock.stop()


if __name__ == "__main__":
    raise SystemExit(main())
