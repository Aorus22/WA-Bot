use crate::views::chat::*;

impl ChatView {
    /// WhatsApp's typing bubble above the composer: three pulsing dots, with the
    /// typist's avatar in a group.
    pub(crate) fn render_typing_indicator(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chat_id = self.selected_chat_id.as_deref()?;
        if !cx.has_global::<ChatStore>() {
            return None;
        }
        let typers = ChatStore::global(cx).typers(chat_id);
        let (jid, name, _) = typers.first()?.clone();
        let is_group = chat_id.ends_with("@g.us");

        let theme = *cx.app_theme();
        let is_dark = theme.background.l < 0.5;
        let bubble_bg: Hsla = match theme.chat {
            Some(chat) => chat.bubble_in.into(),
            None if is_dark => rgb(0x202c33).into(),
            None => rgb(0xffffff).into(),
        };
        let base_url = Self::compose_base_url(cx);

        let dots = (0..3usize).map(|index| {
            div()
                .id(SharedString::from(format!("typing-dot-{index}")))
                .size(px(7.0))
                .rounded_full()
                .bg(theme.primary)
                .with_animation(
                    SharedString::from(format!("typing-dot-pulse-{index}")),
                    Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                    move |dot, delta| {
                        // Each dot peaks a little after the one before it.
                        let phase = (delta + (2 - index) as f32 * 0.18) % 1.0;
                        let level = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
                        dot.opacity(0.35 + 0.65 * level)
                    },
                )
        });

        Some(
            h_flex()
                .px_4()
                .pb_2()
                .pt_1()
                .gap_2()
                .items_end()
                .children(is_group.then(|| render_avatar(&jid, &name, "", false, 28.0, &base_url, &theme)))
                .child(
                    h_flex()
                        .px_3()
                        .py(px(11.0))
                        .gap(px(5.0))
                        .items_center()
                        .rounded_2xl()
                        .bg(bubble_bg)
                        .shadow_sm()
                        .children(dots),
                )
                .into_any_element(),
        )
    }
}
