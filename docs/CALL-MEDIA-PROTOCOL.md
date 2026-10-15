# Call media protocol (client ⇄ backend)

Live voice/video calls are bridged to WhatsApp by the backend (meowcaller). A
client (web/Electron or the GPUI desktop app) attaches its microphone and camera
to the active call over one binary WebSocket and plays back what the other
party sends. Call *control* (answer, reject, hang up, start/accept/stop video)
stays on the REST API and the JSON `/ws` hub (`call.*` events).

## 1. Getting connected

1. Place or receive a call through the normal REST/`call.*` flow. Media only
   works while that call is the active one.
2. `POST /api/calls/{id}/media-session` (same auth as the rest of `/api`,
   i.e. `X-API-Secret` where the client already sends it). Response:

   ```json
   { "token": "…", "expires_at": 1760000000000, "ws_path": "/ws/calls/call_123/media" }
   ```

   The token is single-use and expires after 120 s. Request a new one for every
   (re)connect.
3. Open `ws(s)://<backend host>{ws_path}?token=<token>&video=jpeg` (binary
   messages). No other auth is needed on the WebSocket. `video=jpeg` selects the
   JPEG video format below; always send it.
4. Connect once the call is answered (status `connected`/`active`) or earlier;
   audio you send before the call is connected is discarded. Close the socket
   when the call ends (the server also closes it). A dropped media socket does
   **not** hang up the call — reconnect with a new token.

Errors on connect: `401 media session expired`, `403 invalid media session`,
`409 call not active`.

## 2. Frame format

Every WebSocket message is binary:

```
byte 0      kind
bytes 1-2   metadata length N, big-endian uint16
bytes 3..   N bytes of UTF-8 JSON metadata (may be empty)
rest        payload
```

| kind | direction | payload |
|------|-----------|---------|
| `0x01` | client → server | outgoing audio |
| `0x02` | client → server | outgoing video (one JPEG) |
| `0x03` | server → client | incoming audio |
| `0x04` | server → client | incoming video (one JPEG) |
| `0x05` | server → client | keyframe request — ignore in JPEG mode |

### Audio (0x01 / 0x03)

* Signed 16-bit little-endian PCM, **mono, 16 000 Hz**.
* Exactly **960 samples (1920 bytes) = 60 ms** per frame. Send one frame every
  60 ms in real time, no faster (the server drops the oldest frame past a
  3-frame backlog and inserts silence when nothing arrived in time).
* Incoming frames have the same format and arrive in real time; keep a small
  jitter buffer (~2–3 frames) on playback, and drop old audio if the buffer
  grows beyond ~200 ms.
* Muting: just stop sending frames (or send zeros). Also keep using the REST
  API if a mute notification to the peer is wanted (not required for audio).

### Video (0x02 / 0x04)

* Payload is one complete baseline JPEG image per message (`FF D8 … FF D9`).
  The backend does the H.264 encode/decode with ffmpeg.
* Outgoing: ≤ 640×480 (keep aspect ratio), ~15 fps, JPEG quality ≈ 0.6, a
  frame well under 100 KB. Only send while the call has video enabled
  (`call.video_state` / call `video_enabled`).
* Incoming metadata JSON: `{"participant_jid":"", "orientation":0}` where
  `orientation` is clockwise quarter turns (0‥3) to apply to the decoded
  picture so it shows upright (phones held in portrait). The picture size can
  change mid-call.

## 3. Reference

* Server side: `internal/delivery/http/call_media_websocket.go`,
  `internal/infrastructure/call/media_session.go`, `audio_bridge.go`,
  `video_transcode.go`.
* Frame codec: `internal/infrastructure/call/media_frame.go`.
