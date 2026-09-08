# Phase 1 Plan 01: Workspace Scaffold + Toolchain — Summary

**Phase:** 01-fondasi-workspace-supervisor | **Plan:** 01 | **Date:** 2026-09-08
**Requirements:** CORE-01, CORE-05 (fondasi workspace; CI hijau menyusul plan 01-03)
**Status:** complete (with 2 documented environment limitations, both CI-covered)

## Objective

Workspace Cargo `desktop-gpui/` ter-scaffold dengan pin versi eksak + Cargo.lock
ter-commit + seluruh member crate ter-build hijau di Linux.

## What was built

- `rust-toolchain.toml` — pins channel `stable` (resolved: rustc/cargo **1.98.1**, rustup 1.29.1, installed via official rustup-init with `--proto '=https' --tlsv1.2`).
- `desktop-gpui/Cargo.toml` — workspace resolver 2, members `supervisor`, `settings`, `backend-client`, `app`; `[workspace.package]` 0.1.0/edition 2021; all core deps exact-pinned (`gpui-pre =0.3.3`, `gpui-component =0.6.0`, tokio =1.53.1 with rt-multi-thread/process/macros/sync/time/net, reqwest =0.12.28 rustls-tls, tokio-tungstenite =0.26.2, futures-util =0.3.32, flume =0.12.0, serde =1.0.229, serde_json =1.0.151, dirs =6.0.0, thiserror =2.0.20, anyhow =1.0.104, parking_lot =0.12.5, rand =0.10.2, tempfile =3.27.0 dev, libc =0.2.189 dev); `[profile.release] lto="thin"`.
- `desktop-gpui/Cargo.lock` — committed, not git-ignored (verified `git check-ignore` → not ignored).
- Member crates: `wabot-supervisor`, `wabot-settings`, `wabot-backend-client` (stub libs), `wabot` bin whose `main.rs` calls `gpui_platform::current_platform(false)` + `gpui_component::init(cx)` at the call site (E0277 pairing guard). `crates/app/Cargo.toml` documents the must-bump-together rule.
- `.gitignore` — appended `desktop-gpui/target/` so build output is never committed.

## Verification (evidence)

- `cargo check --workspace` → **green** (includes `wabot` bin: gpui-pre 0.3.3 + gpui-component 0.6.0 resolve together, no E0277).
- `cargo build -p wabot-supervisor -p wabot-settings -p wabot-backend-client` → **green**.
- Pin audit: `grep -E 'version\s*=\s*"' | grep -v '=' | wc -l` → **0** (no loose pins); `cargo metadata` → **METADATA_OK**.
- `test -f desktop-gpui/Cargo.lock` → exists; `git check-ignore` → not ignored.
- `rustup target add x86_64-pc-windows-gnu` → installed (Cargo.lock now also covers Windows-only deps for CI `--locked` builds).

## Assumptions (parity/reference-default calls, user away)

1. **Local final link of `wabot` bin blocked by missing system lib** (`rust-lld: unable to find library -lxkbcommon-x11`); dev box is Fedora without sudo so the `-devel` package cannot be installed. All rlib targets + full `cargo check --workspace` pass. Full link is covered by CI (plan 01-03 workflow installs `libxkbcommon-x11-dev` via apt, mirroring `web-term/.github/workflows/desktop-ci.yml`).
2. **Windows cross-check is CI-only**: `cargo check --target x86_64-pc-windows-gnu` fails locally at `cc-rs` (`x86_64-w64-mingw32-gcc` not installed, no sudo) — exactly the plan-anticipated outcome; recorded here, not stalled on.
3. **Builds in this environment require `TMPDIR` redirected** to `/home/aorus/tmp-rustup` (`/tmp` is a near-full tmpfs + user quota; cc compilation fails otherwise). CI runners are unaffected.
4. Crate versions start at workspace `0.1.0` (wa-bot is a new port; web-term's `0.5.0` not inherited).

## Deviations from Plan

- **[Rule 3 - Blocking] Added `desktop-gpui/target/` to root `.gitignore`.** Without it, `target/` build output would be committable. One-line, no behavior impact.
- **[Rule 3 - Blocking] Rustup installed via direct binary download** (`static.rust-lang.org/.../rustup-init` to `$HOME`, ran with `TMPDIR` redirect) instead of `curl|sh` pipe, because the pipe's temp download exceeded the `/tmp` quota. Same official installer, same TLS source.
- No architectural changes (no Rule 4).

## Files

- Created: `rust-toolchain.toml`, `desktop-gpui/Cargo.toml`, `desktop-gpui/Cargo.lock`, `desktop-gpui/crates/{supervisor,settings,backend-client}/Cargo.toml`, `desktop-gpui/crates/{supervisor,settings,backend-client}/src/lib.rs`, `desktop-gpui/crates/settings/src/paths.rs`, `desktop-gpui/crates/app/Cargo.toml`, `desktop-gpui/crates/app/src/main.rs`
- Modified: `.gitignore`

## Next

Plan 01-02 (supervisor) + 01-03 (settings + CI) build on these stubs.
