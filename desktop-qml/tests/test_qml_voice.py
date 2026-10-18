"""The QML voice-note control reports a missing microphone without opening hardware."""
from __future__ import annotations

import os
import subprocess
import sys
import io
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_common import BUDI  # noqa: E402


def test_record_button_reports_unavailable_microphone(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / "voice-microphone-error.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--voice-no-device",
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
        assert "voice record click reported unavailable microphone and returned to idle" in result.stdout
        assert not any(call["name"] in {"send-media", "send-message"}
                       for call in backend.state["composer"]["calls"])
        assert output.is_file()
    finally:
        backend.stop()


def test_record_and_send_voice_note_uploads_canonical_wav_as_ptt(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    output = tmp_path / "voice-note-sent.png"
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "shot.py"),
                str(output),
                "--url", backend.url,
                "--route", "chat",
                "--select-chat", BUDI,
                "--voice-send-synthetic",
                "--preset", "default-dark",
                "--theme-mode", "dark",
                "--size", "1100x720",
                "--wait", "500",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        assert "[qml]" not in result.stdout + result.stderr
        assert "QML voice recording flow uploaded a synthetic WAV push-to-talk message" in result.stdout
        uploads = [call for call in backend.state["composer"]["calls"] if call["name"] == "send-media"]
        assert len(uploads) == 1
        upload = uploads[0]["body"]
        assert upload["fields"]["target"] == BUDI
        assert upload["fields"]["type"] == "ptt"
        assert upload["fields"]["ptt"] == "true"
        assert upload["fields"]["seconds"] == "2"
        assert upload["file"]["name"] == "voice-note.wav"
        assert upload["file"]["content_type"] == "audio/wav"
        with wave.open(io.BytesIO(upload["file"]["data"]), "rb") as wav:
            assert (wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes()) == (1, 2, 16_000, 32_000)
        assert output.is_file()
    finally:
        backend.stop()
