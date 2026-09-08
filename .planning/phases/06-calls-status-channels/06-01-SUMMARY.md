---
phase: 06-calls-status-channels
plan: 01
title: Call State Machine, History Filter, and Global Call Overlay
status: passed
---

# Plan 06-01 Summary: Call State Machine & Overlay

## Accomplishments
- Implemented `CallManager` in `desktop-gpui/crates/app/src/state/call.rs`:
  - Lifecycle transitions: initiating -> ringing -> connected -> hangup with duration math.
  - Incoming call queue with accept/reject handlers (`accept_incoming`, `reject_incoming`).
  - Audio/mic mute controls and video upgrade workflow (`request_video_upgrade`, `accept_video_upgrade`).
  - History log filtering across direction, type, and status (`filtered_history`).
- Implemented `CallOverlayHelper` in `desktop-gpui/crates/app/src/components/call_overlay.rs`:
  - Duration formatting (`MM:SS`).
  - Global overlay activation condition check (`is_overlay_needed`).
- Unit tests (`test_call_lifecycle_and_history`, `test_incoming_call_and_video_upgrade`, `test_call_overlay_helper`) passing cleanly.

## Key Files
- `desktop-gpui/crates/app/src/state/call.rs`
- `desktop-gpui/crates/app/src/components/call_overlay.rs`
- `desktop-gpui/crates/app/src/state/mod.rs`
- `desktop-gpui/crates/app/src/components/mod.rs`
