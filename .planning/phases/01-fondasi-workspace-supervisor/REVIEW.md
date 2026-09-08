---
phase: 01-fondasi-workspace-supervisor
reviewed: 2026-09-08T00:00:00Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - desktop-gpui/crates/supervisor/src/lib.rs
  - desktop-gpui/crates/supervisor/tests/lifecycle.rs
  - desktop-gpui/crates/supervisor/Cargo.toml
  - desktop-gpui/crates/settings/src/lib.rs
  - desktop-gpui/crates/settings/src/paths.rs
  - desktop-gpui/crates/settings/tests/persistence.rs
  - desktop-gpui/crates/settings/Cargo.toml
  - desktop-gpui/crates/backend-client/src/lib.rs
  - desktop-gpui/crates/app/src/main.rs
  - desktop-gpui/crates/app/Cargo.toml
  - desktop-gpui/Cargo.toml
  - rust-toolchain.toml
  - .github/workflows/desktop-gpui.yml
findings:
  critical: 2
  warning: 11
  info: 3
  total: 16
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-09-08T00:00:00Z
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

Reviewed all new Phase 1 source (supervisor, settings, backend-client stub, app stub, workspace manifests, CI workflow) against the focus areas: process-kill correctness, handshake parsing, path resolution, settings atomicity/permissions, CI correctness.

Two Blockers: (1) the post-handshake stdout drain task busy-spins forever once the child exits — a guaranteed CPU peg + task leak on every backend shutdown; (2) `load_from` treats *any* read error (including permission-denied) as corruption and destructively renames the user's settings file away. Eleven warnings follow, led by zombie leaks on kill-without-wait paths, a SIGTERM-to-recycled-PID race, and a CI pin-audit that cannot see shorthand pins. Deliberate design calls were respected, not flagged: no-0600 settings mode (no secrets by design), `work_dir` requirement, ephemeral-port policy.

## Critical Issues

### CR-01: Stdout drain task busy-loops forever after child exit

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:478-481`
**Issue:** After a successful handshake the drain task runs `while reader.next_line().await.is_ok() {}`. `next_line()` returns `Ok(None)` at EOF — which `is_ok()` treats as "keep going". Once the backend exits, every subsequent `next_line()` resolves to `Ok(None)` immediately, so this becomes a tight infinite loop: one tokio worker thread pegged at 100% CPU plus a leaked task per backend incarnation, accumulating across restarts. The pre-handshake reader (line 467, `while let Ok(Some(line))`) handles EOF correctly, so this is an inconsistency, not a pattern.
**Fix:**
```rust
tokio::spawn(async move {
    let mut reader = reader;
    while let Ok(Some(_)) = reader.next_line().await {}
});
```

### CR-02: Unreadable settings file (e.g. permission-denied) treated as corrupt and renamed away

**File:** `desktop-gpui/crates/settings/src/lib.rs:121-134`
**Issue:** `Err(_) =>` on `fs::read_to_string` collapses every IO failure — including `PermissionDenied`, which says nothing about file content — into the corrupt-recovery path, which *renames the user's file* to a `.corrupt-*` backup and overwrites it with defaults. Read permission (file) and write permission (directory rename) are independent, so a `mode 000` settings file in a writable config dir is destroyed and replaced with defaults while `load_from` returns `Ok`, i.e. the caller believes defaults were legitimately loaded. Corruption must be diagnosed from content, never from an unreadable file.
**Fix:**
```rust
let content = match fs::read_to_string(&file_path) {
    Ok(c) => c,
    Err(e) if e.kind() == std::io::ErrorKind::InvalidData => { /* corrupt path */ }
    Err(e) => return Err(SettingsError::Io(e)),
};
```
(`read_to_string` surfaces non-UTF8 as `InvalidData`; permission errors propagate as `Io` instead of triggering destructive recovery.)

## Warnings

### WR-01: `kill()` without `wait()` on failure/stop paths leaks zombies

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:483-494`, `531-537`, `611-616`
**Issue:** Three paths kill the child and then drop the `Child` handle without reaping: handshake-missing (484), handshake-timeout (488), readiness-timeout (532), and `stop()`'s grace-expiry fallback (613-615, `child.kill().await` with no `wait()` after). `kill_on_drop(true)` re-sends the signal on drop but never reaps, so each of these leaves a zombie PID entry until the desktop process exits. The port-based orphan test cannot see zombies (the port is released), which is why tests pass despite the leak. `kill_force` (587-588) shows the correct kill-then-wait shape.
**Fix:** After every `child.kill().await`, add `let _ = child.wait().await;` before the handle is dropped/returned from.

