use crate::views::chat::*;

impl ChatView {
    /// Fetch all chats from GET /api/chats and populate ChatStore
    pub fn load_chats(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_chats {
            return;
        }
        self.is_loading_chats = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_chats().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_chats = false;
                            if let Ok(Ok(chats)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).set_chats(chats);
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

    /// Select a chat, mark as read, and load its messages
    pub fn select_chat(&mut self, chat_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_chat_id = Some(chat_id.clone());
        // One-to-one typing indicators only arrive for chats we subscribed to;
        // groups send them anyway.
        if !chat_id.ends_with("@g.us") {
            let (base_url, target) = (Self::compose_base_url(cx), chat_id.clone());
            TOKIO_RT.spawn(async move {
                let _ = HttpClient::new(&base_url).subscribe_chat_presence(&target).await;
            });
        }
        self.mention_query = None;
        self.group_info = None;
        self.info_sheet_tab = InfoSheetTab::Media;
        self.reply_to = None;
        self.editing_message = None;
        self.pending_attachment = None;
        self.update_compose_placeholder(window, cx);
        self.context_menu = None;
        self.chat_context_menu = None;
        // Switching conversations cuts both drawers immediately — a lingering
        // exit animation would sit over the wrong thread.
        self.is_info_sheet_open = false;
        self.is_search_sheet_open = false;
        self.info_sheet_closing = false;
        self.search_sheet_closing = false;
        self.search_sheet_query.clear();
        self.search_sheet_results.clear();
        self.is_searching_messages = false;
        self.is_loading_more = false;
        self.is_loading_newer = false;
        self.has_more = true;
        self.has_more_next = false;
        self.highlighted_message_id = None;
        self.messages_list_state.scroll_to_end();

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.active_chat_id = Some(chat_id.clone());
            store.mark_chat_read(&chat_id);
        }

        // Mark as read in backend
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid_clone = chat_id.clone();
        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| {
            async move {
                let client = HttpClient::new(&base_url);
                let _ = TOKIO_RT
                    .spawn(async move { client.mark_as_read(&cid_clone).await })
                    .await;
            }
        })
        .detach();

        self.load_messages(&chat_id, cx);
        if chat_id.ends_with("@g.us") {
            self.load_group_info(&chat_id, cx);
        }
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// Rebuild cached render messages for 60fps smooth scrolling
    pub(crate) fn rebuild_message_cache(&mut self, chat_id: &str, msgs: &[Message]) {
        let mut lookup = std::collections::HashMap::new();
        for m in msgs {
            let sender = if m.from == "me" {
                "You".to_string()
            } else {
                m.sender_name.clone().unwrap_or_else(|| "Sender".to_string())
            };
            let decoded = MessageBubbleHelper::decode_content(&m.content);
            let first_line = decoded.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
            lookup.insert(m.id.clone(), (sender, first_line));
        }

        self.cached_chat_id = Some(chat_id.to_string());
        self.cached_render_messages = msgs
            .iter()
            .map(|m| {
                let is_from_me = MessageBubbleHelper::is_from_me(m, "me");
                let content = MessageBubbleHelper::decode_content(&m.content);
                let time_str = Self::format_chat_time(m.timestamp);
                let ticks = MessageBubbleHelper::ticks(m, is_from_me);
                let quoted = m.reply_to_id.as_ref().and_then(|rid| lookup.get(rid).cloned());

                RenderMessage {
                    msg: m.clone(),
                    is_from_me,
                    content,
                    time_str,
                    ticks,
                    quoted,
                }
            })
            .collect();

        let total_items = if self.cached_render_messages.is_empty() { 0 } else { self.cached_render_messages.len() + 1 };
        self.messages_list_state.reset(total_items);
        self.messages_list_state.scroll_to_end();
    }

