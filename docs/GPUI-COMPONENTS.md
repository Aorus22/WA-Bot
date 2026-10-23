# GPUI component structure

The GPUI client has three component layers. This is an internal organization
for `desktop-gpui`; it does not change the React client.

## `components/ui`

This is the app-facing UI kit, following shadcn/ui's one-component-per-module
organization. The GPUI layer now has 57 component modules, with matching names
for the web kit wherever GPUI has a suitable control or container.

The modules cover buttons and groups; inputs, OTP, fields and forms; alerts,
dialogs, drawers, sheets, menus and popovers; avatars, badges, labels, items,
lists and tables; accordion, carousel, collapsible and tabs; calendars, charts,
progress, sliders, switches, toggles, skeletons, spinners, tooltips, and
resizable or scrollable surfaces. See the files in `components/ui/` for the
component-specific GPUI API.

Several UI components have app-owned behavior and theme integration:

| Component | Responsibility |
| --- | --- |
| `button` / `action_button` / `destructive_button` / `icon_button` | Shared button size and shape tokens, semantic variants, disabled behavior, and accessible icon names. |
| `TextField` / `TextArea` | Shared editable state, events, and app styling. |
| `card` | Themed card surface, border, radius, and elevation. |
| `boxed_list` | Shared list surface, row sizing, separators, and GTK-style grouped corners. |
| `dropdown_menu` | Anchored popup list with shared popover surface, border, radius, and elevation. |
| `form_row` | Generic label, hint, and trailing-control layout. |
| `aspect_ratio` | GPUI layout wrapper that keeps content at a requested aspect ratio. |

Most remaining modules expose the corresponding stateful `gpui-component`
control through a stable app-facing path. The GPUI library does not have a
direct counterpart for shadcn's direction provider or navigation menu yet.

UI components own reusable appearance, interaction, accessibility, and theme
token use. They should not know about a specific page or business workflow.
When app-specific styling is needed, wrap the underlying GPUI component here
instead of repeating that styling in views.

## Shared components in `components/`

These are app or feature components that combine UI controls and carry meaning
or behavior beyond a single control. A component belongs here when it is used
from more than one place. For example, `connection_banner` is used by both the
shell and settings, `message_bubble` is used by chat and notifications, and the
app shell wraps all routed pages from `components/shell/`.

Shared components compose `components/ui` and keep reusable feature behavior in
one place. They may use domain concepts, but should not become a second home
for generic controls.

## Local components beside a view

Components used by only one screen live with that screen under `views/`, in the
screen's module or subfolder. They can compose UI and shared components
and own screen-specific content. Keep page-specific positioning, grouping, and
gaps at this layer when those choices do not generalize. Chat's composer state,
layout preference, message renderers, and media viewers/player are examples of
this local layer.
Larger screens use a folder module: `views/settings/mod.rs` assembles the page,
while `ai.rs`, `backup.rs`, `controls.rs`, `history.rs`, `history_media.rs`,
and `history_sync.rs` hold its local sections and supporting logic. Their
buttons come from `components/ui/button` like buttons elsewhere in the app.
Calls, channels, and status pages also use `views/<page>/mod.rs` so any future
page-only components have a home beside their page.

## Placement rule

Start with `components/ui` for a generic control or surface. Compose it locally
when it serves one screen. Promote that composition to `components/` when two
or more views use it. A move alone is not a component abstraction: the shared
piece should own a clear reusable visual or behavioral contract.
