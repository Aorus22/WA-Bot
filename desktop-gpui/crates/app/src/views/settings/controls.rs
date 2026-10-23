//! Settings-only layout helpers shared by its local sections.

use gpui::*;

use crate::components::ui::form_row;
use crate::theme::shape::{ShapeTokens, TextRoleTokens};

#[derive(Clone, Copy)]
pub struct Palette {
    pub shape: ShapeTokens,
    pub typography: TextRoleTokens,
    pub text: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub secondary: Hsla,
    pub tag: Hsla,
    pub border: Hsla,
}

pub fn header(
    p: Palette,
    title: &'static str,
    description: &'static str,
    extra: Option<String>,
    action: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .justify_between()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.text).child(title))
                .child(div().text_size(rems(p.typography.caption_size)).text_color(p.muted).child(description))
                .children(extra.map(|e| div().text_size(rems(p.typography.caption_size)).text_color(p.muted).child(e))),
        )
        .child(div().flex_none().child(action))
}

pub fn option_row(p: Palette, label: &'static str, hint: String, control: impl IntoElement) -> Div {
    let label = div()
        .text_size(rems(p.typography.caption_size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text)
        .child(label);
    let hint = div()
        .text_size(rems(p.typography.caption_size))
        .text_color(p.muted)
        .child(hint);
    form_row(label.into_any_element(), Some(hint.into_any_element()), control.into_any_element())
}

pub fn progress_bar(p: Palette, segments: &[(f32, Hsla)]) -> Div {
    let mut bar = div().w_full().h(px(6.0)).flex().flex_row().rounded_full().bg(p.tag).overflow_hidden();
    for (share, color) in segments {
        bar = bar.child(div().h_full().w(relative(*share)).bg(*color));
    }
    bar
}
