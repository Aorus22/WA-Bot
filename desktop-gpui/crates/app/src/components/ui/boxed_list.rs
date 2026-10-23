//! Reusable GTK-style container for related rows.

use gpui::{div, px, AnyElement, App, Div, IntoElement, ParentElement, Styled};
use gpui_component::v_flex;

use super::card::card_group;
use crate::theme::shape::ShapeExt;
use crate::theme::surface::SurfaceExt;

/// Wrap rows in the GTK boxed-list surface while leaving the existing layout
/// untouched when GTK style is disabled.
pub fn boxed_list<T>(cx: &App, rows: impl IntoIterator<Item = T>) -> Div
where
    T: IntoElement + Styled + 'static,
{
    let shape = cx.shape();
    let surface = cx.surface();
    let rows: Vec<T> = rows.into_iter().collect();
    let row_count = rows.len();
    let mut children: Vec<AnyElement> = Vec::with_capacity(row_count.saturating_mul(2).saturating_sub(1));

    for (index, row) in rows.into_iter().enumerate() {
        if shape.styled && index > 0 {
            children.push(
                div()
                    .w_full()
                    .h(px(shape.list_separator_size))
                    .bg(surface.card_shade)
                    .into_any_element(),
            );
        }

        let row = if shape.styled {
            let top_radius = if index == 0 { shape.radius_group } else { shape.radius_flat };
            let bottom_radius = if index + 1 == row_count { shape.radius_group } else { shape.radius_flat };
            row.min_h(px(shape.list_row_min_height))
                .px(px(shape.list_row_padding_x))
                .py(px(shape.list_row_padding_y))
                .rounded_tl(px(top_radius))
                .rounded_tr(px(top_radius))
                .rounded_bl(px(bottom_radius))
                .rounded_br(px(bottom_radius))
        } else {
            row
        };
        children.push(row.into_any_element());
    }

    let list = if shape.styled {
        card_group(cx)
    } else {
        v_flex().w_full()
    };

    list.children(children)
}
