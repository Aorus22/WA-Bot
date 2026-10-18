"""Deterministic microphone PCM conversion tests; no audio device is required."""
from __future__ import annotations

import io
import struct
import wave

from bridge.composer_voice import PcmAccumulator


def _wav_samples(data: bytes) -> list[int]:
    with wave.open(io.BytesIO(data), "rb") as wav:
        assert wav.getnchannels() == 1
        assert wav.getsampwidth() == 2
        return list(struct.unpack("<" + "h" * wav.getnframes(), wav.readframes(wav.getnframes())))


def test_accumulator_preserves_partial_pcm_chunks_and_writes_canonical_wav():
    source = struct.pack("<hhhh", 1000, -2000, 3000, -4000)
    acc = PcmAccumulator(16_000)
    for chunk in (source[:1], source[1:4], source[4:7], source[7:]):
        acc.feed(chunk)

    result = acc.finish()

    assert result["seconds"] == 4 / 16_000
    assert result["peak"] == 3999
    assert _wav_samples(result["wav"]) == [999, -1999, 2999, -3999]


def test_accumulator_downmixes_stereo_and_resamples_48khz_in_fragmented_input():
    stereo_frames = [
        (1000, 3000), (2000, 4000), (3000, 5000),
        (5000, 7000), (6000, 8000), (7000, 9000),
    ]
    source = struct.pack("<" + "h" * (2 * len(stereo_frames)), *(v for row in stereo_frames for v in row))
    acc = PcmAccumulator(48_000, channels=2)
    for chunk in (source[:5], source[5:14], source[14:21], source[21:]):
        acc.feed(chunk)

    result = acc.finish()

    assert result["seconds"] == 2 / 16_000
    assert result["peak"] == 6999
    assert _wav_samples(result["wav"]) == [2999, 6999]
