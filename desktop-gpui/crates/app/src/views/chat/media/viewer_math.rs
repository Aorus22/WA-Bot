//! Pure geometry of the full-screen image viewer (fit, zoom-to-cursor, pan).

use gpui::{Bounds, Pixels};

pub const PREVIEW_TOOLBAR_H: f32 = 56.0;
pub const PREVIEW_PAD: f32 = 24.0;
pub const PREVIEW_MIN_ZOOM: f32 = 1.0;
pub const PREVIEW_MAX_ZOOM: f32 = 8.0;

/// Image area of the viewer for overlay `bounds`: `(width, height, centre x,
/// centre y)` in overlay coordinates. At zoom 1 the image is contained in this
/// rectangle, i.e. scaled by `min(area_w / img_w, area_h / img_h)`.
pub fn preview_geometry(bounds: Bounds<Pixels>) -> (f32, f32, f32, f32) {
    let (mut w, mut h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if w < 50.0 || h < 50.0 {
        // First frame, before the bounds are measured.
        (w, h) = (880.0, 620.0);
    }
    let area_w = (w - 2.0 * PREVIEW_PAD).max(100.0);
    let area_h = (h - PREVIEW_TOOLBAR_H - PREVIEW_PAD).max(100.0);
    (area_w, area_h, w / 2.0, PREVIEW_TOOLBAR_H + area_h / 2.0)
}

/// New pan offset after zooming from `old` to `new` so the image point under
/// `anchor` stays under it. `center` is the viewer centre and `pan` the current
/// offset of the image centre from it (all in overlay coordinates).
pub fn pan_after_zoom(
    anchor: (f32, f32),
    center: (f32, f32),
    pan: (f32, f32),
    old: f32,
    new: f32,
) -> (f32, f32) {
    // Anchor offset from the image centre, in zoom-1 units.
    let qx = (anchor.0 - (center.0 + pan.0)) / old;
    let qy = (anchor.1 - (center.1 + pan.1)) / old;
    (anchor.0 - center.0 - qx * new, anchor.1 - center.1 - qy * new)
}

/// Keep a zoomed image from being dragged entirely out of the viewer.
pub fn clamp_pan(pan: (f32, f32), area_w: f32, area_h: f32, zoom: f32) -> (f32, f32) {
    let max_x = area_w * zoom / 2.0;
    let max_y = area_h * zoom / 2.0;
    (pan.0.clamp(-max_x, max_x), pan.1.clamp(-max_y, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    /// Screen position of the image point `q` (zoom-1 units from its centre).
    fn screen_of(q: (f32, f32), center: (f32, f32), pan: (f32, f32), zoom: f32) -> (f32, f32) {
        (center.0 + pan.0 + q.0 * zoom, center.1 + pan.1 + q.1 * zoom)
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_fixed() {
        let center = (440.0, 330.0);
        let (old, new) = (1.5, 3.0);
        let pan = (30.0, -20.0);
        let anchor = (600.0, 250.0);
        // The image point under the anchor before zooming.
        let q = ((anchor.0 - center.0 - pan.0) / old, (anchor.1 - center.1 - pan.1) / old);
        let after = pan_after_zoom(anchor, center, pan, old, new);
        let at = screen_of(q, center, after, new);
        assert!((at.0 - anchor.0).abs() < 1e-3 && (at.1 - anchor.1).abs() < 1e-3);
    }

    #[test]
    fn zooming_about_the_centre_keeps_the_pan() {
        let center = (440.0, 330.0);
        let after = pan_after_zoom(center, center, (0.0, 0.0), 1.0, 2.0);
        assert_eq!(after, (0.0, 0.0));
    }

    #[test]
    fn pan_is_clamped_to_the_zoomed_image() {
        let (w, h) = (752.0, 528.0);
        assert_eq!(clamp_pan((9999.0, -9999.0), w, h, 2.0), (752.0, -528.0));
        assert_eq!(clamp_pan((10.0, 10.0), w, h, 2.0), (10.0, 10.0));
    }

    #[test]
    fn geometry_leaves_room_for_the_toolbar() {
        let bounds = Bounds {
            origin: point(px(74.0), px(6.0)),
            size: size(px(800.0), px(608.0)),
        };
        let (area_w, area_h, cx_, cy) = preview_geometry(bounds);
        assert_eq!((area_w, area_h), (752.0, 528.0));
        assert_eq!(cx_, 400.0);
        assert_eq!(cy, PREVIEW_TOOLBAR_H + 264.0);
        // Not yet measured: falls back to the popover size.
        let (w, _, _, _) = preview_geometry(Bounds::default());
        assert_eq!(w, 880.0 - 2.0 * PREVIEW_PAD);
    }
}
