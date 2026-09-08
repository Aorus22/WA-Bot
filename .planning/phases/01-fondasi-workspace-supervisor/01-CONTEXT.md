# Phase 1: Fondasi Workspace & Supervisor - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — smart discuss skipped per autonomous mode; user waived questions: pure 1:1 port, no new features)

<domain>
## Phase Boundary

Workspace Cargo `desktop-gpui/` ter-build reproduksibel di Linux + Windows dengan sidecar Go yang tersupervisi bersih. Meliputi: workspace + pin eksak + Cargo.lock (CORE-01), crate `supervisor` spawn/handshake/kill backend Go (CORE-02), crate `settings` preferensi UI di OS config dir (CORE-03), CI matrix Linux + Windows hijau dari fondasi (CORE-05).

</domain>

<decisions>
## Implementation Decisions

### the agent's Discretion
All implementation choices are at the agent's discretion — pure infrastructure phase. Guiding constraints (locked from milestone scope, not grey areas):
- Ikuti pola `../web-term/desktop-gpui`: workspace members supervisor/settings/backend-client/app, gpui-pre 0.3.3 + gpui-component 0.6.0 berpasangan, semua dep di-pin eksak, Cargo.lock ter-commit, reqwest rustls-tls (tanpa OpenSSL system dep), tokio rt-multi-thread/process/macros/sync/time/net
- Supervisor mem-port `desktop/main.js` secara eksak: spawn `wa-bot-backend`, handshake `BACKEND_PORT:` stdout, kill bersih tanpa orphan (catatan ARCHITECTURE.md: wa-bot env tidak punya encryption key — jangan copy pola kunci web-term)
- Settings hanya preferensi UI (kunci tema `wa-bot-theme*`); bukan secret backend
- Rust toolchain via `rust-toolchain.toml`; `[profile.release] lto="thin"`; target Linux + Windows (macOS out of scope)
- Tidak ada perilaku user-facing di phase ini — semua keputusan teknis

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `desktop/main.js` + `desktop/preload.js` — kontrak sidecar (spawn, port discovery via `__BACKEND_PORT__`, fallback `localhost:8080/api`) yang harus di-port ke Rust supervisor
- `../web-term/desktop-gpui/Cargo.toml` — set pin yang terbukti (gpui-pre =0.3.3, gpui-component =0.6.0, tokio =1.53.1, reqwest =0.12.28 rustls-tls, tokio-tungstenite =0.26.2, flume =0.12.0, dirs =6.0.0, dll.)
- `../web-term/desktop-gpui/crates/{supervisor,settings}/src` — implementasi referensi (baca, jangan copy buta)
- `web/src/data/themes.ts` + kunci `wa-bot-theme*` — format persistensi tema yang harus kompatibel (dipakai penuh di Phase 3)

### Established Patterns
- Backend Go: `cmd/api`, modul `wa-bot`, HTTP `/api` + WS `/ws`, secret pass-through `VITE_API_SECRET`
- Commit docs via `gsd_run query commit` (gsd-tools.cjs)

### Integration Points
- Supervisor → backend Go binary (dibangun dari source yang sama via `go build ./cmd/api` atau binary yang sudah ada)
- Settings → OS config dir via `dirs` crate (paritas layout userData Electron)
- Fondasi ini di-consumes oleh Phase 2 (backend-client) dan Phase 3 (app shell)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Acuan pola: `../web-term/desktop-gpui` (boleh dibaca saat struggle).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
