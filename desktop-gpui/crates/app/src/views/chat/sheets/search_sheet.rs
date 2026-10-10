use crate::views::chat::*;

impl ChatView {
    /// Play the Search drawer's exit animation, then unmount it.
    pub fn close_search_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.is_search_sheet_open || self.search_sheet_closing {
            return;
        }
        self.search_sheet_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Re-opening during the exit cancels the pending unmount.
            if this.search_sheet_closing {
                this.is_search_sheet_open = false;
                this.search_sheet_closing = false;
            }
        });
        cx.notify();
    }

    /// Toggle Chat Search Sheet drawer (Right side)
    pub fn toggle_search_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_search_sheet_open && !self.search_sheet_closing {
            self.close_search_sheet(cx);
            return;
        }
        self.search_sheet_closing = false;
        self.is_search_sheet_open = true;
        self.is_info_sheet_open = false;
        self.info_sheet_closing = false;
        self.search_sheet_focus.focus(window, cx);
        if !self.search_sheet_query.is_empty() {
            self.run_search_messages(cx);
        }
        cx.notify();
    }

    /// Run message search both locally (immediate) and remotely (via HttpClient)
    pub fn run_search_messages(&mut self, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let query = self.search_sheet_query.trim().to_string();
        if query.is_empty() {
            self.search_sheet_results.clear();
            self.is_searching_messages = false;
            cx.notify();
            return;
        }

        let q_lower = query.to_lowercase();

        // 1. Instant local results from current cached render messages & ChatStore
        let mut local_results: Vec<Message> = Vec::new();
        let mut seen_ids = HashSet::new();

        for rm in &self.cached_render_messages {
            let content_lower = rm.content.to_lowercase();
            if content_lower.contains(&q_lower) && seen_ids.insert(rm.msg.id.clone()) {
                local_results.push(rm.msg.clone());
            }
        }

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global(cx);
            if let Some(history) = store.messages_by_chat.get(&chat_id) {
                for m in &history.messages {
                    let decoded = MessageBubbleHelper::decode_content(&m.content).to_lowercase();
                    if decoded.contains(&q_lower) && seen_ids.insert(m.id.clone()) {
                        local_results.push(m.clone());
                    }
                }
            }
        }

        // Sort local results newest first
        local_results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.search_sheet_results = local_results;
        self.is_searching_messages = true;
        cx.notify();

        // 2. Fetch server messages via HttpClient::search_messages
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let cid = chat_id.clone();
        let q_fetch = query.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_async = cid.clone();
            let q_async = q_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.search_messages(&cid_async, &q_async, Some(50)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if this.search_sheet_query.trim() == q_fetch.trim() {
                                this.is_searching_messages = false;
                                if let Ok(Ok(server_msgs)) = res {
                                    let mut combined = this.search_sheet_results.clone();
                                    let mut seen: HashSet<String> = combined.iter().map(|m| m.id.clone()).collect();
                                    for sm in server_msgs {
                                        if seen.insert(sm.id.clone()) {
                                            combined.push(sm);
                                        }
                                    }
                                    combined.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                                    this.search_sheet_results = combined;
                                }
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Jump/teleport directly to a specific message in the conversation
    pub fn navigate_to_message(&mut self, message_id: &str, cx: &mut Context<Self>) {
        // Jumping away needs the drawer gone now, not sliding out of the way.
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.highlighted_message_id = Some(message_id.to_string());
        let mid_flash = message_id.to_string();

        // Clear highlight after 2.5 seconds
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let mid_clone = mid_flash.clone();
            async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if this.highlighted_message_id.as_deref() == Some(&mid_clone) {
                                this.highlighted_message_id = None;
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();

        // If message is already loaded in cached_render_messages
        if let Some(pos) = self.cached_render_messages.iter().position(|m| m.msg.id == message_id) {
            self.messages_list_state.scroll_to_reveal_item(pos + 1);
            cx.notify();
            return;
        }

        // If not in local cache, fetch message context around it from backend
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_mid = message_id.to_string();
        let cid = chat_id.clone();
        self.is_loading_messages = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_async = cid.clone();
            let target_mid_async = target_mid.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_message_context(&cid_async, &target_mid_async, Some(50)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_messages = false;
                            if let Ok(Ok(mut msgs)) = res {
                                if !msgs.is_empty() {
                                    msgs.sort_by_key(|m| m.timestamp);
                                    this.has_more = true;
                                    this.has_more_next = true;
                                    this.rebuild_message_cache(&cid, &msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid, msgs, true);
                                    }
                                    if let Some(pos) = this.cached_render_messages.iter().position(|m| m.msg.id == target_mid) {
                                        this.messages_list_state.scroll_to_reveal_item(pos + 1);
                                    }
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Format unix epoch timestamp to readable search date (e.g. "10 Sep 2026")
    pub(crate) fn format_search_date(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let mut days = crate::localtime::local_day(timestamp) as i32;
        if days < 0 {
            return String::new();
        }
        let mut year = 1970;
        loop {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            let days_in_year = if leap { 366 } else { 365 };
            if days >= days_in_year {
                days -= days_in_year;
                year += 1;
            } else {
                break;
            }
        }
        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_months = [
            31, if leap { 29 } else { 28 }, 31, 30, 31, 30,
            31, 31, 30, 31, 30, 31,
        ];
        let month_names = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let mut month = 0;
        for (m_idx, &dim) in days_in_months.iter().enumerate() {
            if days >= dim {
                days -= dim;
            } else {
                month = m_idx;
                break;
            }
        }
        let day = days + 1;
        format!("{} {} {}", day, month_names[month], year)
    }

    /// Format unix epoch timestamp to readable search time (e.g. "23:26")
    pub(crate) fn format_search_time(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let (hours, mins) = crate::localtime::hour_minute(timestamp);
        format!("{:02}:{:02}", hours, mins)
    }

    /// Render message search sheet drawer (Right side, matching Web / GTK 1:1)
    pub(crate) fn render_search_sheet(
        &self,
        _chat: &Chat,
        theme: ActiveTokens,
        border_color: Hsla,
        bg_color: Hsla,
        _card_bg: Hsla,
        primary_color: Hsla,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w(px(SHEET_WIDTH))
            .min_w(px(SHEET_WIDTH))
            .max_w(px(SHEET_WIDTH))
            .h_full()
            .overflow_hidden()
            .border_l_1()
            .border_color(border_color.opacity(0.4))
            .bg(bg_color)
            .flex_shrink_0()
            // Header
            .child(
                v_flex()
                    .border_b_1()
                    .border_color(border_color.opacity(0.4))
                    .bg(theme.muted.opacity(0.2))
                    // Header Title row
                    .child(
                        h_flex()
                            .px_4()
                            .py_3()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(svg().data(SEARCH_SVG).size(px(18.0)).text_color(primary_color))
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(text_color)
                                            .child("Search Messages"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-close-search-sheet")
                                    .cursor_pointer()
                                    .p_1()
                                    .rounded_full()
                                    .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Close").build(window, cx)
                                    })
                                    .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))                                     .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                         this.close_search_sheet(cx);
                                     })),
                            ),
                    )
                    // Search Input Box
                    .child(
                        div()
                            .px_3()
                            .pb_3()
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .bg(theme.muted.opacity(0.5))
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(border_color.opacity(0.3))
                                    .items_center()
                                    .gap_2()
                                    .cursor_text()
                                    .track_focus(&self.search_sheet_focus)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.search_sheet_focus.focus(window, cx);
                                    }))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => {
                                                this.search_sheet_query.pop();
                                                this.run_search_messages(cx);
                                            }
                                            "escape" => {
                                                if !this.search_sheet_query.is_empty() {
                                                    this.search_sheet_query.clear();
                                                    this.search_sheet_results.clear();
                                                    this.is_searching_messages = false;
                                                } else {
                                                    this.close_search_sheet(cx);
                                                }
                                                cx.notify();
                                            }
                                            "space" => {
                                                this.search_sheet_query.push(' ');
                                                this.run_search_messages(cx);
                                            }
                                            "enter" => {
                                                this.run_search_messages(cx);
                                            }
                                            k if k.len() == 1 => {
                                                this.search_sheet_query.push_str(k);
                                                this.run_search_messages(cx);
                                            }
                                            _ => {}
                                        }
                                    }))
                                    .child(
                                        svg()
                                            .data(SEARCH_SVG)
                                            .size(px(14.0))
                                            .text_color(muted_text),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .cursor_text()
                                            .text_xs()
                                            .text_color(if self.search_sheet_query.is_empty() { muted_text } else { text_color })
                                            .child(if self.search_sheet_query.is_empty() {
                                                "Search messages...".to_string()
                                            } else {
                                                self.search_sheet_query.clone()
                                            }),
                                    )
                                    .children(if !self.search_sheet_query.is_empty() {
                                        Some(
                                            div()
                                                .id("btn-clear-search-sheet")
                                                .cursor_pointer()
                                                .p_1()
                                                .rounded_full()
                                                .hover(|s| s.bg(theme.muted.opacity(0.65)))
                                                .tooltip(move |window, cx| {
                                                    Tooltip::new("Clear search").build(window, cx)
                                                })
                                                .child(svg().data(X_SVG).size(px(12.0)).text_color(muted_text))
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.search_sheet_query.clear();
                                                    this.search_sheet_results.clear();
                                                    this.is_searching_messages = false;
                                                    cx.notify();
                                                })),
                                        )
                                    } else {
                                        None
                                    }),
                            ),
                    ),
            )
            // Body Content
            .child(
                if self.is_searching_messages && self.search_sheet_results.is_empty() {
                    // Loading state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(crate::components::motion::spinning(Icon::new(IconName::LoaderCircle).size(px(24.0))))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_text)
                                .child("Searching history..."),
                        )
                        .into_any_element()
                } else if !self.search_sheet_results.is_empty() {
                    // Results list
                    v_flex()
                        .id("search-sheet-results-list")
                        .flex_1()
                        .overflow_y_scroll()
                        .p_2()
                        .gap_1()
                        .children(self.search_sheet_results.iter().map(|m| {
                            let msg_id = m.id.clone();
                            let is_me = m.from == "me" || MessageBubbleHelper::is_from_me(m, "me");
                            let sender = if is_me {
                                "You".to_string()
                            } else {
                                m.sender_name.clone().unwrap_or_else(|| {
                                    m.from.split('@').next().unwrap_or(&m.from).to_string()
                                })
                            };

                            let decoded = MessageBubbleHelper::decode_content(&m.content);
                            let snippet = if decoded.trim().is_empty() {
                                format!("[{}]", if m.message_type.is_empty() { "Message" } else { &m.message_type })
                            } else {
                                decoded
                            };

                            let date_str = Self::format_search_date(m.timestamp);
                            let time_str = Self::format_search_time(m.timestamp);

                            v_flex()
                                .id(SharedString::from(format!("search-result-{}", msg_id)))
                                .cursor_pointer()
                                .p_3()
                                .rounded_xl()
                                .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                .border_b_1()
                                .border_color(border_color.opacity(0.15))
                                .gap_1()
                                .tooltip(move |window, cx| {
                                    Tooltip::new("Click to jump to message").build(window, cx)
                                })
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.navigate_to_message(&msg_id, cx);
                                }))
                                // Sender name & Date
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(primary_color)
                                                .max_w(px(170.0))
                                                .overflow_hidden()
                                                .child(sender),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted_text)
                                                .child(date_str),
                                        ),
                                )
                                // Snippet
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_color)
                                        .max_h(px(36.0))
                                        .overflow_hidden()
                                        .child(snippet),
                                )
                                // Time
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_text.opacity(0.7))
                                                .child(time_str),
                                        ),
                                )
                                .into_any_element()
                        }))
                        .into_any_element()
                } else if !self.search_sheet_query.trim().is_empty() {
                    // Empty results state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .gap_3()
                        .child(
                            div()
                                .w(px(56.0))
                                .h(px(56.0))
                                .rounded_2xl()
                                .bg(theme.muted.opacity(0.3))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg().data(MESSAGE_SQUARE_SVG).size(px(26.0)).text_color(muted_text.opacity(0.7))),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child("No results found"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_text)
                                .text_center()
                                .child(format!("We couldn't find any messages matching \"{}\"", self.search_sheet_query)),
                        )
                        .into_any_element()
                } else {
                    // Initial prompt state
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .gap_3()
                        .child(
                            div()
                                .w(px(56.0))
                                .h(px(56.0))
                                .rounded_2xl()
                                .bg(theme.muted.opacity(0.3))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg().data(SEARCH_SVG).size(px(26.0)).text_color(muted_text.opacity(0.7))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted_text)
                                .child("Search for keywords, dates, or phrases"),
                        )
                        .into_any_element()
                },
            )
            .into_any_element()
    }

}
