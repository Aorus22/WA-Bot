//! Shared motion language for the shell.
//!
//! One set of durations and easings so every transition in the app moves the
//! same way: things enter with a fast start that settles softly (ease-out
//! quint) and never bounce. All helpers go through gpui's `with_animation`,
//! which already honours `App::reduce_motion` (static end state, no frames).
//!
//! gpui keys animation state by element id and drops it once the element is
//! no longer rendered, so an id that *changes* (route, chat, status) restarts
//! the animation, while a stable id plays once per mount.

use std::time::Duration;

use gpui::{
    percentage, px, Animation, AnimationExt as _, AnimationElement, ElementId, Hsla, IntoElement,
    Rgba, Styled, Transformation,
};
use crate::components::ui::Icon;

/// Hover-adjacent feedback: menus, popovers, small chips.
pub const FAST_MS: u64 = 140;
/// Default entrance: overlays, cards, route changes.
pub const BASE_MS: u64 = 220;

/// Ease-out quint: quick start, long soft landing.
pub fn ease_out(d: f32) -> f32 {
    1.0 - (1.0 - d).powi(5)
}

/// Ease-out cubic: a little gentler, for small travel distances.
pub fn ease_out_cubic(d: f32) -> f32 {
    1.0 - (1.0 - d).powi(3)
}

/// A one-shot animation with the shared ease-out curve.
pub fn enter(ms: u64) -> Animation {
    Animation::new(Duration::from_millis(ms)).with_easing(ease_out)
}

/// Linear interpolation.
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Interpolate two colors in sRGB (including alpha), so a fade between a
/// transparent and an opaque color never passes through a muddy hue.
pub fn lerp_color(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (from.into(), to.into());
    Rgba {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
    .into()
}

/// Fade a whole element in.
pub fn fade_in<E: Styled + IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
) -> AnimationElement<E> {
    el.with_animation(id, enter(ms), |el, d| el.opacity(d))
}

/// Fade in while rising `distance` px into place. The element must be
/// `relative()` for the offset to apply.
pub fn rise_in<E: Styled + IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
    distance: f32,
) -> AnimationElement<E> {
    el.with_animation(id, enter(ms), move |el, d| {
        el.opacity(d).top(px(distance * (1.0 - d)))
    })
}

/// WhatsApp's bubble entrance: a quick fade while the bubble settles up into
/// place (WhatsApp also grows it from about 97%; gpui cannot scale a div, so
/// the settle carries the motion). No overshoot. The element must be `relative()`.
pub fn bubble_in<E: Styled + IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
    rise: f32,
) -> AnimationElement<E> {
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(ms)).with_easing(ease_out_cubic),
        move |el, d| el.opacity(d).top(px(rise * (1.0 - d))),
    )
}

/// The reverse of [`bubble_in`]: fade out while sinking `distance` px.
pub fn pop_out<E: Styled + IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
    distance: f32,
) -> AnimationElement<E> {
    el.with_animation(id, enter(ms), move |el, d| {
        el.opacity((1.0 - d).clamp(0.0, 1.0)).top(px(distance * d))
    })
}

/// Entrance for a full-window modal backdrop that centres its card: the
/// backdrop fades in and the card (centred by the backdrop's flex) drifts up
/// by half of the padding it starts with.
pub fn modal_in<E: Styled + IntoElement + 'static>(
    backdrop: E,
    id: impl Into<ElementId>,
) -> AnimationElement<E> {
    backdrop.with_animation(id, enter(BASE_MS), |el, d| {
        el.opacity(d).pt(px(24.0 * (1.0 - d)))
    })
}

/// A loader icon that turns continuously (one revolution per second).
pub fn spinning(icon: Icon) -> AnimationElement<Icon> {
    icon.with_animation(
        "spin",
        Animation::new(Duration::from_millis(1000)).repeat(),
        |icon, d| icon.transform(Transformation::rotate(percentage(d))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_hits_endpoints_and_is_monotonic() {
        for f in [ease_out, ease_out_cubic] {
            assert_eq!(f(0.0), 0.0);
            assert_eq!(f(1.0), 1.0);
            let mut prev = 0.0;
            for i in 1..=20 {
                let v = f(i as f32 / 20.0);
                assert!(v >= prev);
                prev = v;
            }
        }
    }

    #[test]
    fn color_lerp_endpoints() {
        let a = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 };
        let b = Hsla { h: 0.5, s: 1.0, l: 0.5, a: 1.0 };
        assert_eq!(lerp_color(a, b, 1.0).a, 1.0);
        assert_eq!(lerp_color(a, b, 0.0).a, 0.0);
    }
}