### WR-02: `stop()` sends SIGTERM to a possibly-recycled PID

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:601-608`
**Issue:** `stop()` reads `child.id()` and signals it via raw `libc::kill` without first checking whether the child already exited. The exit-monitor task reaps via `try_wait()` (554), which frees the PID for kernel reuse; a later `stop()` can then SIGTERM a foreign process that recycled the PID. The error return of `libc::kill` is also discarded (`unsafe { libc::kill(...); }`).
**Fix:**
```rust
if child.try_wait()?.is_none() {
    if let Some(pid) = child.id() {
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    }
}
```

### WR-03: `adopt_or_clear` fabricates `port: 0` when the URL has no parseable port

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:631-644`
**Issue:** Port extraction uses `.parse::<u16>().unwrap_or(0)` — a URL without a port (e.g. `last_backend_url` hand-edited to `http://localhost`) yields `Some(BackendInfo { port: 0, pid: 0, .. })`, a bogus "adopted" backend that fails confusingly downstream instead of falling through to fresh spawn. Related inconsistency: adopt requires `is_success()` (2xx) while `spawn`/`wait_ready` accept anything `< 500` (2xx-4xx) — the same backend can be adopted by one path and rejected by another.
**Fix:** Return `None` when the port parse fails (`.ok()?` instead of `.unwrap_or(0)`), and use one readiness predicate everywhere.

### WR-04: `stop()` leaves `status()` as `Ready` after shutdown

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:598-620`
**Issue:** `stop()` clears `info` but never touches the watch channel, so after a clean shutdown `status()` still reports `BackendStatus::Ready` while `info()` is `None` — contradictory observable state for Phase 3 boot logic (`Ready` with no backend). There is no `Stopped` variant, so a stopped supervisor is indistinguishable from a running one via the status channel.
**Fix:** Add a terminal status update on stop (e.g. send `BackendStatus::Crashed { exit_code: None }` is wrong semantically — prefer a `Stopped` variant, or at minimum document that `info().is_none()` is the post-stop source of truth).

### WR-05: Handshake parser accepts port 0, wasting a full readiness timeout

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:203-211`
**Issue:** `parse_handshake_line("BACKEND_PORT:0")` returns `Some(0)` (unit test at line 662 locks this in). Port 0 is never a connectable backend; `spawn` then builds `http://127.0.0.1:0` and burns the entire 15s readiness timeout before failing, with a misleading timeout error instead of an immediate handshake rejection.
**Fix:**
```rust
let port: u16 = digits.parse().ok()?;
if port == 0 { return None; }
Some(port)
```

