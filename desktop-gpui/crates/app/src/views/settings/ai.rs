//! AI section of the Settings page: pick a provider and fill in what it needs.

use gpui::*;
use crate::theme::shape::ShapeExt;

use crate::components::ui::{
    card_group, Icon, IconName, dropdown_menu::{dropdown_list, DropdownMenuStyle},
};
use super::{settings_section_heading, EditField, SettingsView};
use crate::theme::manager::AppThemeExt;

const PROVIDERS: [(&str, &str); 3] = [
    ("gemini", "Gemini"),
    ("openai", "OpenAI-compatible API"),
    ("claude", "Claude-compatible API"),
];

fn provider_label(id: &str) -> &'static str {
    PROVIDERS.iter().find(|(p, _)| *p == id).map_or("Gemini", |(_, label)| *label)
}

impl SettingsView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_ai_section(
        &mut self,
        cx: &mut Context<Self>,
        text_color: Hsla,
        muted_text: Hsla,
        primary_color: Hsla,
        secondary_bg: Hsla,
        tag_bg: Hsla,
        border_color: Hsla,
    ) -> Div {
        let dropdown_style = DropdownMenuStyle::from_theme(cx);
        let theme = *cx.app_theme();
        let provider = self.ai_provider.clone();
        let is_gemini = provider == "gemini";

        // One editable value: label and current value on the left, an edit button on the right.
        let field = |cx: &mut Context<Self>, id: &'static str, label: &'static str, value: String, button: &'static str, edit: EditField| {
            div()
                .px_4()
                .py_3()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child(label))
                        .child(div().text_xs().text_color(muted_text).child(value)),
                )
                .child(
                    div()
                        .id(id)
                        .flex_none()
                        .px_3()
                        .py_1()
                        .rounded(px(cx.shape().radius_control))
                        .bg(tag_bg)
                        .border_1()
                        .border_color(border_color)
                        .cursor_pointer()
                        .hover(|s| s.bg(secondary_bg))
                        .text_xs()
                        .text_color(text_color)
                        .child(button)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                            this.open_edit_modal(edit.clone(), window, cx);
                        })),
                )
        };
        let divider = || div().h(px(1.0)).bg(border_color).w_full();
        let key_state = |set: bool| if set { "•••••••••••••••• (set)" } else { "Not set" };
        let or_dash = |v: &str, empty: &str| if v.trim().is_empty() { empty.to_string() } else { v.to_string() };

        let mut card = card_group(cx)
            // Provider
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(Icon::new(IconName::Bot).text_color(muted_text))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(text_color).child("AI provider"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted_text)
                                            .child("Used by scripts and the script assistant"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .w(px(200.0))
                            .child(div()
                            .id("ai-provider-button")
                            .w(px(200.0))
                            .h(px(32.0))
                            .px_3()
                            .rounded(px(cx.shape().radius_control))
                            .border_1()
                            .border_color(border_color)
                            .bg(tag_bg)
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .child(div().text_xs().text_color(text_color).child(provider_label(&provider)))
                            .child(Icon::new(IconName::ChevronDown).text_color(muted_text))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.show_ai_provider_picker = !this.show_ai_provider_picker;
                                this.show_tts_provider_picker = false;
                                cx.notify();
                            })),)
                            .children(self.show_ai_provider_picker.then(|| {
                                dropdown_list(
                                    dropdown_style,
                                    200.0,
                                    PROVIDERS.into_iter().map(|(id, label)| {
                                        let selected = provider == id;
                                        div()
                                            .px_3()
                                            .py_1p5()
                                            .text_xs()
                                            .text_color(if selected { primary_color } else { text_color })
                                            .cursor_pointer()
                                            .hover(|s| s.bg(tag_bg))
                                            .child(label)
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.ai_provider = id.to_string();
                                                this.show_ai_provider_picker = false;
                                                cx.notify();
                                            }))
                                    }),
                                )
                            })),
                    ),
            )
            .child(divider());

        if is_gemini {
            card = card.child(field(
                cx,
                "ai-gemini-key",
                "Gemini API key",
                key_state(self.has_gemini_key).to_string(),
                "Edit key",
                EditField::GeminiApiKey,
            ));
        } else {
            let default_base = if provider == "claude" { "https://api.anthropic.com" } else { "https://api.openai.com/v1" };
            card = card
                .child(field(
                    cx,
                    "ai-base-url",
                    "API base URL",
                    or_dash(&self.ai_base_url, &format!("{default_base} (default)")),
                    "Edit URL",
                    EditField::AiBaseUrl,
                ))
                .child(divider())
                .child(field(cx, "ai-api-key", "API key", key_state(self.has_ai_key).to_string(), "Edit key", EditField::AiApiKey))
                .child(divider())
                .child(field(cx, "ai-model", "Model", or_dash(&self.ai_model, "Not set"), "Edit model", EditField::AiModel));
        }

        card = card
            .child(divider())
            .child(field(cx, "ai-server-url", "AI server URL", self.ai_server_url.clone(), "Edit URL", EditField::AiServerUrl))
            .child(divider())
            .child(
                div().px_4().py_3().flex().flex_row().justify_end().child(
                    div()
                        .px_4()
                        .py_1p5()
                        .rounded(px(cx.shape().radius_control))
                        .bg(primary_color)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.primary_foreground)
                                .child(if self.is_saving_ai { "Saving..." } else { "Save" }),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.save_ai_settings(cx))),
                ),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(settings_section_heading(cx.shape(), "AI", "AI", text_color, muted_text))
            .child(card)
    }
}
