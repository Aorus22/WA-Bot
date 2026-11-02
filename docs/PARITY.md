# gpui / web parity

The desktop app (`desktop-gpui/`) and the web app (`web/`) are two clients of the
same Go backend. **They must behave the same.** A change to one needs the matching
change in the other, in the same commit or the same PR.

## The rule

- A feature, setting, label, default or behavior that exists in one client exists in
  the other, with the same wording and the same options.
- Backend changes (`internal/`, `cmd/`) reach both clients: check that both still
  work, and that both read any new field or endpoint.
- A bug fix counts too. If the bug is in logic both clients share (a tick, an unread
  count, a typing line), look for the same bug in the other one.
- Layout may follow each toolkit (a dropdown is a `Select` in the web and a popover in
  gpui), but the information, options and order stay the same.

## Checklist before you commit

1. Which client did I touch? Open the equivalent screen in the other one.
2. New or changed setting key? Both settings pages read and write it.
3. New endpoint or WebSocket event? Both clients handle it (`backend-client` for gpui,
   `web/src/lib/api.ts` and `ws-bus` for the web).
4. Copy: same title, same hint text, same button labels.
5. Update the table below if you add a feature or an intentional difference.

## Where the same thing lives

| Feature | gpui | web |
| --- | --- | --- |
| Settings page | `crates/app/src/views/settings.rs` and `settings_*.rs` | `src/pages/settings/SettingsPage.tsx` |
| Chat history and media rows | `views/settings_history.rs` | `SettingsPage.tsx`, `HistoryMediaSection.tsx` |
| Backup (back up now, per-backup Restore that merges: adds only what is missing, replaces nothing) | `views/settings/backup.rs` | `BackupSection.tsx` |
| AI provider | `views/settings_ai.rs` | `SettingsPage.tsx` (AI) |
| Theme picker | `settings.rs` (modal) | `SettingsPage.tsx` (dialog) |
| WinUI / Fluent style | `theme/shape.rs` (`WINUI`), `SurfaceTokens::apply_fluent` in `theme/surface.rs`, setting `winui_style` | `--winui-*` vars and `:root[data-style="winui"]` rules in `index.css`, key `wa-bot-winui-style` |
| GTK style (shape tokens) | `theme/shape.rs`, `ThemeManager::apply_gtk_style`, setting `gtk_style` | `--shape-*` vars in `index.css`, `data-style="adwaita"` set by `AppThemeProvider.tsx`, key `wa-bot-gtk-style` |
| Typing indicator | `state/chat.rs`, `chat_list.rs`, `conversation/typing.rs` | `stores/typingStore.ts`, `ChatSidebar.tsx`, `ChatArea.tsx` |
| Calls page | `views/calls.rs` | `src/pages/calls/CallHistoryPage.tsx` |
| Message ticks | `state/chat.rs` (`MessageStatus`), `message_bubble.rs` | chat store, message bubble |
| Unread and read receipts | `chat_actions.rs`, `data/loading.rs` | `ChatSidebar.tsx` |
| API client | `crates/backend-client/src/client.rs`, `dto/` | `src/lib/api.ts` |

## Intentional differences

These exist because of the platform, not by accident. Keep this list short.

| Difference | Why |
| --- | --- |
| Window transparency, backdrop blur, interface and chat text size, chat layout | A native window has them; a browser tab does not. |
| Window controls (minimize, maximize, close) sit in the chat headers, after a separator, and the empty header space drags the window | The web page has no window of its own. The title bar row is gone on the chat page; other pages keep it. |
| When the window is too narrow for the rail, the list and a 560 px conversation, `Auto` layout shows the chat list and the conversation as separate pages | Lets the window be tiled to half of a small screen. The web follows its own breakpoints. |
| OS notifications, autostart, resident daemon | Desktop only. |
| In-app updater, beta channel | The web is served, not installed. Beta builds hide the update buttons. |
| Backup folder: native folder picker (gpui), typed absolute path (web) | A browser cannot return an absolute path. Both talk to `/api/backup/*`. |
| Settings layout details (popover vs `Select`, modal vs dialog) | Each toolkit's own control. |

## Behavior both clients share

- Only the avatar and name in a conversation header open the info sheet. The rest of
  the header does nothing. In gpui, pressing and dragging that block moves the window,
  so a narrow window can still be moved.
- Read receipts off: opening a chat does not clear its unread state, anywhere. Only an
  explicit "Mark as read" (`POST /chats/{id}/read?explicit=1`) does.
- Typing indicators need the account online. Opening any chat, group or private, calls
  `presence-subscribe`.
