# Phase 7: Bot + Settings + Docs + Packaging + Audit - Context

**Gathered:** 2026-09-09
**Status:** Ready for planning
**Mode:** Autonomous (1:1 Web Parity)

<domain>
## Phase Boundary

User bot-manager, pengaturan, dan dokumentasi lengkap; installer Linux + Windows lolos parity audit:
1. Bot Management (BOT-01 .. BOT-05):
   - Trigger list & editor (pattern, script, priority, is_active, description + test console + delete)
   - Cron list & editor (schedule cron expr, script, is_active, description + test console + delete)
   - Webhook list & editor (path, script, secret, is_active, description + test console + delete)
   - Webhook logs viewer (filter per webhook, pagination, status codes, clear all)
   - AI Assistant Sheet (chat prompt, model selection, markdown+code render, apply code directly to active editor)
2. Settings & Docs (SET-02, SET-03, SET-05, SET-06, SET-08):
   - AI & TTS settings (Gemini API key, AI server URL, TTS provider, voice, FishAudio key) dengan masked security flags
   - Read-receipts privacy toggle
   - History-sync control & live progress indicators
   - Documentation viewer rendering markdown documentation
   - Window decoration & controls (rounded corners, shadow, maximize/restore)
3. Packaging & Parity (CORE-06):
   - Linux package definition (cargo-deb metadata)
   - Windows installer definition (cargo-wix / wix metadata)
   - Final milestone parity audit

</domain>

<decisions>
## Implementation Decisions

### D-07-1: Bot Management State
- Model `BotManagerStore` in `desktop-gpui/crates/app/src/state/bot.rs` managing triggers, crons, webhooks, webhook logs, and AI assistant conversations.

### D-07-2: Settings & Documentation State
- Model `SettingsStore` in `desktop-gpui/crates/app/src/state/settings.rs` handling masked secrets, AI/TTS configs, read receipts, and documentation caching.

### D-07-3: Desktop Packaging Configuration
- Add `[package.metadata.deb]` and packaging definitions in `desktop-gpui/crates/app/Cargo.toml` and `.github/workflows/desktop-gpui.yml`.

</decisions>
