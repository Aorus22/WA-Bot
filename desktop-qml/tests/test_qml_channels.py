"""Drive a channel list row in a local QML scene, never a live account."""
from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_channels import TECH  # noqa: E402


def test_channel_row_mouse_click_opens_feed(tmp_path):
    backend = MockBackend(areas=["channels"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / "channel-feed.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "channels",
                "--click-channel", TECH,
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
        assert f"channel row mouse click: {TECH} →" in result.stdout
        assert output.is_file()
    finally:
        backend.stop()


def test_channel_detail_mute_toggle_uses_shared_rest_contract(tmp_path):
    backend = MockBackend(areas=["channels"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / "channel-muted.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "channels",
                "--click-channel", TECH,
                "--channel-mute",
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
        assert "channel mute mouse action applied: True" in result.stdout
        channel = next(row for row in backend.state["channels"] if row["jid"] == TECH)
        assert channel["muted"] is True
        assert ("mute", TECH, True) in backend.state["channel_calls"]
        assert output.is_file()
    finally:
        backend.stop()


def test_channel_post_quick_reaction_uses_shared_rest_contract(tmp_path):
    backend = MockBackend(areas=["channels"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    try:
        output = tmp_path / "channel-reaction.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "channels",
                "--click-channel", TECH,
                "--channel-reaction", "👍",
                "--channel-reaction-post", "p1",
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
        assert "channel reaction mouse action applied: 👍" in result.stdout
        deadline = time.monotonic() + 3
        while not backend.state["channel_calls"] and time.monotonic() < deadline:
            time.sleep(0.02)
        assert ("react", TECH, 1, "p1", "👍") in backend.state["channel_calls"]
        reactions = backend.state["channel_posts"][TECH][0]["reactions"]
        assert reactions["👍"] == 6
        assert output.is_file()
    finally:
        backend.stop()


def test_channel_discovery_follow_flow_uses_shared_rest_contract(tmp_path):
    backend = MockBackend(areas=["channels"]).start()
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    url = "https://whatsapp.com/channel/NewTech"
    try:
        output = tmp_path / "channel-follow.png"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "channels",
                "--channel-follow-url", url,
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
        assert "channel followed through QML mouse controls" in result.stdout
        assert ("follow", url) in backend.state["channel_calls"]
        assert any(channel["name"] == "Preview NewTech" for channel in backend.state["channels"])
        assert output.is_file()
    finally:
        backend.stop()
