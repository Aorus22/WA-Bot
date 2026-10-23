# WA-Bot

Go backend (`cmd/`, `internal/`), a native desktop client (`desktop-gpui/`, Rust, gpui)
and a web client (`web/`, React). Focus is the gpui app, but see below.

## gpui and web stay 1:1

Both clients show the same features, settings, wording and behavior. **If you change
one, change the other in the same commit.** A backend change must be checked against
both. Read `docs/PARITY.md` before touching UI, settings, chat state or the API, and
update it when you add a feature or an intentional difference.

For GPUI component ownership, see [`docs/GPUI-COMPONENTS.md`](docs/GPUI-COMPONENTS.md).

## Workflow

- Experiments go on `develop` (beta). Releases go to `main` (stable, with the updater).
  Beta: `gh workflow run desktop-gpui-release.yml --ref develop -f channel=beta`, then
  `desktop-gpui/scripts/install-beta.sh`.
- Commits carry no Co-Authored-By trailer.
- After building, stop and restart the running app and its backend yourself.

## Checks

- Go: `go build ./... && go test ./internal/...`
- gpui: `cd desktop-gpui && cargo check -p wabot && cargo test -p wabot`
- web: `cd web && npx tsc -b` (plain `tsc --noEmit -p .` checks nothing here)