### WR-06: `work_dir` accepts relative/nonexistent paths while db/media must be absolute

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:143-146`, `176-178`
**Issue:** `SpawnOptions::new` rightly rejects relative `db_path`/`media_path` as cwd-dependent, but `with_work_dir` takes any path unchecked — a relative `work_dir` makes `current_dir` resolve against the parent cwd, silently changing which `database/*.db` the Go backend opens (the exact cwd-sensitivity this phase documented as critical). A nonexistent dir surfaces only later as a spawn `Failed`, one step removed from the misconfiguration.
**Fix:** Validate in `with_work_dir` (or at top of `spawn`): reject relative paths with `InvalidPath`, and optionally `fs::exists` check for early error.

### WR-07: Unknown `theme_mode` value (or mistyped field) wipes the whole settings file

**File:** `desktop-gpui/crates/settings/src/lib.rs:30-37`, `135-152`
**Issue:** `ThemeMode` has no fallback variant, so a forward-compat value (e.g. a future `"auto"` from web) makes `serde_json::from_str` fail and the entire file is renamed to `.corrupt-*` and replaced with defaults — all user prefs destroyed because of one unknown string. This contradicts the stated forward-compat goal (foreign `theme_preset` passes through, but a foreign `theme_mode` nukes everything). Same fate for any type-level drift such as `"width": "800"`.
**Fix:** Add a forward-compat fallback:
```rust
#[derive(...)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
    #[serde(other)]
    Unknown,
}
```
(plus decide round-trip serialization for `Unknown`), or use `#[serde(default)]`-tolerant field-level handling.

### WR-08: Duplicated `ensure_data_dirs` in two crates

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:306-312`, `desktop-gpui/crates/settings/src/paths.rs:47-53`
**Issue:** Byte-identical `ensure_data_dirs(base)` (`database/` + `media/`) lives in both `wabot-supervisor` and `wabot-settings::paths`. Any layout change (permissions, a third dir) must be made twice; drift means the supervisor and settings disagree on the userData layout.
**Fix:** Keep one canonical implementation (e.g. in `wabot-settings::paths`) and re-export or depend on it from `wabot-supervisor`.

### WR-09: CI pin-audit is blind to shorthand pins

**File:** `.github/workflows/desktop-gpui.yml:69-75`
**Issue:** The audit greps only `version\s*=\s*"..."` lines, but the workspace uses shorthand form in two places: `gpui-component = "=0.6.0"` (`desktop-gpui/Cargo.toml:26`) and `gpui-platform = { package = "gpui-pre-platform", version = "=0.3.3" }` is fine — however `crates/app/Cargo.toml:18` (`gpui-platform ... version = "=0.3.3"`) IS covered while shorthand `gpui-component = "=0.6.0"` and any future `dep = "x.y.z"` loosening pass the audit silently. Loosening `gpui-component = "0.6.0"` would sail through CI despite the exact-pin policy.
**Fix:** Extend the audit to also match shorthand pins, e.g. `grep -E '^\s*[a-z0-9_-]+\s*=\s*"' desktop-gpui/Cargo.toml desktop-gpui/crates/*/Cargo.toml | grep -v '='`, or audit `cargo metadata` / `cargo tree` output instead of grep.

### WR-10: Config-dir fallback to `"."` silently makes settings cwd-dependent

**File:** `desktop-gpui/crates/settings/src/paths.rs:8-10`
**Issue:** When `dirs::config_dir()` returns `None`, `default_base_dir()` falls back to the relative path `"."`, so all subsequent reads/writes land in whatever the process cwd happens to be — settings silently scatter per launch context with no log or error. Rare, but exactly the cwd-dependence this phase eliminated elsewhere.
**Fix:** Return `Result`/`Option` from `default_base_dir` (or fall back to a deterministic absolute dir such as `std::env::current_dir()` joined at call time) and surface the degraded state to the caller.

### WR-11: Third-party CI actions pinned to mutable refs

**File:** `.github/workflows/desktop-gpui.yml:28`, `31`, `37`
**Issue:** `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2` track movable tags — a compromised or breaking upstream tag change silently alters the build environment. `dtolnay/rust-toolchain@stable` additionally floats the toolchain independently of `rust-toolchain.toml`.
**Fix:** Pin each action to a full commit SHA with a tag comment (e.g. `actions/checkout@<sha> # v4`), and pass an explicit `toolchain:` version matching the pinned channel.

## Info

### IN-01: Corrupt-backup timestamps collide within the same second

**File:** `desktop-gpui/crates/settings/src/lib.rs:192-198`
**Issue:** Backup name uses whole-second unix time; two corruptions in the same second make `fs::rename` overwrite the first backup, losing the earlier evidence. Low likelihood, no behavior impact today.
**Fix:** Append process id or a counter (`format!("json.corrupt-{ts}-{pid}")`), or use `create_new` semantics and retry with an incremented suffix.

### IN-02: CI jobs have no `timeout-minutes`

**File:** `.github/workflows/desktop-gpui.yml:18-60`
**Issue:** Neither `build-test` nor `pin-audit` sets `timeout-minutes`; a hung GUI build or test (the supervisor lifecycle tests spawn real processes) burns runner minutes up to the 6h default.
**Fix:** Add `timeout-minutes: 30` (generous for GPUI link + tests) to both jobs.

### IN-03: Double `spawn()` on one `Supervisor` kills the first child via drop, unreaped

**File:** `desktop-gpui/crates/supervisor/src/lib.rs:418-544`
**Issue:** Calling `spawn` twice replaces `self.child` (`*self.child.lock() = Some(child)` at line 544); the first `Child` is dropped and SIGKILLed via `kill_on_drop` without `wait()` (WR-01 class) and without any status transition — abrupt, unreaped, and silent. Callers are expected to `stop()` first, but nothing enforces it.
**Fix:** Return an error (or implicitly `stop()` first) when `self.child` is already `Some`.

---

_Reviewed: 2026-09-08T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_

Notes (deliberate calls, not flagged): no-0600 settings file mode is accepted — settings hold no secrets by design (T-01-03b); ephemeral-port-only spawn policy and adopt-if-serving semantics match the main.js contract; test-only Python/Go skips are hermetic and reasonable.
