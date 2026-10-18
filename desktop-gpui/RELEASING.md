# Releasing the desktop app

Two channels, one workflow (`.github/workflows/desktop-gpui-release.yml`).

| Channel | Branch | How it ships | Who gets it |
|---|---|---|---|
| beta | `develop` | Actions, "Desktop GPUI Release", channel `beta`. Rolling prerelease `beta`, version `X.Y.Z-beta.N` | You, by downloading the AppImage by hand. The updater never offers it. |
| stable | `main` | Tag `vX.Y.Z` | Everyone, through Settings, Version & Update |

## Trying a new feature (beta)
1. Work on `develop`, push.
2. Run the workflow from `develop` with channel `beta`.
3. Download `wa-bot_X.Y.Z-beta.N_x86_64.AppImage` from the `beta` release and run it.

## Shipping (stable)
1. Squash-merge `develop` into `main`.
2. Raise `version` in `desktop-gpui/Cargo.toml`, commit.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`. The tag must equal the Cargo version or the build fails.
4. Reset `develop` to `main` so it does not diverge.

## How the updater works
Settings, Version & Update, "Check for updates" asks GitHub for `/releases/latest`
(prereleases are excluded, so beta is invisible). "Update now" downloads the
AppImage, checks it against `SHA256SUMS.txt`, replaces the running file atomically
and "Restart" relaunches it with `--popover`. It only works from an AppImage; set
`WABOT_UPDATE_API` to point the check at a mock server for testing.
