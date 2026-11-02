//! Downscaled copies of the pictures a chat bubble draws.
//!
//! gpui uploads a picture to its sprite atlas at its full resolution and
//! samples that tile with a linear filter over a single mip level, so a 1600px
//! screenshot squeezed into a 340px bubble keeps about one texel in
//! twenty-five: it comes out aliased and half unreadable, while the full-screen
//! viewer, which draws the same file at nearly 1:1, stays sharp. Handing the
//! bubble a picture that is already the size it is drawn at leaves the GPU
//! nothing to shrink.
//!
//! Each copy is made once and kept in [`ThumbnailCache`], which lives on the
//! chat view for as long as the pictures are on screen.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::views::chat::*;

/// A cached copy: the source picture it came from, its width and its height,
/// in device pixels.
pub(crate) type ThumbnailKey = (usize, u32, u32);

/// Downscaled pictures, most recently used first.
pub(crate) type ThumbnailCache = VecDeque<(ThumbnailKey, Arc<RenderImage>)>;

/// How many downscaled pictures to keep. Every one of them takes its own
/// sprite-atlas tile while it is cached, and a bubble is at most 340x360
/// logical pixels, so this stays in the low tens of megabytes even when every
/// entry is a big picture.
pub(crate) const THUMBNAIL_CACHE_LIMIT: usize = 32;

/// Below this linear shrink the GPU's own filtering is good enough and a copy
/// would only cost memory.
const MIN_SHRINK: f32 = 1.5;

/// The picture to draw in a bubble: the one at `url`, copied at the size the
/// bubble shows it. `None` while it is still loading, when it failed, or when
/// it is small enough to draw as it is.
pub(crate) fn bubble_picture(
    cache: &mut ThumbnailCache,
    url: &str,
    frame: (f32, f32),
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let source = preview_render_image(url, window, cx)?;
    let scale = window.scale_factor();
    let target = (
        (frame.0 * scale).round().max(1.0) as u32,
        (frame.1 * scale).round().max(1.0) as u32,
    );
    let key: ThumbnailKey = (source.id.0, target.0, target.1);
    let cached_index = cache.iter().position(|(cached, _)| *cached == key);
    let cached = cached_index.and_then(|index| cache.remove(index));
    if let Some(entry) = cached {
        // Reuse the copy, and mark it as the most recently used one.
        let picture = entry.1.clone();
        cache.push_front(entry);
        return Some(picture);
    }
    let picture = downscale(&source, target)?;
    evict_if_full(cache, window, cx);
    cache.push_front((key, picture.clone()));
    Some(picture)
}

/// A Lanczos3 copy of `source` for a bubble of `target` device pixels. `None`
/// when the picture is not enough larger than that to be worth copying.
pub(crate) fn downscale(source: &Arc<RenderImage>, target: (u32, u32)) -> Option<Arc<RenderImage>> {
    let natural = source.size(0);
    let (source_w, source_h) = (natural.width.0, natural.height.0);
    let (target_w, target_h) = target;
    if source_w <= 0 || source_h <= 0 || target_w == 0 || target_h == 0 {
        return None;
    }
    // How much the picture has to be scaled down to fill the bubble. The
    // bubble crops whatever sticks out on the long side (`ObjectFit::Cover`),
    // and its frame is clamped for extreme aspect ratios, so the copy keeps
    // the picture's own shape: a squashed one would come out stretched.
    let cover = (target_w as f32 / source_w as f32).max(target_h as f32 / source_h as f32);
    if cover * MIN_SHRINK > 1.0 {
        return None;
    }
    // An animated picture would have to be redone frame by frame, and gpui
    // draws the frames it is handed: those keep the GPU's own scaling.
    if source.frame_count() != 1 {
        return None;
    }
    // Rounding up keeps the copy covering the frame, so `Cover` has nothing to
    // stretch, only to crop.
    let copy_w = (source_w as f32 * cover).ceil().max(1.0) as u32;
    let copy_h = (source_h as f32 * cover).ceil().max(1.0) as u32;
    // A `RenderImage` holds BGRA, and `resize` works channel by channel, so the
    // copy is BGRA too, which is what the atlas upload expects.
    let buffer =
        image::RgbaImage::from_raw(source_w as u32, source_h as u32, source.as_bytes(0)?.to_vec())?;
    let resized = image::imageops::resize(
        &buffer,
        copy_w,
        copy_h,
        image::imageops::FilterType::Lanczos3,
    );
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(resized)])))
}

/// Drop the least recently used picture once the cache is full. The window has
/// to be told: a sprite-atlas tile outlives the `RenderImage` that made it.
fn evict_if_full(cache: &mut ThumbnailCache, window: &mut Window, cx: &mut App) {
    let dropped = if cache.len() >= THUMBNAIL_CACHE_LIMIT {
        cache.pop_back()
    } else {
        None
    };
    if let Some((_, dropped)) = dropped {
        cx.drop_image(dropped, Some(window));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `super::*` brings in gpui's own `test` macro, which does not expand here.
    use std::prelude::v1::test;

    fn picture(width: u32, height: u32, frames: usize) -> Arc<RenderImage> {
        let frames = (0..frames)
            .map(|_| {
                image::Frame::new(image::RgbaImage::from_pixel(
                    width,
                    height,
                    image::Rgba([1, 2, 3, 4]),
                ))
            })
            .collect::<Vec<_>>();
        Arc::new(RenderImage::new(frames))
    }

    #[test]
    fn a_picture_bigger_than_its_frame_is_copied_at_the_frame_size() {
        let source = picture(1000, 800, 1);
        let copy = downscale(&source, (250, 200)).expect("1000x800 down to 250x200 is a 4x copy");
        let size = copy.size(0);
        assert_eq!((size.width.0, size.height.0), (250, 200));
    }

    #[test]
    fn a_picture_that_already_fits_is_left_alone() {
        let source = picture(300, 200, 1);
        assert!(downscale(&source, (250, 200)).is_none());
    }

    #[test]
    fn an_animated_picture_is_left_to_the_gpu() {
        let source = picture(1000, 800, 2);
        assert!(downscale(&source, (250, 200)).is_none());
    }

    #[test]
    fn a_picture_with_an_extreme_aspect_ratio_keeps_its_shape() {
        let source = picture(4000, 400, 1);
        // A 340x180 bubble frame, on a 1.25x screen: the picture is ten times
        // as wide as it is tall and `Cover` crops the long side afterwards.
        let copy = downscale(&source, (425, 225)).expect("a 4000x400 picture is worth copying");
        let size = copy.size(0);
        assert_eq!((size.width.0, size.height.0), (2250, 225));
    }

    #[test]
    fn a_picture_the_bubble_stretches_is_left_alone() {
        // The frame is taller than the picture's own shape, so `Cover` would
        // enlarge it: there is nothing to shrink.
        let source = picture(1000, 100, 1);
        assert!(downscale(&source, (425, 225)).is_none());
    }
}
