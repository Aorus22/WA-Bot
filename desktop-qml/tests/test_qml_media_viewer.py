"""Image preview and document-save flows present in the GPUI chat view."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

from mock_backend import MockBackend  # noqa: E402
from mock_chats import _msg  # noqa: E402
from mock_common import BUDI  # noqa: E402


def _run_shot(output, backend, *args):
    env = os.environ.copy()
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["PYTHONUNBUFFERED"] = "1"
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts" / "shot.py"),
            str(output),
            "--url", backend.url,
            "--route", "chat",
            "--select-chat", BUDI,
            *args,
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
    assert output.is_file()
    return result


def test_image_message_mouse_click_opens_media_viewer(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    rows = backend.state["messages"][BUDI]
    image_row = next(row for row in rows if row["id"] == "bm1")
    image_row["timestamp"] = max(row["timestamp"] for row in rows) + 1000
    rows.sort(key=lambda row: row["timestamp"])
    try:
        output = tmp_path / "image-viewer.png"
        result = _run_shot(output, backend, "--click-media", "bm1")
        assert "media thumbnail mouse click opened viewer: bm1" in result.stdout
        assert any(method == "GET" and path.endswith("/media/b-photo-1.png") for method, path, _ in backend.requests)
    finally:
        backend.stop()


def test_info_drawer_thumbnail_opens_historical_image_outside_loaded_page(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    historical = _msg("info-history-image", BUDI, "Historical photo", 200_000,
                      type_="image", mediaUrl="/media/b-photo-1.png")
    historical["timestamp"] = 1_600_000_000_000
    recent = []
    for index in range(105):
        row = _msg(f"info-history-text-{index}", BUDI, f"Recent {index}", 1)
        row["timestamp"] = 1_700_000_000_000 + index * 1000
        recent.append(row)
    backend.state["messages"][BUDI] = [historical, *recent]
    try:
        output = tmp_path / "historical-info-media-viewer.png"
        result = _run_shot(output, backend, "--chat-info-media", "info-history-image")
        assert "Info drawer media thumbnail opened viewer: info-history-image; loaded_page=False" in result.stdout
        assert any(method == "GET" and path.endswith("/media/b-photo-1.png") for method, path, _ in backend.requests)
    finally:
        backend.stop()


def test_image_accessible_name_and_space_opens_media_viewer(tmp_path):
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    rows = backend.state["messages"][BUDI]
    image_row = next(row for row in rows if row["id"] == "bm1")
    image_row["timestamp"] = max(row["timestamp"] for row in rows) + 1000
    rows.sort(key=lambda row: row["timestamp"])
    try:
        output = tmp_path / "accessible-image-viewer.png"
        result = _run_shot(output, backend, "--keyboard-image", "bm1")
        assert "image accessible name='Open image'; Space opened viewer" in result.stdout
    finally:
        backend.stop()


def test_document_save_button_downloads_to_selected_path(tmp_path):
    payload = b"%PDF-1.4\n% local QML media save fixture\n%%EOF\n"
    backend = MockBackend(areas=["auth", "chats", "rich", "composer", "calls", "bot", "status", "channels"]).start()
    backend.state["logged_in"] = True
    rows = backend.state["messages"][BUDI]
    document = next(row for row in rows if row["id"] == "bm2")
    document["timestamp"] = max(row["timestamp"] for row in rows) + 1000
    rows.sort(key=lambda row: row["timestamp"])

    @backend.route("GET", r"/media/laporan\.pdf")
    def document_file(_req):
        return 200, payload, "application/pdf"

    try:
        output = tmp_path / "document-save.png"
        destination = tmp_path / "saved-laporan.pdf"
        result = _run_shot(output, backend, "--save-document", "bm2", "--save-path", str(destination))
        assert "document QML save control downloaded to:" in result.stdout
        assert destination.read_bytes() == payload
        assert any(method == "GET" and path.endswith("/media/laporan.pdf") for method, path, _ in backend.requests)
    finally:
        backend.stop()
