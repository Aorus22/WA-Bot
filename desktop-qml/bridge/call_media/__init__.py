"""Live call media for the QML client: microphone, speaker and camera bridged to the backend
over the binary media WebSocket documented by the Go call-media implementation.

    frame       wire codec (kind | u16 BE meta length | JSON meta | payload)
    dsp         pure audio helpers: resampler, 960-sample chunker, playback jitter buffer
    net         token request + QWebSocket loop with reconnect/backoff
    audio       QtMultimedia microphone/speaker (QAudioSource/QAudioSink) + device selection
    video       camera capture (QCamera) and GStreamer/OpenH264 Annex-B codec
    session     everything for one call, running on its own QThread
    controller  follows the call state and exposes pictures/errors to `Calls`
    permissions OS microphone/camera permission handling (macOS/Windows; always granted on Linux)

All heavy work (resampling, H.264 encode/decode, socket I/O, audio pacing) runs off the UI thread.
"""
