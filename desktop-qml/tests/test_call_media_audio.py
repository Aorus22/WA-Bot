"""Pure call-audio framing and device-format conversion tests; no audio device is opened."""
from __future__ import annotations

from array import array

from PySide6.QtMultimedia import QAudioFormat

from bridge.call_media import dsp, frame
from bridge.call_media.audio import SF, _CaptureConverter, _PlaybackConverter


def _format(rate: int, channels: int, sample_format) -> QAudioFormat:
    fmt = QAudioFormat()
    fmt.setSampleRate(rate)
    fmt.setChannelCount(channels)
    fmt.setSampleFormat(sample_format)
    return fmt


def test_frame_chunker_emits_only_complete_wire_frames_and_clears_partial_data():
    chunker = dsp.FrameChunker()
    payload = frame.samples_to_pcm([1200] * (frame.AUDIO_FRAME_SAMPLES + 12))
    assert chunker.push(payload[:1000]) == []
    frames = chunker.push(payload[1000:])
    assert len(frames) == 1
    assert len(frames[0]) == frame.AUDIO_FRAME_BYTES
    assert chunker.pending == 12
    chunker.clear()
    assert chunker.pending == 0


def test_jitter_buffer_waits_for_two_frames_then_rebuffers_on_underrun():
    first = frame.samples_to_pcm([1000] * frame.AUDIO_FRAME_SAMPLES)
    second = frame.samples_to_pcm([2000] * frame.AUDIO_FRAME_SAMPLES)
    jitter = dsp.JitterBuffer()
    jitter.push(first)
    assert jitter.pull(frame.AUDIO_FRAME_SAMPLES) == bytes(frame.AUDIO_FRAME_BYTES)
    jitter.push(second)
    assert jitter.pull(frame.AUDIO_FRAME_SAMPLES) == first
    drained = jitter.pull(frame.AUDIO_FRAME_SAMPLES * 2)
    assert drained[:frame.AUDIO_FRAME_BYTES] == second
    assert drained[frame.AUDIO_FRAME_BYTES:] == bytes(frame.AUDIO_FRAME_BYTES)
    assert jitter.pull(frame.AUDIO_FRAME_SAMPLES) == bytes(frame.AUDIO_FRAME_BYTES)


def test_pcm_formats_mix_stereo_and_clamp_to_supported_range():
    stereo = dsp.encode_pcm([-0.5, 0.0, 0.5], "i16", 2)
    decoded = dsp.decode_pcm(stereo, "i16", 2)
    assert decoded == [-0.5, 0.0, 0.5]
    float_pcm = dsp.encode_pcm([-2.0, 0.25, 2.0], "f32", 1)
    assert dsp.decode_pcm(float_pcm, "f32", 1) == [-1.0, 0.25, 1.0]


def test_capture_converter_handles_split_stereo_device_chunks():
    converter = _CaptureConverter(_format(48_000, 2, SF.Int16), "i16")
    raw = array("h", [8192, 8192] * 4800).tobytes()
    split = len(raw) // 2 + 1  # split in the middle of an interleaved sample
    wire = converter.to_wire(raw[:split]) + converter.to_wire(raw[split:])
    samples = frame.pcm_to_samples(wire)
    assert len(samples) >= frame.AUDIO_FRAME_SAMPLES
    assert all(8000 < value < 8400 for value in samples[10:])  # ignore the sinc filter's startup transient
    chunker = dsp.FrameChunker()
    assert len(chunker.push(wire)) >= 1


def test_playback_converter_resamples_wire_audio_to_stereo_device_format():
    jitter = dsp.JitterBuffer()
    jitter.push(frame.samples_to_pcm([8192] * (frame.AUDIO_FRAME_SAMPLES * 2)))
    converter = _PlaybackConverter(_format(48_000, 2, SF.Float), "f32")
    output = converter.fill(jitter, 480, muted=False)
    decoded = dsp.decode_pcm(output, "f32", 2)
    assert len(decoded) == 480
    assert 0.24 < sum(decoded) / len(decoded) < 0.26
