# GTK style (Adwaita) plan

Goal: the "GTK style" switch (Settings → Appearance) makes the app *feel* like a
libadwaita app, on top of any colour theme. It is a switch, not a theme: off = today's look.

Reference studied: `ricewind012/discord-gnome-theme` (SCSS in `src/`; values below are
copied from `src/global/variables/libadwaita-*.scss`, `src/mixins/controls.scss`).

## Decisions already taken (change only with the user's say-so)

1. **Colour:** in GTK style the surface levels (window / view / sidebar / card / dialog /
   popover) are **derived from the active palette**, not new palettes. Details in phase 0.
2. **Icon buttons** are 9px rounded squares everywhere. Circles only for avatars, OSD
   buttons over media, window controls, and unread badges.
3. **Font:** follow the system GTK font when the switch is on (no bundling).
4. **Icons:** keep the current set. A symbolic 16px set is out of scope.
5. **Chat bubbles stay bubbles** (WhatsApp identity): 12px, flat, no shadow. Not Discord-style rows.
6. **Vendored `gpui-component`** (`desktop-gpui/vendor/gpui-component`) may be patched, but
   only behind the theme flag below so the switch off stays pixel-identical. Keep each patch small
   and note it in `vendor/gpui-component/PATCHES.md` (create it) so bumps are not painful.

## Rules for every phase

- Read `CLAUDE.md` and `docs/PARITY.md` first. **gpui and web change together**, same commit.
- Switch off must look exactly like before. Check both states for every change.
- Never put a literal radius/colour/height in a view. Add a token, then use it.
  gpui: `ShapeTokens` in `crates/app/src/theme/shape.rs`, read with `cx.shape()`.
  web: `--shape-*` / `--adw-*` vars in `web/src/index.css`, override in `:root[data-style="adwaita"]`.
- Checks: `cargo check -p wabot && cargo test -p wabot`, `cd web && npx tsc -b`.
- Update `docs/PARITY.md` (the "Shape tokens" section) when you add a token.
- One phase = one PR/commit series on `develop`. Beta: `gh workflow run desktop-gpui-release.yml
  --ref develop -f channel=beta`, then `desktop-gpui/scripts/install-beta.sh`. Restart the app yourself.
- `gh` may be on the wrong account when pushing; the repo owner is `Aorus22`
  (`gh auth switch --user Aorus22`, then push immediately).
- Commits carry no Co-Authored-By trailer.

## Where things stand (already done)

- `ShapeTokens` + `gtk_style` setting + switch in both clients.
- Tokens present: `radius_control`, `radius_card`, `radius_group`, `radius_button`,
  `radius_input`, `radius_bubble`, `widget_radius(_lg)`, `header_height`, `shadows`.
- `sync_font_size` pushes `widget_radius`, `widget_radius_lg`, `shadows` into
  `gpui_component::Theme`, so stock widgets already get 9px / 12px.
- Web: `:root[data-style="adwaita"]` with `--radius: 11px` (so shadcn `rounded-md` = 9px),
  `--shape-*`, `--header-h`; `rounded-card|bubble|button|input|group` utilities.
- Migrated so far: bubbles, chat list rows, header icon buttons, composer, settings groups, headers 46px.
- Boxed lists now cover call history, starred/search/message-info lists, group members, backup history,
  and the related info-sheet and compose-dialog rows in both clients.
- Remaining application icon buttons use the `rounded-button` utility in web; GPUI controls use
  `ShapeTokens::radius_button`. The emoji/sticker switch uses 9px group corners and 6px selected
  segment corners in GTK style.

## Reference values (libadwaita, from the repo)

| Token | Value |
| --- | --- |
| button / input / menu radius | 9px |
| card radius | 12px |
| popover radius | 15px (menu radius + 6) |
| window radius | 15px (button radius + 6) |
| headerbar height | 46px |
| icon button | 34 x 34px |
| titlebar control face / hit area | 26px / 34px |
| button height | 34px, text bold |
| scrollbar width | 8px |
| spacing unit | 6px (density 0.75 / 1 / 1.25) |
| transition | 200ms `cubic-bezier(0.25, 0.46, 0.45, 0.94)`; none under reduced motion |
| caption text | 0.82rem, colour = fg at 50% |
| title-4 / 3 / 2 | 1.18 / 1.36 / 1.36rem, weight 700 / 700 / 800 |
| dim level (disabled, muted) | 50% |
| border | fg at 15% |

