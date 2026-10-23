//! Window glass and backdrop support behind a small GPUI compatibility layer.
//!
//! The app content always paints translucent color fills. The local GPUI patch
//! adds compositor blur and rounded blur regions; upstream GPUI falls back to
//! plain transparency while keeping the same colors and layout.

use gpui::{
    point, px, size, App, Bounds, BoxShadow, Hsla, Pixels, Window, WindowBackgroundAppearance,
};
use wabot_settings::MAX_WINDOW_TRANSPARENCY;

pub const WINDOW_CORNER_RADIUS: f32 = 15.0;

/// Active rounded CSD radius, unchanged when the GTK style is disabled.
pub fn corner_radius(cx: &App) -> f32 {
    crate::theme::manager::ThemeManager::global(cx).shape.radius_window
}
pub const WINDOW_SHADOW_MARGIN: f32 = 16.0;

/// Apply the shared glass tint to an opaque theme color. Transparency is
/// expressed as a percentage, where 100 makes the tint fully transparent.
pub fn glass_fill(color: Hsla, transparency: u8) -> Hsla {
    let transparency = transparency.min(MAX_WINDOW_TRANSPARENCY) as f32 / 100.0;
    color.opacity(1.0 - transparency)
}

/// Request GPUI's compositor blur mode when enabled. Patched GPUI adds the
/// COSMIC `ext-background-effect-v1` path; upstream GPUI retains its supported
/// blur protocols and otherwise behaves as plain transparency.
pub fn background_appearance(blur_enabled: bool) -> WindowBackgroundAppearance {
    if blur_enabled {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Transparent
    }
}

/// Whether the restored app window should draw its inset rounded CSD frame.
pub fn is_framed_window(window: &Window) -> bool {
    !window.is_maximized() && !window.is_fullscreen()
}

/// Approximate a rounded rectangle with horizontal strips because Wayland blur
/// protocols accept rectangular regions only.
pub fn rounded_blur_regions(width: f32, height: f32, radius: f32) -> Vec<Bounds<Pixels>> {
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    if radius == 0.0 {
        return vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(width), px(height)),
        }];
    }

    let segments = radius.ceil().clamp(1.0, 10.0) as usize;
    let band = radius / segments as f32;
    let mut regions = Vec::with_capacity(segments * 2 + 1);
    for segment in 0..segments {
        let offset = segment as f32 * band;
        let from_corner = radius - offset;
        let inset = radius - (radius * radius - from_corner * from_corner).max(0.0).sqrt();
        let strip_width = (width - inset * 2.0).max(0.0);
        for y in [offset, height - offset - band] {
            regions.push(Bounds {
                origin: point(px(inset), px(y)),
                size: size(px(strip_width), px(band)),
            });
        }
    }
    let middle = height - radius * 2.0;
    if middle > 0.0 {
        regions.push(Bounds {
            origin: point(px(0.0), px(radius)),
            size: size(px(width), px(middle)),
        });
    }
    regions
}

/// Keep compositor blur inside the inset CSD panel, leaving its shadow margin
/// transparent. The returned rectangles are in window-local logical pixels.
pub fn inset_rounded_blur_regions(
    width: f32,
    height: f32,
    inset: f32,
    radius: f32,
) -> Vec<Bounds<Pixels>> {
    let inset = inset.max(0.0);
    let inner_width = (width - inset * 2.0).max(0.0);
    let inner_height = (height - inset * 2.0).max(0.0);
    rounded_blur_regions(inner_width, inner_height, radius)
        .into_iter()
        .map(|region| Bounds {
            origin: point(region.origin.x + px(inset), region.origin.y + px(inset)),
            size: region.size,
        })
        .collect()
}

/// Paint a soft frame shadow inside the reserved transparent margin.
pub fn frame_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(3.0), gpui::rgba(0x0000004D).into())
        .blur_radius(px(10.0))]
}

/// Limit compositor blur to the card bounds when the local GPUI patch is
/// enabled. Upstream GPUI has no region API, so this safely becomes a no-op.
pub fn set_blur_regions(window: &Window, regions: &[Bounds<Pixels>]) {
    #[cfg(feature = "gpui-wayland-blur-patch")]
    window.set_background_blur_regions(regions);

    #[cfg(not(feature = "gpui-wayland-blur-patch"))]
    let _ = (window, regions);
}
