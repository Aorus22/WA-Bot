"""Exercise GPUI's group Add member action against a local mock backend."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import TEAM  # noqa: E402


def _run_group_action(tmp_path, backend, *options):
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / ("group-" + "-".join(option.lstrip("-") for option in options[::2]) + ".png")
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--select-chat", TEAM,
            *options,
            "--preset", "default-dark",
            "--theme-mode", "dark",
            "--size", "1100x720",
            "--wait", "500",
        ],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=25,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "[qml]" not in result.stdout + result.stderr
    assert output.is_file()
    return result.stdout


def test_group_add_member_dialog_uses_participant_rest_contract(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    member = "6286666666666@s.whatsapp.net"
    try:
        output = _run_group_action(
            tmp_path,
            backend,
            "--group-member-action", "add",
            "--group-member", member,
        )
        assert f"group admin mouse action applied: add:{member}" in output
        assert any(p["jid"] == member for p in backend.state["groups"][TEAM]["participants"])
        requests = [json.loads(body) for method, path, body in backend.requests
                    if method == "POST" and path.endswith("/participants")]
        assert requests[-1] == {"action": "add", "jids": [member]}
    finally:
        backend.stop()