Dark surfaces: window `#222226`, view `#1d1d20`, sidebar `#2e2e32`, secondary sidebar
`#28282c`, headerbar `#2e2e32`, card white 8%, dialog/popover `#36363a`.
Light: window `#fafafb`, view `#ffffff`, sidebar `#ebebed`, secondary `#f3f3f5`,
headerbar `#ffffff`, card `#ffffff`, dialog `#fafafb`, popover `#ffffff`.
Unfocused window: headerbar and sidebars switch to their *backdrop* colour
(dark `#222226` / `#28282c` / `#252529`, light `#fafafb` / `#f2f2f4` / `#f6f6fa`) and headerbar text dims to 50%.

Elevation (box-shadow stacks):
- low: `0 1px shade50%, 0 2px 4px shade50%` (shade = headerbar-shade)
- medium: `0 0 0 1px rgb(0 0 6/3%), 0 1px 3px 1px rgb(0 0 6/7%), 0 2px 6px 2px rgb(0 0 6/3%)`
- high: `0 0 14px 2px rgb(0 0 6/3%), 0 0 5px 2px rgb(0 0 6/10%), 0 0 0 1px rgb(0 0 6/5%)`

gpui has no multi-shadow CSS: approximate with a 1px border at the ring alpha plus one
`shadow_sm`-like blur, and keep it very faint.

## Phase 0: tokens

Add everything later phases need, with `DEFAULT` = current look.

gpui (`theme/shape.rs`, `theme/manager.rs`):
- Add `SurfaceTokens` (window, view, sidebar, secondary_sidebar, headerbar, card, dialog,
  popover, plus each `*_backdrop`) as `Hsla`, built by `SurfaceTokens::from_tokens(&ActiveTokens, is_dark)`.
  Derivation (so it works with every palette): `view` = background darkened ~4% (dark) /
  lightened (light); `window` = background; `sidebar` = background lightened ~6% (dark) /
  darkened ~6% (light); `card` = foreground at 8% over view (dark) / white (light);
  `dialog`/`popover` = sidebar-ish +2%. Do the maths in OKLab or HSL with unit tests.
  When the active preset is the "Desktop (GTK)" one (`theme/gtk.rs`), read
  `window_bg_color`, `view_bg_color`, `sidebar_bg_color`, `headerbar_bg_color`, `card_bg_color`,
  `popover_bg_color`, `dialog_bg_color` straight from `UserCss` instead of deriving.
- `fg_muted` = foreground at 50%. `border` = foreground at 15%. `tint(n)` = foreground at n%
  (used for button fill 10 / hover 15 / active 30 and row hover 3 / active 8).
- Spacing unit 6px + `density` multiplier. Do not add a density setting yet.
- Motion: `transition_ms = 200` (0 if the desktop asks for reduced motion; read
  `gtk-enable-animations` from `~/.config/gtk-4.0/settings.ini`, default on).
