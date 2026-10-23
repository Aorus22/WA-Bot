# Local patches

These changes are kept local until an upstream release includes equivalent behavior.
Re-check them when updating `gpui-component`.

- `src/button/button.rs`: when the flag is enabled, use Adwaita variant fills and
  destructive text, 9px rounding, 34px button/icon-button dimensions, bold labels, and
  50% disabled opacity. The stock variant styling remains in place when disabled.
- `src/checkbox.rs`: use a 1.375rem indicator with a foreground-tinted unchecked ring
  and palette background only while enabled.
- `src/input/input.rs`: use a 10%-foreground field fill, remove its border, and use a
  2px/50%-ring focus border only while enabled.
- `src/radio.rs`: use a 1.375rem radio with a 2px ring and a 7px selected centre only
  while enabled.
- `src/slider.rs`: use a white thumb and 4px track only while enabled.
- `src/switch.rs`: use the 40x24 track, 20px white thumb, and Adwaita off-state tints
  only while enabled.
- `src/theme/mod.rs`: add the non-serialized `Theme::adwaita` switch and optional
  host-provided dialog surface and scrollbar width. They default to `false`/`None`;
  the active scrollbar tint is strengthened only while enabled.
- `src/dialog/dialog.rs`: use the optional host dialog surface only while Adwaita is
  enabled; dialog radius comes from the host-synced `Theme::radius_lg`.
- `src/tooltip.rs`: use the Adwaita tooltip surface and compact padding only while the
  flag is enabled.
- `src/notification.rs`: use the GTK-style pill toast surface, radius, and compact
  padding only while the flag is enabled; placement is unchanged.
- `src/theme/mod.rs`: use the host scrollbar width and strengthen the active thumb
  tint only while Adwaita is enabled.
- `src/button/button.rs`: the WinUI flag uses 32px controls, semibold labels, subtle
  bordered neutral fills, and the active palette accent for primary actions.
- `src/checkbox.rs`, `src/radio.rs`, `src/switch.rs`, and `src/slider.rs`: WinUI uses
  the host's Fluent radii and accent while retaining component-specific dimensions.
- The non-serialized `Theme::winui` flag selects WinUI control styling and the same
  host-provided dialog and scrollbar tokens without enabling Adwaita styling.
