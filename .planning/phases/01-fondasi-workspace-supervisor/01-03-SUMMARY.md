# Phase 1 Plan 03: Settings + CI Matrix — Summary

**Phase:** 01-fondasi-workspace-supervisor | **Plan:** 03 | **Date:** 2026-09-08
**Requirements:** CORE-03, CORE-05
**Status:** complete

## Objective

Crate `wabot-settings` menyimpan preferensi UI di OS config dir (termasuk trio
kunci tema web) plus CI matrix Linux + Windows hijau dari fondasi.

## What was built

**`wabot-settings` crate** (`src/lib.rs`, `src/paths.rs`, `tests/persistence.rs`):

- `DesktopSettings` (serde JSON): `theme_preset` (default `"default-dark"`, same semantics as web `wa-bot-theme-preset`), `theme_mode: System|Light|Dark` (default `System`, same semantics as web `wa-bot-theme` / `ThemeProvider defaultTheme="system"`), `backend_path_override`, `last_backend_url`, `window_state`.
- `paths`: `dirs::config_dir` + **`wa-bot-desktop`** (not webterm-desktop), `with_base` injectable everywhere, `ensure_data_dirs` (`database/`+`media/`, paritas main.js).
- `load` missing → default; `save` atomic (temp+rename); corrupt (bad JSON **or** non-UTF8/unreadable) → timestamped `settings.json.corrupt-<unix>` backup + default + re-save, never panic.
- Validation without rejection: foreign `theme_preset` preserved (forward-compat), negative/zero window bounds → `None`, relative `backend_path_override` → `None`, unknown JSON fields tolerated (no `deny_unknown_fields`).
- UI-only boundary enforced in module doc-comment (AI keys, TTS, read-receipts stay in backend DB via `/api/settings`).
- No encryption-key custody (wa-bot has none — deliberate divergence from web-term reference).

**`.github/workflows/desktop-gpui.yml`**: paths-filtered triggers (never touches `docker-build.yml`), matrix `ubuntu-latest` + `windows-latest` (no macOS), stable toolchain + rust cache, Linux apt GUI deps (`libxkbcommon-x11-dev` etc. — the exact libs whose absence blocks local link, Assumption 01-01#1) + `mingw-w64`, `cargo build/test/clippy --workspace --locked` (`--locked` ×5, drift fails CI), Windows-target cross-check, `pin-audit` job rejecting loose pins.

## Verification (evidence)

- `cargo test -p wabot-settings`: **5/5 pass** (round-trip+restart, corrupt recovery+backup, unknown-field/preset compat, validation, dir layout).
- `cargo clippy -p wabot-settings --all-targets -- -D warnings`: **clean**.
- Workflow YAML valid, matrix `[ubuntu-latest, windows-latest]`, `--locked` present (5×).
- Beaver check: `git status` shows only intended files; no `target/` leakage (gitignored in 01-01).

## Assumptions

1. `theme_mode` mirrors `wa-bot-theme` (system/light/dark). Web's separate `wa-bot-theme-mode` preset-filter (`all/dark/light`) is intentionally not a second field — Phase 3 maps filtering from `theme_preset` + mode; recorded for Phase 3.
2. Negative window origins normalize to `None` per plan (multi-monitor negatives sacrificed for fail-safe restore; Phase 3 can refine).
3. No `0600` file mode (web-term has it for key custody; wa-bot settings hold no secrets).
4. Cross-check step scoped to `-p supervisor/settings/backend-client`: the GPUI app's Windows check needs the full MSVC/native env that only the `windows-latest` job provides (which builds the whole workspace natively).
5. CI not executed here (no `gh`/runner access from this box); workflow correctness is static (YAML parse + matrix assert + `--locked` grep).

## Deviations from Plan

- **[Rule 2 - Missing critical] Unreadable files (non-UTF8) treated as corrupt**, not propagated as `Io` error — `read_to_string` on garbage bytes would otherwise crash startup, violating the plan's own "file korup tidak merusak startup" must-have.
- **[Rule 1 - Bug] Test compared `load_from` result to `Default::default()`** but load sets `custom_base: Some(..)`; fixed expectation (test-only).
- No architectural changes (no Rule 4).

## Files

- Created: `desktop-gpui/crates/settings/src/lib.rs` (rewrote stub), `desktop-gpui/crates/settings/src/paths.rs` (rewrote stub), `desktop-gpui/crates/settings/tests/persistence.rs`, `.github/workflows/desktop-gpui.yml`
- Modified: `desktop-gpui/Cargo.lock` (settings deps resolved)

## Threat handling (T-01-03a/b/c)

Corrupt→backup+default, unknown-tolerant, bounds normalized; UI-only boundary documented, no secret fields; `--locked` + pin-audit against dep drift.