- Expose `cx.surface()` like `cx.shape()` (`SurfaceExt`).
- Font: when GTK style is on, read `gtk-font-name` from `~/.config/gtk-4.0/settings.ini`
  (fallback `gtk-3.0`), parse "Family Size". Apply the family via gpui-component `Theme.font_family`
  and the size as the base rem (keep the user's "Interface text size" percentage on top).
  Fallback: leave the font alone when the family is missing.

web (`index.css`, `AppThemeProvider.tsx`):
- `--adw-window`, `--adw-view`, `--adw-sidebar`, `--adw-card`, `--adw-popover`, `--adw-headerbar`
  and `--adw-*-backdrop`, computed with `color-mix(in oklab, var(--background) ..., white|black)`
  under `[data-style="adwaita"]` (so no JS). Map `--background`, `--card`, `--popover`,
  `--sidebar`, `--muted` onto them in the same block so shadcn components follow.
- `--tint-10/15/30` etc. as `color-mix(in srgb, currentColor n%, transparent)`.
- `--ease-adw: cubic-bezier(0.25,0.46,0.45,0.94)`, `--dur-adw: 200ms`, `0ms` under
  `@media (prefers-reduced-motion)`.
- Font: `font-family: "Adwaita Sans", Cantarell, system-ui, sans-serif` under the data attribute.
- Window focus: toggle `class="app-focused"` on `<html>` from `window` focus/blur events.

Done when: tokens exist in both clients, unit tests cover the colour derivation and the
settings.ini parser, nothing visible changes yet with the switch off.

## Phase 1: buttons and controls

Look (repo: `mixins/controls.scss`):
- **Button:** no border, no shadow, radius 9, height 34, bold label, padding 5/10. Fill
  `tint(10)`, hover `tint(15)`, active `tint(30)`, 200ms. Disabled = 50% opacity.
- **Accent button:** `accent` solid; hover = accent mixed 5% black; active = 30% black.
- **Destructive button:** `destructive` text on `tint(15)`; hover 20, active 35.
- **Icon button:** 34x34, transparent, same hover tints, radius 9.
- **Switch:** track 40x24 (2px inset), knob 20px white with `0 2px 4px black20%`; off track =
  fg 15% (hover 22.5, active 8); on = accent. Knob slides in 100ms.
- **Slider:** track 4px, fill accent, grabber 20px white circle with
  `0 0 0 1px rgb(0 0 6/10%), 0 2px 4px rgb(0 0 6/20%)`.
- **Text field:** fill `currentColor 10%` over window, **no border**, radius 9, padding 8/9.
  Focus: 2px outline accent at 50%, offset -2px, animated 200ms.
- **Toggle group** (Auto / Detail / Sidebar): container `tint(10)` radius 9 padding 3; selected
  item is a raised pill (`active-toggle-bg`: white light / fg 20% dark) with elevation-medium
  and radius 6; separators 1px at 25%, hidden around hover/selected.
- **Checkbox/radio:** radio = 22px, 2px ring at 25%, selected = 7px accent border with white centre.

gpui: patch `vendor/gpui-component/src/{button,switch.rs,slider.rs,input,checkbox.rs,radio.rs}`
behind a `Theme` field (e.g. `theme.adwaita: bool`, set from `sync_font_size`).
Repo-owned controls: `components/text_field.rs`, `views/settings.rs` toggle groups, composer buttons.
Then finish the long tail of `.rounded_full()` icon buttons (grep `rounded_full`; keep avatars,
badges, OSD, window controls): sheets (`views/chat/sheets/*`), viewers (`image_viewer.rs`,
`video_viewer.rs`), `components/*`, `views/calls.rs`, `views/settings_*.rs`.
web: restyle `components/ui/{button,switch,slider,input,textarea,toggle-group,tabs,checkbox,radio-group}.tsx`
through the tokens only (no per-call-site edits); then replace remaining `rounded-full`
icon buttons with `rounded-button`.

Done when: every button, switch, slider, input and toggle group in both clients matches the
table above in both states; focus rings animate; no literal radius left on a control.

## Phase 2: structure

- **Headerbar** 46px, bg `headerbar`, bottom edge = elevation-low, title 700 weight, window
  controls as 10%-tint circles (they are the one circular exception). Applies to chat header,
  chat-list header, calls, settings header, sheets (`views/chat/sheets/*`, `viewed_by.rs`,
  `selection_bar.rs`, `components/titlebar.rs`).
- **Sidebars:** nav rail and chat list on `sidebar` / `secondary_sidebar`, 1px right border
  `sidebar-border` (fg 7% light, black 36% dark). Selected row = `tint(10)` flat with radius 6;
  hover `tint(5)`; no accent bar. Width default 263 (keep the user's resizable width).
- **Boxed lists:** one container, radius 12, medium elevation (or 1px ring), rows separated by
  1px `card-shade`, only first/last row corners round, row hover `tint(3)` / active `tint(8)`,
  row min-height 54, padding 10/16. Use for settings groups (done partly), call history, starred,
  info sheet member list, backup/history rows. Provide one gpui helper (`boxed_list(rows)`) and one
  web component so new screens cannot drift.
- **Unfocused window:** headerbar and sidebars use the backdrop colours, headerbar text 50%.
  gpui: `window.is_window_active()` plus observe activation to refresh. web: `.app-focused`.
- **Window:** corner radius 15 (`window_material::WINDOW_CORNER_RADIUS`); check the framed/CSD paths.

## Phase 3: overlays

- **Popover / menu:** radius 15 (menu items 9), bg `popover`, shadow `0 1px 5px 1px black9%, 0 2px 14px 3px black5%`, item padding 12/6.
- **Dialog / sheet:** bg `dialog`, radius 15, elevation-high, scrim 50% black.
- **Tooltip:** `rgba(0,0,0,.8)`, white text, 1px white 10% border, radius 9, padding 6/10, no shadow.
- **Toast:** pill (radius 150), bg `#505053`, white text, small padding, bottom-centre
  (`components/toast.rs`, `web/src/components/ui/sonner.tsx`). Today toasts are top-centre
  on purpose; keep the placement, change only the shape.
- **Scrollbar:** 8px, thumb fg 20% (hover 40, active 60), track transparent, 200ms linear.
  gpui: `theme.scrollbar`; web: `::-webkit-scrollbar`.
- **Selection colour:** accent 30% when focused, fg 10% otherwise.

## Phase 4: chat surfaces

- Bubbles: 12px, flat, no shadow (done); tail corner 4px; outgoing = accent-tinted
  (accent 20% over view), incoming = `card`. Keep read ticks and reactions as they are.
- Composer: rounded 9px field on a `window` strip, send = accent button 34px, others icon buttons.
- Chat list rows: 12px -> flat selectable rows (see phase 2), unread badge stays a pill.
- Sheets (info, search, starred, viewed-by), forward / new-group dialogs: boxed lists + headerbar.
- Media viewers: OSD controls (circular, black 65%, group radius 999) are the allowed circles.
- Day pills, typing line, banners: banner = `#7d7d83` 30% over window, bold text, padding 6.
- Context menus (`views/chat/conversation/header_menu.rs`) follow phase 3.

## Phase 5: typography and density (last, optional parts marked)

- Text roles as tokens: caption 0.82rem at 50% fg, body 1rem, heading 700, title-4 1.18rem
  700, title-3 1.36rem 700, title-2 1.36rem 800. Replace ad-hoc `text_xs`/`text-xs` at
  section labels and hints. Keep sizes multiplied by the user's interface text size.
- Settings section titles ("APPEARANCE") become `heading` style, not small uppercase, in
  GTK style only.
- Optional: density multiplier (0.75 / 1 / 1.25) as a setting. Not started; ask first.
- Optional: symbolic 16px icon set. Out of scope unless the user asks.

## Verification checklist (for the reviewer agent)

For each phase, with the switch ON and OFF, dark and light, and the "Desktop (GTK)" palette:
- OFF is identical to `main` (screenshot diff on settings, chat, calls, a dialog).
- ON: no circular icon buttons outside the allowed list; no literal radius left in the files
  the phase touched; hover/active/disabled/focus states exist for every control.
- gpui and web show the same options, wording and shapes (checklist in `docs/PARITY.md`).
- `cargo check -p wabot && cargo test -p wabot`, `cd web && npx tsc -b`, `go build ./...` if the backend was touched.
- Window unfocused: headerbar and sidebars dim.
- Reduced motion on: no transitions.

## Risks

- Vendor patches rot on `gpui-component` bumps. Keep them gated, small, and listed in `PATCHES.md`.
- Deriving surface levels from arbitrary palettes can give low contrast. Clamp with the
  contrast helpers already in `theme/gtk.rs` (`MIN_ACCENT_CONTRAST`, `MIN_MUTED_CONTRAST`).
- System font may be missing or tall; fall back silently, never crash.
- Multi-layer shadows do not exist in gpui; accept an approximation and do not chase it.
