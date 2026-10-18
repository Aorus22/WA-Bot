# Desktop platform integration parity

Status: **partial; notification policy, private D-Bus delivery and same-user local-instance commands have mock/private-bus tests.** A real tray host, GNOME notification source, and Wayland panel behavior were not available for runtime verification.

| GPUI source | QML/Python | Contract | Evidence | Status |
|---|---|---|---|---|
| `notifications.rs` | `bridge/platform.py`, `bridge/platform_core.py` | `new_message` policy, mute/read/age filters, cross-client claim receipts, Freedesktop `Notify` | `tests/test_platform.py` with a private D-Bus service | partial — policy, cross-client claim receipts, and private-bus delivery with the wa-bot-qml desktop-entry hint are tested; a real notification daemon/permissions check remains open |
| `tray.rs`, `main.rs` | `QApplication`, `QSystemTrayIcon`, `Main.qml` close/show handling | show, toggle, quit; close hides only when a tray host is available | `tests/test_platform.py` private StatusNotifierWatcher under Xvfb plus local socket tests | partial — Qt registration/activation pass against a private watcher; Main.qml close-to-tray and show/toggle visibility pass with a Platform test double; actual GNOME/COSMIC host behavior remains unverified |
| `popover.rs` | `bridge/platform_core.py`, `Platform`, `PopoverMain.qml` | `--popover`, `--background`, `--toggle`; panel edge, pointer cache, backdrop dismissal | source-matched placement logic; compositor capture remains open | partial — QML uses the optional KDE LayerShellQt integration on Wayland when `wayland-info` reports `zwlr_layer_shell_v1`, including edge anchors, the GPUI pointer cache, and the transparent dismissal backdrop; the current GNOME session advertises no layer-shell protocol and therefore takes the regular-window fallback; forcing the QPA integration there aborts, so supported-compositor behavior remains unverified |
| single instance | `LocalInstanceServer` | per-user `QLocalServer` command channel; explicit backend URLs stay standalone | `tests/test_platform.py` | partial — open/toggle reach Platform signals, fragmented JSON frames buffer, and the QML window responds to show/toggle signals; a real second process raising a desktop window remains untested |

The app does not claim universal tray or Wayland layer-shell support. Without a tray host, closing the window exits normally.