    /// Flag messages appended to the open conversation since the last build so
    /// their rows can ease in. Only a small tail append counts: a prepended
    /// history page, a swapped optimistic bubble (an id disappears) or a
    /// different chat is not "new".
    pub(crate) fn mark_fresh_messages(&mut self, chat_id: &str, msgs: &[Message]) {
        const MAX_FRESH: usize = 3;
        let now = std::time::Instant::now();
        self.fresh_msgs
            .retain(|_, at| now.duration_since(*at) < FRESH_MSG_TTL);
        if self.cached_chat_id.as_deref() != Some(chat_id) || self.cached_render_messages.is_empty() {
            return;
        }
        let old_ids: HashSet<&str> = self
            .cached_render_messages
            .iter()
            .map(|m| m.msg.id.as_str())
            .collect();
        let new_ids: HashSet<&str> = msgs.iter().map(|m| m.id.as_str()).collect();
        if old_ids.iter().any(|id| !new_ids.contains(id)) {
            return;
        }
        let added = msgs.iter().filter(|m| !old_ids.contains(m.id.as_str())).count();
        if added == 0 || added > MAX_FRESH {
            return;
        }
        // Must be a tail append, not messages slotted in earlier.
        let tail_start = msgs.len().saturating_sub(added);
        if msgs[..tail_start].iter().any(|m| !old_ids.contains(m.id.as_str())) {
            return;
        }
        for m in &msgs[tail_start..] {
            self.fresh_msgs.insert(m.id.clone(), now);
        }
    }

    /// Rebuild cached render messages without jumping to bottom (preserves scroll position)
    pub(crate) fn rebuild_message_cache_preserve_scroll(&mut self, chat_id: &str, msgs: &[Message]) {
        self.mark_fresh_messages(chat_id, msgs);
        let old_ids: Vec<String> = self.cached_render_messages.iter().map(|m| m.msg.id.clone()).collect();
        let new_ids: Vec<String> = msgs.iter().map(|m| m.id.clone()).collect();
        let mut lookup = std::collections::HashMap::new();
        for m in msgs {
            let sender = if m.from == "me" {
                "You".to_string()
            } else {
                m.sender_name.clone().unwrap_or_else(|| "Sender".to_string())
            };
            let decoded = MessageBubbleHelper::decode_content(&m.content);
            let first_line = decoded.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
            lookup.insert(m.id.clone(), (sender, first_line));
        }

        self.cached_chat_id = Some(chat_id.to_string());
        self.cached_render_messages = msgs
            .iter()
            .map(|m| {
                let is_from_me = MessageBubbleHelper::is_from_me(m, "me");
                let content = MessageBubbleHelper::decode_content(&m.content);
                let time_str = Self::format_chat_time(m.timestamp);
                let ticks = MessageBubbleHelper::ticks(m, is_from_me);
                let quoted = m.reply_to_id.as_ref().and_then(|rid| lookup.get(rid).cloned());

                RenderMessage {
                    msg: m.clone(),
                    is_from_me,
                    content,
                    time_str,
                    ticks,
                    quoted,
                }
            })
            .collect();

        let total_items = if self.cached_render_messages.is_empty() { 0 } else { self.cached_render_messages.len() + 1 };

        let old_items = self.messages_list_state.item_count();

        // Splice only the changed message range. ListState adjusts the anchor
        // when rows are appended or prepended, so a tail follower stays at the
        // bottom and someone reading older messages stays at the same row.
        if old_items == 0 {
            if total_items > 0 {
                self.messages_list_state.splice(0..0, total_items);
            }
        } else if total_items == 0 {
            self.messages_list_state.splice(0..old_items, 0);
        } else {
            let mut prefix = 0;
            while prefix < old_ids.len().min(new_ids.len()) && old_ids[prefix] == new_ids[prefix] {
                prefix += 1;
            }
            let mut suffix = 0;
            while suffix < old_ids.len().saturating_sub(prefix).min(new_ids.len().saturating_sub(prefix))
                && old_ids[old_ids.len() - 1 - suffix] == new_ids[new_ids.len() - 1 - suffix]
            {
                suffix += 1;
            }

            let old_start = prefix + 1; // row zero is the in-flow date pill
            let old_end = old_ids.len() - suffix + 1;
            let new_count = new_ids.len() - prefix - suffix;
            if old_start != old_end || new_count != 0 {
                self.messages_list_state.splice(old_start..old_end, new_count);
            }
        }
        if total_items > 0 {
            self.messages_list_state.remeasure_items(0..total_items);
        }
    }

