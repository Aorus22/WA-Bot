"""Wire compatibility checks for the Go call-media H.264 Annex-B contract."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from urllib.request import urlopen

import pytest
from PySide6.QtCore import QCoreApplication, QEventLoop, QTimer
from PySide6.QtGui import QColor, QImage

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from mock_backend import MockBackend  # noqa: E402
from bridge.call_media import frame, video  # noqa: E402
from bridge.call_media.net import MediaNet, media_ws_url  # noqa: E402


def _test_image(width: int = 321, height: int = 241) -> QImage:
    image = QImage(width, height, QImage.Format.Format_RGB888)
    image.fill(QColor(80, 120, 160))
    return image


def _nal_types(access_unit: bytes) -> set[int]:
    starts = list(re.finditer(b"\x00\x00(?:\x00)?\x01", access_unit))
    return {access_unit[start.end()] & 0x1F for start in starts if start.end() < len(access_unit)}


def _spin_until(predicate, timeout_ms: int = 4000) -> bool:
    loop = QEventLoop()
    timer = QTimer(loop)
    timer.setInterval(5)
    timer.timeout.connect(lambda: loop.quit() if predicate() else None)
    timeout = QTimer(loop)
    timeout.setSingleShot(True)
    timeout.timeout.connect(loop.quit)
    timeout.start(timeout_ms)
    timer.start()
    loop.exec()
    timer.stop()
    timeout.stop()
    return predicate()


def test_encoder_emits_decodable_annex_b_with_sps_pps_and_keyframes():
    encoder = video.H264Encoder()
    decoder = video.H264Decoder()
    try:
        first = encoder.encode(_test_image())
        assert first and frame.is_h264_annexb(first)
        assert {7, 8, 5} <= _nal_types(first)  # SPS, PPS, IDR
        decoded = decoder.decode(first)
        assert decoded is not None
        assert (decoded.width(), decoded.height()) == (320, 240)

        delta = encoder.encode(_test_image())
        assert delta and 5 not in _nal_types(delta)
        decoded_delta = decoder.decode(delta)
        assert decoded_delta is not None
        assert (decoded_delta.width(), decoded_delta.height()) == (320, 240)

        encoder.request_keyframe()
        requested = encoder.encode(_test_image())
        assert requested and 5 in _nal_types(requested)
        assert decoder.decode(requested) is not None
    finally:
        decoder.close()
        encoder.close()


def test_video_wire_rejects_jpeg_and_oversized_access_units():
    with pytest.raises(frame.FrameError, match="Annex-B"):
        frame.outgoing_video(b"\xff\xd8\xff\xd9")
    with pytest.raises(frame.FrameError, match="limit"):
        frame.outgoing_video(b"\x00\x00\x01\x65" + b"x" * frame.MAX_FRAME_BYTES)


def test_media_websocket_url_uses_only_single_use_token():
    url = media_ws_url("https://127.0.0.1:3090/api", {
        "token": "token+/=safe", "ws_path": "/ws/calls/call-1/media",
    })
    assert url == "wss://127.0.0.1:3090/ws/calls/call-1/media?token=token%2B%2F%3Dsafe"


def test_media_net_delivers_access_units_and_keyframe_requests():
    net = MediaNet("call-1", lambda: "")
    videos, keyframes = [], []
    net.videoReceived.connect(videos.append)
    net.keyframeRequested.connect(lambda: keyframes.append(True))

    access_unit = b"\x00\x00\x00\x01\x65sample"
    net._on_binary(frame.encode_frame(frame.IN_VIDEO, b'{"participant_jid":"peer"}', access_unit))
    net._on_binary(frame.encode_frame(frame.KEYFRAME_REQUEST, b"", b'{"type":"keyframe_request"}'))

    assert videos == [access_unit]
    assert keyframes == [True]


def test_media_net_requests_token_connects_and_transfers_binary_frames():
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["calls"]).start()
    mock.state["active_call"] = {"id": "call-1", "status": "connected"}
    net = MediaNet("call-1", lambda: mock.url, retry_delay_ms=100)
    videos, keyframes, connections = [], [], []
    net.videoReceived.connect(videos.append)
    net.keyframeRequested.connect(lambda: keyframes.append(True))
    net.connectedChanged.connect(connections.append)
    access_unit = b"\x00\x00\x00\x01\x65encoded-frame"
    try:
        net.start()
        assert _spin_until(lambda: net.connected), mock.requests
        assert mock.wait_for_media_client()
        token_requests = [r for r in mock.requests if r[0] == "POST" and "/media-session" in r[1]]
        assert len(token_requests) == 1
        assert token_requests[0][1].startswith("/api/calls/call-1/media-session")
        assert token_requests[0][2] == b"{}"
        assert len(mock.media_handshakes) == 1
        path, token = mock.media_handshakes[0]
        assert path.endswith(f"?token={token}") and token.startswith("mock-media-")
        assert token not in mock.state["media_sessions"]

        mock.send_media_frame(frame.encode_frame(frame.IN_VIDEO, b"{}", access_unit))
        mock.send_media_frame(frame.encode_frame(frame.KEYFRAME_REQUEST, b"", b'{"type":"keyframe_request"}'))
        assert _spin_until(lambda: videos and keyframes)
        assert videos == [access_unit]
        assert keyframes == [True]

        pcm = bytes(frame.AUDIO_FRAME_BYTES)
        net.send_audio(pcm)
        net.send_video(access_unit)
        assert _spin_until(lambda: len(mock.media_frames) == 2)
        outgoing = [frame.decode_frame(data) for data in mock.media_frames]
        assert [(item.kind, item.payload) for item in outgoing] == [
            (frame.OUT_AUDIO, pcm), (frame.OUT_VIDEO, access_unit),
        ]

        mock.close_clients()
        assert _spin_until(lambda: net.stats["connects"] >= 2 and net.connected)
        assert mock.wait_for_media_client()
        assert len(mock.media_handshakes) == 2
        assert mock.media_handshakes[1][1] != token
        assert mock.state["media_session_count"] == 2
    finally:
        net.stop()
        _spin_until(lambda: not mock.media_clients, 1000)
        mock.stop()


def test_two_media_clients_exchange_audio_and_annex_b_video_frames():
    """Exercise independent QML media transports as two participants in one call."""
    app = QCoreApplication.instance() or QCoreApplication([])
    mock = MockBackend(areas=["calls"]).start()
    mock.state["active_call"] = {"id": "call-peer-test", "status": "connected"}
    left = MediaNet("call-peer-test", lambda: mock.url, retry_delay_ms=100)
    right = MediaNet("call-peer-test", lambda: mock.url, retry_delay_ms=100)
    received = {"left_audio": [], "left_video": [], "right_audio": [], "right_video": []}
    left.audioReceived.connect(received["left_audio"].append)
    left.videoReceived.connect(received["left_video"].append)
    right.audioReceived.connect(received["right_audio"].append)
    right.videoReceived.connect(received["right_video"].append)
    left_pcm, right_pcm = bytes([0x11]) * frame.AUDIO_FRAME_BYTES, bytes([0x22]) * frame.AUDIO_FRAME_BYTES
    left_video = b"\x00\x00\x00\x01\x65left-frame"
    right_video = b"\x00\x00\x01\x65right-frame"
    try:
        session_url = f"{mock.url}/calls/call-peer-test/media-session"
        with urlopen(session_url, data=b"{}") as response:
            reusable_token = json.load(response)["token"]
        with urlopen(session_url, data=b"{}") as response:
            assert json.load(response)["token"] == reusable_token

        left.start()
        assert _spin_until(lambda: left.connected), mock.requests
        assert mock.wait_for_media_clients(1)
        assert mock.media_handshakes[0][1] == reusable_token
        # The first WebSocket consumes its token. A later client gets a fresh
        # single-use token, matching CreateMediaSession's Go lifecycle.
        right.start()
        assert _spin_until(lambda: right.connected), mock.requests
        assert mock.wait_for_media_clients(2)
        assert len(mock.media_handshakes) == 2
        assert mock.media_handshakes[0][1] != mock.media_handshakes[1][1]

        left.send_audio(left_pcm)
        left.send_video(left_video)
        right.send_audio(right_pcm)
        right.send_video(right_video)
        assert _spin_until(lambda: all(len(received[k]) == 1 for k in received))
        assert received == {
            "left_audio": [right_pcm], "left_video": [right_video],
            "right_audio": [left_pcm], "right_video": [left_video],
        }
        assert mock.state["media_session_count"] == 2
        assert len(mock.media_frames) == 4
    finally:
        left.stop()
        right.stop()
        _spin_until(lambda: not mock.media_clients, 1000)
        mock.stop()
