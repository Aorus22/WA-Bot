# Phase 1 Plan 02: Supervisor Sidecar — Summary

**Phase:** 01-fondasi-workspace-supervisor | **Plan:** 02 | **Date:** 2026-09-08
**Requirements:** CORE-02
**Status:** complete

## Objective

Crate `wabot-supervisor` men-spawn Go backend sebagai sidecar, handshake port,
readiness probe, dan kill bersih tanpa orphan — port eksak `desktop/main.js`.

## What was built

`desktop-gpui/crates/supervisor/src/lib.rs` (tokio-only, no GPUI) + `tests/lifecycle.rs`:

- **SpawnOptions** `{backend_path, db_path, media_path (absolute-enforced), allowed_origins="*", args, work_dir, readiness/handshake timeouts 15s}`. `env_vars()` returns exactly the 5 main.js pairs (`PORT=:0`, `ALLOWED_ORIGINS=*`, `DB_PATH`, `MEDIA_PATH`, `TZ=Asia/Jakarta`) — explicitly no encryption key (wa-bot has none).
- **Handshake**: `parse_handshake_line` (noise-tolerant, first-match); stdout drained for the child's whole lifetime in a background task (prevents SIGPIPE death — found via live test, Rule 1).
- **Supervisor**: `spawn` (kill_on_drop, stderr newest-wins 2000-char ring, early-exit detection) → `BackendStatus` watch channel (`Starting/Ready/Failed/Crashed`) → `stop` (SIGTERM+grace unix, kill Windows) + `kill_force` (crash simulation) + `wait_ready` (reusable for `--no-backend` mode) + `adopt_or_clear` (single-instance adopt).
- **CLI parity**: `has_cli_flag` (`--name`/`-name`), `get_cli_value` (`=` and space forms), `parse_backend_port` (`None`→8080, digits 1-65535, else `Err` — never silent default), `ExternalBackend{port}`.
- **Paths**: `packaged_backend_path` (`resources/be/wa-bot-backend[.exe]`), `dev_backend_path` (`<repo>/wa-bot-backend[.exe]`), two-level `resolve_backend_path`; `ensure_data_dirs` (`database/`+`media/`, idempotent).
- Startup kill policy documented on `Supervisor`: adopt-if-serving, kill-only-own-child (ephemeral ports can't collide).

## Verification (evidence)

- `cargo test -p wabot-supervisor`: **10/10 pass** (5 unit + 5 integration), incl. tracer spawn→handshake→ready→stop→no-orphan vs python fake backend (ephemeral bind(0), HTTP 200 probe), crash-restart respawn, invalid-path Failed-with-tail, data-dirs idempotency, **live backend** (`go build ./cmd/api` → real spawn → Ready → stop → no orphan).
- `cargo clippy -p wabot-supervisor --all-targets -- -D warnings`: **clean**.
- `Cargo.toml`: moved `libc` from dev-deps to `[target.'cfg(unix)'.dependencies]` (needed by `stop()`; dev-deps don't link into the lib — Rule 1 fix).

## Assumptions

1. Fake backend uses the system `python3`/`python` (present here; ubuntu/windows CI runners both ship Python). Absent → tests skip with reason, still pass.
2. `work_dir` semantics follow main.js/desktop-gtk exactly (cwd=userData base).

## Deviations from Plan

- **[Rule 2 - Missing critical] Added `SpawnOptions::work_dir` + `cmd.current_dir`.** The Go backend opens `database/*.db` relative to cwd and ignores `DB_PATH` for open (verified in `cmd/api/main.go:124` + `desktop-gtk/.../manager.go:70-71`); without this the real backend can never start. Env paths stay absolute (plan's cwd-independence preserved).
- **[Rule 1 - Bug] Stdout drain task after handshake.** Dropping the stdout read end SIGPIPE-killed the Go backend mid-readiness (observed: `signal 13`). Reference web-term code has the same latent shape but its backend logs less.
- **[Rule 1 - Bug] Spawn-failure tail**: `Failed` now carries `Execution error: {io}` as stderr_tail when no child exists (test asserts non-empty tail).
- **[Rule 1 - Bug] Test-only fixes**: `has_cli_flag("port")` assertion corrected (flag present with value — matches main.js), live-test repo-root depth `nth(2)`→`nth(3)`, removed `mut` on watch receiver.
- No architectural changes (no Rule 4).

## Files

- Created: `desktop-gpui/crates/supervisor/src/lib.rs` (rewrote stub), `desktop-gpui/crates/supervisor/tests/lifecycle.rs`
- Modified: `desktop-gpui/crates/supervisor/Cargo.toml`, `desktop-gpui/Cargo.lock` (libc unix-dep)

## Threat handling (T-01-02a/b/c)

Secrets env-only (no key exists at all); kill_on_drop + explicit stop + adopt-or-kill + restart test; stderr ring metadata-only, env never logged.
