"""Binary media frame codec — mirrors `internal/infrastructure/call/media_frame.go` and
`backend-client/src/media.rs`.

    byte 0      kind
    bytes 1-2   metadata length N, big-endian uint16
    bytes 3..   N bytes of UTF-8 JSON metadata (may be empty)
    rest        payload
"""
from __future__ import annotations

import json
import struct
import sys
from array import array
from dataclasses import dataclass

OUT_AUDIO = 0x01
OUT_VIDEO = 0x02
IN_AUDIO = 0x03
IN_VIDEO = 0x04
KEYFRAME_REQUEST = 0x05

AUDIO_RATE = 16_000
AUDIO_FRAME_SAMPLES = 960          # 60 ms of mono 16 kHz
AUDIO_FRAME_BYTES = AUDIO_FRAME_SAMPLES * 2
MAX_FRAME_BYTES = 1 << 20          # the server reads at most 1 MiB per message
MAX_METADATA = 0xFFFF


class FrameError(ValueError):
    pass


@dataclass(frozen=True)
class MediaFrame:
    kind: int
    metadata: bytes = b""
    payload: bytes = b""

    def encode(self) -> bytes:
        return encode_frame(self.kind, self.metadata, self.payload)

    def video_metadata(self) -> tuple[str, int]:
        """(participant_jid, orientation) of an incoming video frame; malformed -> ("", 0).
        Orientation is clockwise quarter turns, normalised to 0..3."""
        if not self.metadata:
            return "", 0
        try:
            meta = json.loads(self.metadata)
            if not isinstance(meta, dict):
                return "", 0
            jid = meta.get("participant_jid")
            turns = meta.get("orientation")
            return (jid if isinstance(jid, str) else ""), (int(turns) % 4 if isinstance(turns, (int, float)) else 0)
        except (ValueError, TypeError):
            return "", 0


def encode_frame(kind: int, metadata: bytes = b"", payload: bytes = b"") -> bytes:
    if len(metadata) > MAX_METADATA:
        raise FrameError("media frame metadata longer than 65535 bytes")
    return struct.pack(">BH", kind, len(metadata)) + bytes(metadata) + bytes(payload)


def decode_frame(data: bytes) -> MediaFrame:
    if len(data) < 3:
        raise FrameError("media frame too short")
    kind, meta_len = struct.unpack(">BH", data[:3])
    if 3 + meta_len > len(data):
        raise FrameError("media frame metadata length exceeds the frame")
    return MediaFrame(kind, bytes(data[3:3 + meta_len]), bytes(data[3 + meta_len:]))


def outgoing_audio(pcm_s16le: bytes) -> bytes:
    """A client->server audio frame; the payload must be exactly 960 samples."""
    if len(pcm_s16le) != AUDIO_FRAME_BYTES:
        raise FrameError("audio frames must be exactly 960 samples (1920 bytes)")
    return encode_frame(OUT_AUDIO, b"", pcm_s16le)


def outgoing_video(jpeg: bytes) -> bytes:
    return encode_frame(OUT_VIDEO, b"", jpeg)


def is_jpeg(data: bytes) -> bool:
    return len(data) >= 4 and data[0] == 0xFF and data[1] == 0xD8


# --- PCM helpers (little-endian signed 16 bit) ----------------------------------------
def pcm_to_samples(pcm: bytes) -> array:
    """Wire bytes -> array('h'). A trailing odd byte is ignored."""
    a = array("h")
    a.frombytes(bytes(pcm[: len(pcm) & ~1]))
    if sys.byteorder == "big":
        a.byteswap()
    return a


def samples_to_pcm(samples) -> bytes:
    a = samples if isinstance(samples, array) and samples.typecode == "h" else array("h", samples)
    if sys.byteorder == "big":
        a = array("h", a)
        a.byteswap()
    return a.tobytes()
