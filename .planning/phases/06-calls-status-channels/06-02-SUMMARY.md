---
phase: 06-calls-status-channels
plan: 02
title: Status Story Tray, Status Posting, and Fullscreen Viewer
status: passed
---

# Plan 06-02 Summary: Status Stories & Viewer

## Accomplishments
- Implemented `StatusStore` in `desktop-gpui/crates/app/src/state/status.rs`:
  - Sender grouping (`StatusGroup`) and chronological status additions (`add_status_entry`).
  - Unviewed-first priority sorting (`sorted_groups`).
  - Live read-receipt / viewed update mechanism (`mark_as_viewed`).
- Implemented `StatusViewerState`:
  - Per-sender story progression and index management (`open`, `close`, `next`, `prev`).
  - Auto-advance timer modeling (5s duration).
- Unit tests (`test_status_grouping_and_sorting`, `test_status_viewer_navigation`) passing cleanly.

## Key Files
- `desktop-gpui/crates/app/src/state/status.rs`
- `desktop-gpui/crates/app/src/state/mod.rs`
