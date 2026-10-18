use crate::views::chat::*;

/// Pointed top-left corner of the typing bubble, like WhatsApp's tail.
const TAIL_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="10" viewBox="0 0 8 10"><path d="M8 0H0L8 10Z" fill="currentColor"/></svg>"#;
const TYPING_POP_MS: u64 = 220;
const TYPING_LEAVE_MS: u64 = 160;

impl ChatView {
    /// WhatsApp's typing bubble under the last message: three jumping dots in a
    /// tailed bubble that pops in when someone starts typing and drops out when
    /// they stop (or their message arrives). A group shows the typist's avatar.
    pub(crate) fn render_typing_indicator(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chat_id = self.selected_chat_id.clone()?;
        let current = cx
            .has_global::<ChatStore>()
            .then(|| ChatStore::global(cx).typers(&chat_id).first().cloned())
            .flatten();

        let (jid, name, leaving) = match current {
            Some((jid, name, _)) => {
                let same_chat = self.typing_shown.as_ref().is_some_and(|(c, _, _)| *c == chat_id);
                if !same_chat || self.typing_left_at.is_some() {
                    // A new typing session replays the pop-in.
                    self.typing_session = self.typing_session.wrapping_add(1);
                }
                self.typing_left_at = None;
                self.typing_shown = Some((chat_id.clone(), jid.clone(), name.clone()));
                (jid, name, false)
            }
            None => {
                let (shown_chat, jid, name) = self.typing_shown.clone()?;
                if shown_chat != chat_id {
                    self.typing_shown = None;
                    self.typing_left_at = None;
                    return None;
                }
                let left_at = match self.typing_left_at {
                    Some(at) => at,
                    None => {
                        let now = std::time::Instant::now();
                        self.typing_left_at = Some(now);
                        // Nothing else repaints when the exit animation ends.
                        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                            let cx = cx.clone();
                            async move {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(TYPING_LEAVE_MS + 40))
                                    .await;
                                let _ = cx.update(|cx: &mut App| {
                                    if let Some(view) = this.upgrade() {
                                        view.update(cx, |_, cx| cx.notify());
                                    }
                                });
                            }
                        })
                        .detach();
                        now
                    }
                };
                if left_at.elapsed() >= std::time::Duration::from_millis(TYPING_LEAVE_MS) {
                    self.typing_shown = None;
                    self.typing_left_at = None;
                    return None;
                }
                (jid, name, true)
            }
        };
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
                .relative()
                .size(px(7.0))
                .rounded_full()
                .bg(theme.primary)
                .with_animation(
                    SharedString::from(format!("typing-dot-jump-{index}")),
                    Animation::new(std::time::Duration::from_millis(1100)).repeat(),
                    move |dot, delta| {
                        // The dots jump one after the other, then rest.
                        let phase = (delta - index as f32 * 0.14).rem_euclid(1.0);
                        let bump = if phase < 0.42 { (phase / 0.42 * std::f32::consts::PI).sin() } else { 0.0 };
                        dot.top(px(-4.5 * bump)).opacity(0.5 + 0.5 * bump)
                    },
                )
        });

        let row = h_flex()
            .relative()
            .px_4()
            .pb_2()
            .pt_1()
            .gap(px(if is_group { 12.0 } else { 0.0 }))
            .items_start()
            .children(is_group.then(|| render_avatar(&jid, &name, "", false, 28.0, &base_url, &theme)))
            .child(
                h_flex()
                    .relative()
                    .px_3()
                    .py(px(11.0))
                    .gap(px(5.0))
                    .items_center()
                    .rounded_2xl()
                    .rounded_tl(px(0.0))
                    .bg(bubble_bg)
                    .shadow_sm()
                    .child(
                        div()
                            .absolute()
                            .left(px(-7.0))
                            .top(px(0.0))
                            .child(svg().data(TAIL_SVG).w(px(8.0)).h(px(10.0)).text_color(bubble_bg)),
                    )
                    .children(dots),
            );

        let session = self.typing_session;
        Some(if leaving {
            crate::components::motion::pop_out(row, SharedString::from(format!("typing-out-{session}")), TYPING_LEAVE_MS, 6.0)
                .into_any_element()
        } else {
            crate::components::motion::bubble_in(row, SharedString::from(format!("typing-in-{session}")), TYPING_POP_MS, 8.0)
                .into_any_element()
        })
    }
}
