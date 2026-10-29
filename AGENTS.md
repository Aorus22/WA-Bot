# WA Bot project instructions

## Desktop releases

When the user asks for a WA Bot beta:

- Commit and push the requested changes to `develop`, then dispatch `.github/workflows/desktop-gpui-release.yml` with `channel=beta`.
- Give the commit a clear subject and include a complete bullet list of the changes in its body.
- After CI publishes the rolling beta, replace generic release notes with a complete bullet list describing every change.

When the user asks to promote changes to a stable WA Bot release:

- Squash the requested `develop` changes into one commit on `main`. Give the commit a clear subject and include a complete bullet list of the changes in its body.
- Keep the desktop workspace version in `desktop-gpui/Cargo.toml`, the matching `Cargo.lock` package versions, and the `vX.Y.Z` tag in sync.
- Build stable releases through `.github/workflows/desktop-gpui-release.yml` by pushing the matching tag.
- After CI publishes the stable release, replace generic release notes with a complete bullet list describing every change. The commit body and published release notes must both explain what changed.

For either release channel:

- Build through CI only. Do not build or run Cargo tests locally; the user's local machine is resource constrained.
- Verify that CI succeeded and that the AppImage and `SHA256SUMS.txt` are attached. Report links to the release and AppImage.
- Do not install the release or switch, pull, reset, or otherwise update the user's local checkout unless they explicitly ask. The user prefers to update their local installation themselves.
- For `Aorus22/WA-Bot`, use the existing GitHub CLI authentication and the `Aorus22` account; switch accounts only if needed. Never ask the user to provide a token when `gh` is already authenticated.
- Keep the user's existing local changes untouched. Use a temporary clone for release operations when needed.
