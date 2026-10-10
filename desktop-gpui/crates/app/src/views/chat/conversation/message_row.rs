use crate::views::chat::*;

impl ChatView {
    /// One row of the message list: optional day separator, the message body and its entrance animation.
    /// Also triggers loading older/newer pages when the row index nears either end.
    pub(crate) fn render_message_row(
        this: &mut ChatView,
        index: usize,
        window: &mut Window,
        cx: &mut Context<ChatView>,
    ) -> AnyElement {
        let theme = *cx.app_theme();
        let muted_text = theme.muted_foreground;
        let _text_color = theme.foreground;

        let is_group_chat = this.selected_chat_id.as_ref().map(|cid| {
            cid.ends_with("@g.us") || if cx.has_global::<ChatStore>() {
                ChatStore::global(cx).chats.iter().find(|c| c.id == *cid).map(|c| c.is_group).unwrap_or(false)
            } else {
                false
            }
        }).unwrap_or(false);

        // Auto-load older messages when user scrolls near top
        if index <= 2 && this.has_more && !this.is_loading_more && !this.is_loading_messages {
            this.load_older_messages(cx);
        }
        // Auto-load newer messages when user scrolls near bottom
        if index + 3 >= this.cached_render_messages.len() && this.has_more_next && !this.is_loading_newer && !this.is_loading_messages {
            this.load_newer_messages(cx);
        }

        let body: AnyElement = if index == 0 {
            let first_label = this
                .date_label_at(1)
                .unwrap_or_else(|| "TODAY".to_string());
            h_flex()
                .w_full()
                .justify_center()
                .py_3()
                .child(Self::day_pill(
                    first_label,
                    theme.muted.opacity(0.4),
                    rgba(0x00000000).into(),
                    muted_text,
                ))
                .into_any_element()
        } else {
            Self::render_message_bubble(this, index, window, cx, is_group_chat)
        };

        // Day separator: shown above the first message of
        // each day, like the web's date pill.
        let day_separator: Option<AnyElement> = if index > 1 {
            let current = this
                .cached_render_messages
                .get(index - 1)
                .map(|m| MessageBubbleHelper::day_index(m.msg.timestamp));
            let previous = this
                .cached_render_messages
                .get(index - 2)
                .map(|m| MessageBubbleHelper::day_index(m.msg.timestamp));
            match (current, previous) {
                (Some(cur), Some(prev)) if cur != prev => this
                    .date_label_at(index)
                    .map(|label| {
                        h_flex()
                            .w_full()
                            .justify_center()
                            .py_3()
                            .child(Self::day_pill(
                                label,
                                theme.muted.opacity(0.4),
                                rgba(0x00000000).into(),
                                muted_text,
                            ))
                            .into_any_element()
                    }),
                _ => None,
            }
        } else {
            None
        };

        let row = v_flex()
            .w_full()
            .relative()
            .children(day_separator)
            .child(body);

        let fresh_id = index
            .checked_sub(1)
            .and_then(|i| this.cached_render_messages.get(i))
            .map(|m| m.msg.id.as_str())
            .filter(|id| {
                this.fresh_msgs.get(*id).is_some_and(|at| {
                    at.elapsed() < FRESH_MSG_TTL
                })
            });
        match fresh_id {
            Some(id) => crate::components::motion::rise_in(
                row,
                SharedString::from(format!("msg-in-{id}")),
                MSG_ENTER_MS,
                12.0,
            )
            .into_any_element(),
            None => row.into_any_element(),
        }
    }
}
