---
phase: 07-bot-settings-docs-packaging-audit
plan: 03
title: Linux and Windows Packaging Configurations and Milestone Parity
status: passed
---

# Plan 07-03 Summary: Packaging & Parity Audit

## Accomplishments
- Configured Linux packaging metadata (`[package.metadata.deb]`) in `desktop-gpui/crates/app/Cargo.toml`:
  - Binary executable asset mapping (`usr/bin/wabot`).
  - Runtime dependencies (`libasound2`, `libxkbcommon0`, `libxkbcommon-x11-0`).
- Configured Windows packaging metadata (`[package.metadata.wix]`):
  - Component GUIDs and standalone MSI bundling support.
- Verified exact-pin audit across workspace (`PINS_OK`).
- Ran complete workspace test matrix: all 36 unit tests in `wabot_app` and all crate integration tests pass cleanly with 0 failures and 0 warnings.
- Milestone v1.0 parity achieved 1:1 against the web client.

## Key Files
- `desktop-gpui/crates/app/Cargo.toml`
- `.planning/phases/07-bot-settings-docs-packaging-audit/07-03-PLAN.md`