- Day separators ('TODAY', '28 SEPTEMBER 2026') are small content-sized pills; digits are tabular so dates of one length match.
- Day separators ('TODAY', '28 SEPTEMBER 2026') are pills of one fixed width.
- The window controls sit over the top right corner of the chat page in gpui, so anything
  full-width up there (image viewer, video viewer, sheet headers) leaves `controls_room` free.
- Call history rows show the contact's name and profile picture (the backend adds
  `peer_id` and `peer_name`), and no total count.
- Progress bars show only while something runs; afterwards one summary line.

## Shape and surface tokens (GTK style)

The "GTK style" switch is a visual layer, not a colour theme: it preserves the active
palette while applying Adwaita shapes, controls, surfaces, typography, and motion. With
the switch off, both clients retain their existing styling.

- gpui: add shape values to `ShapeTokens` (`theme/shape.rs`) with `DEFAULT` and `ADWAITA`
  values, and read them via `cx.shape()`. Toggle groups use a 9px container, 6px selected
  segment and palette-aware raised selection in Adwaita mode. Boxed-list row height,
  padding, separator, flat corners and overlay scrims are also tokens. Text roles are
  `cx.shape().typography`.
  Palette-derived surfaces live in `SurfaceTokens` (`theme/surface.rs`) and are read
  with `cx.surface()`; this includes the pinned-banner color, applied only when Adwaita
  is enabled. Desktop (GTK) palette overrides are used only when that preset is active.
  `gpui-component` widget overrides are gated by `Theme::adwaita` and documented
  in `desktop-gpui/vendor/gpui-component/PATCHES.md`.
- web: `--shape-*`, `--adw-*` (including `--adw-pinned-banner`), and `--adw-title-*`
  variables live in `web/src/index.css`; the shared `BoxedList` component (`components/ui/boxed-list.tsx`)
  provides boxed rows for call history, backup history, group members and message lists.
  Adwaita overrides are scoped to
  `:root[data-style="adwaita"]`.
  `AppThemeProvider` sets that attribute from the shared `wa-bot-gtk-style` setting and
  tracks window focus.

Use a token instead of a literal radius, surface colour, or control/header height in a
view. Adwaita reference values include 9px control radii, 12px cards and message bubbles,
15px popovers/windows, 34px controls, and a 46px headerbar. They are based on
`ricewind012/discord-gnome-theme` (`libadwaita-other.scss`, `controls.scss`).

## WinUI / Fluent style

The Fluent profile copies the Windows 11 WinUI templates: values come from
microsoft-ui-xaml (`Common_themeresources_any.xaml` and the per-control theme
resources), not from the active palette. Only the accent comes from the palette; the
surface and text colors are neutral. Both clients use the same values.

- Layers: the window, navigation pane and title bar are the base layer (`#202020` /
  `#f3f3f3`, no border). The content area is a sheet above it (`#282828` / `#f9f9f9`)
  with a 1px stroke on its left and top edges and an 8px top-left radius. Cards are a
  5% white (dark) / 70% white (light) overlay with a 1px stroke; flyouts `#2c2c2c` /
  `#fcfcfc`. Text is white / 90% black, secondary 77% / 62%.
- Controls: buttons 4px radius, padding 11/5, regular weight, fill 6% / 8% / 3% ink.
  Switch 40x20 with a 12px knob (outline when off). Checkbox and radio 20px. Slider
  4px track with an 18px thumb and a 12px accent dot. Text box 32px with a 2px accent
  line while focused. Scrollbar thumb 6px in gpui, 2px growing to 6px on web.
- Accent: darker shade on light, lighter shade on dark, with 90% / 80% on hover / press.
  gpui lightens the palette primary and picks black or white text by contrast; web uses
  the palette accent and its own on-accent color.
- Selected nav and list items: 3x16 accent pill on the leading edge, subtle fill.
- Settings: each row is its own card (68px minimum, 4px radius, 4px apart); section
  headings are 14px semibold sentence case. gpui renders this with `SettingsGroup`
  (`components/ui/card.rs`); sections that are one card (backup, history, update) stay
  one card.
- Flyouts and dialogs use the Fluent two-layer shadows; cards have none.
- Font: Segoe UI Variable on Windows, bundled Selawik (Microsoft's open Segoe UI
  stand-in, OFL) elsewhere. gpui bundles `assets/fonts/Selawik-*.ttf`; web serves
  `public/fonts/selawik-*.woff2`.
- gpui caption buttons are 46px wide with a `#c42b1c` close hover and no separator.

Intentional differences: Mica / acrylic blur is not implemented in either client. The
gpui button has one border color, so the brighter-top (dark) / darker-bottom (light)
control stroke is a web-only detail. gpui hover states do not animate, so the 167ms
transition is web-only. The navigation pane stays icon-only at 68px instead of WinUI's
labelled 320px pane.