    /// Fetch messages for a specific chat from GET /api/chats/:id/messages
    pub fn load_messages(&mut self, chat_id: &str, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.to_string();
        self.is_loading_messages = true;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let cid_fetch = cid.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_fetch, Some(100), None, None).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_messages = false;
                            if let Ok(Ok(msgs)) = res {
                                this.has_more = msgs.len() >= 100;
                                this.has_more_next = false;
                                this.rebuild_message_cache(&cid, &msgs);
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).set_messages(&cid, msgs, this.has_more);
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

    /// Fetch the open chat's latest messages again without resetting the view:
    /// the store's version bump makes the next render rebuild its cache in
    /// place. Used when the backend re-resolves names.
    pub(crate) fn refresh_messages_in_place(&mut self, chat_id: &str, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.to_string();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let cid_fetch = cid.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_fetch, Some(100), None, None).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let (Ok(Ok(msgs)), Some(view)) = (res, this.upgrade()) {
                        let has_more = msgs.len() >= 100;
                        if cx.has_global::<ChatStore>() {
                            ChatStore::global_mut(cx).set_messages(&cid, msgs, has_more);
                        }
                        view.update(cx, |_, cx| cx.notify());
                    }
                });
            }
        })
        .detach();
    }

    /// Load older messages when scrolling near the top (paginating backwards)
    pub fn load_older_messages(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_more || !self.has_more {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let oldest_ts = self.cached_render_messages.first().map(|m| m.msg.timestamp).unwrap_or(0);
        if oldest_ts == 0 {
            return;
        }
        self.is_loading_more = true;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_fetch = cid.clone();
            let cid_async = cid_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_async, Some(40), Some(oldest_ts), None).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_more = false;
                            if let Ok(Ok(older_msgs)) = res {
                                if older_msgs.is_empty() {
                                    this.has_more = false;
                                } else {
                                    this.has_more = older_msgs.len() >= 40;
                                    let mut all_msgs = older_msgs;
                                    for rm in &this.cached_render_messages {
                                        all_msgs.push(rm.msg.clone());
                                    }
                                    all_msgs.sort_by_key(|m| m.timestamp);
                                    all_msgs.dedup_by(|a, b| a.id == b.id);
                                    this.rebuild_message_cache_preserve_scroll(&cid_fetch, &all_msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid_fetch, all_msgs, this.has_more);
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

    /// Load newer messages when scrolling near the bottom (after jumping to an older historical message)
    pub fn load_newer_messages(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_newer || !self.has_more_next {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };
        let newest_ts = self.cached_render_messages.last().map(|m| m.msg.timestamp).unwrap_or(0);
        if newest_ts == 0 {
            return;
        }
        self.is_loading_newer = true;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let cid = chat_id.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            let cid_fetch = cid.clone();
            let cid_async = cid_fetch.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.get_messages(&cid_async, Some(40), None, Some(newest_ts)).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_loading_newer = false;
                            if let Ok(Ok(newer_msgs)) = res {
                                if newer_msgs.is_empty() {
                                    this.has_more_next = false;
                                } else {
                                    this.has_more_next = newer_msgs.len() >= 40;
                                    let mut all_msgs: Vec<Message> = this.cached_render_messages.iter().map(|rm| rm.msg.clone()).collect();
                                    all_msgs.extend(newer_msgs);
                                    all_msgs.sort_by_key(|m| m.timestamp);
                                    all_msgs.dedup_by(|a, b| a.id == b.id);
                                    this.rebuild_message_cache_preserve_scroll(&cid_fetch, &all_msgs);
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).set_messages(&cid_fetch, all_msgs, this.has_more);
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

    pub(crate) fn load_group_info(&mut self, group_id: &str, cx: &mut Context<Self>) {
        let group_id = group_id.to_string();
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        self.is_loading_info_members = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let result = TOKIO_RT.spawn(async move { client.get_group(&group_id).await }).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(info)) = result {
                                this.group_info = Some(info);
                            }
                            this.is_loading_info_members = false;
                            cx.notify();
                        });
                    }
                });
            }
        }).detach();
    }

}
