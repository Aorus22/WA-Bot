//! Content container with a fixed width-to-height ratio.
use gpui::{Div, IntoElement, ParentElement, Styled, div};

pub fn aspect_ratio(content: impl IntoElement, ratio: f32) -> Div {
    div().w_full().aspect_ratio(ratio).child(content)
}
