use crate::views::chat::*;

impl ChatView {
    /// React to message with emoji. Toggles like the web client: sending the
    /// same emoji again removes "my" reaction (backend ApplyReaction treats
    /// the sender list as authoritative).
    pub fn react_message(&mut self, chat_id: String, msg_id: String, emoji: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Optimistic local update, mirroring the web's applyReactionLocal:
        // drop "me" from every entry, re-add under the new emoji.
        let new_reactions = {
            let mut out: Vec<ReactionEntry> = Vec::new();
            let existing = cx
                .has_global::<ChatStore>()
                .then(|| {
                    ChatStore::global(cx)
                        .messages_by_chat
                        .get(&chat_id)
                        .and_then(|e| e.messages.iter().find(|m| m.id == msg_id))
                        .and_then(|m| m.reactions.clone())
                })
                .flatten()
                .unwrap_or_default();
            for r in existing {
                let mut kept: Vec<String> = r.senders.into_iter().filter(|s| s != "me").collect();
                if r.emoji == emoji {
                    kept.push("me".to_string());
                }
                if !kept.is_empty() {
                    out.push(ReactionEntry { emoji: r.emoji, senders: kept });
                }
            }
            if !out.iter().any(|r| r.emoji == emoji) {
                out.push(ReactionEntry { emoji: emoji.clone(), senders: vec!["me".to_string()] });
            }
            out
        };
        if cx.has_global::<ChatStore>() {
            let reactions_clone = new_reactions.clone();
            ChatStore::global_mut(cx).patch_message(&chat_id, &msg_id, move |m| {
                m.reactions = Some(reactions_clone);
            });
        }
        if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == msg_id) {
            rm.msg.reactions = Some(new_reactions.clone());
            // The chips row changes the item's rendered height — keep the
            // list's height summary in sync without moving the scroll.
            self.messages_list_state.remeasure();
        }

        let cid = chat_id.clone();
        let mid = msg_id.clone();
        let em = emoji.clone();

        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| {
            let cx_handle = _cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.react_to_message(&cid, &mid, &em, None).await })
                    .await;

                // On failure, revert is unnecessary: the authoritative
                // `message_reaction` WS broadcast (sent on success only) won't
                // arrive, and a fresh load restores server state. Surface the
                // error like the web client does.
                if !matches!(res, Ok(Ok(_))) {
                    let _ = cx_handle.update(|cx: &mut App| {
                        toast::error("Gagal mengirim reaksi", cx);
                    });
                }
            }
        })
        .detach();

        cx.notify();
    }

    /// Cast (or withdraw, with an empty list) my vote on a poll. The tally
    /// updates at once; the backend echoes the authoritative extra back.
    pub fn vote_poll(&mut self, chat_id: String, msg_id: String, options: Vec<String>, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        if cx.has_global::<ChatStore>() {
            let mine = options.clone();
            ChatStore::global_mut(cx).patch_message(&chat_id, &msg_id, move |m| {
                if let Some(poll) = m.extra.as_mut().and_then(|extra| extra.poll.as_mut()) {
                    let votes = poll.votes.get_or_insert_with(Default::default);
                    if mine.is_empty() {
                        votes.remove("me");
                    } else {
                        votes.insert("me".to_string(), mine);
                    }
                }
            });
        }

        cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.send_poll_vote(&chat_id, &msg_id, &options).await })
                    .await;
                if !matches!(res, Ok(Ok(_))) {
                    let _ = cx.update(|cx: &mut App| {
                        toast::error("Gagal mengirim suara", cx);
                    });
                }
            }
        })
        .detach();
        cx.notify();
    }

    // ========================================================================
    // ATTACH MENU, PICKERS AND COMPOSE DIALOGS (web parity)
    // ========================================================================

    /// Start replying to a message
    pub fn start_reply(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = Some(msg);
        self.editing_message = None;
        self.context_menu = None;
        self.update_compose_placeholder(window, cx);
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// Cancel active reply
    pub fn cancel_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = None;
        self.update_compose_placeholder(window, cx);
        cx.notify();
    }

    /// Start editing an existing outgoing message
    pub fn start_edit(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_message = Some(msg);
        self.reply_to = None;
        self.context_menu = None;
        let text = self.editing_message.as_ref().map(|m| m.content.clone()).unwrap_or_default();
        self.set_compose_text(text, window, cx);
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// Cancel active edit
    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_message = None;
        self.set_compose_text(String::new(), window, cx);
        cx.notify();
    }

    /// Copy message text to system clipboard
    pub fn copy_message_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        toast::success("Pesan disalin ke clipboard", cx);
        cx.notify();
    }

    /// Delete a message
    pub fn delete_message(&mut self, chat_id: String, msg_id: String, cx: &mut Context<Self>) {
        self.context_menu = None;
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Deleted for everyone, never from our own database: flag it at once and
        // put the flag back if the request fails.
        let revoked_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).patch_message(&chat_id, &msg_id, |m| m.revoked_at = Some(revoked_at));
        }
        cx.notify();

        let cid = chat_id.clone();
        let mid = msg_id.clone();
        let (chat_id_back, msg_id_back) = (chat_id.clone(), msg_id.clone());
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.delete_message(&cid, &mid).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            match res {
                                Ok(Ok(_)) => {
                                    toast::success("Pesan berhasil dihapus", cx);
                                }
                                _ => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_message(&chat_id_back, &msg_id_back, |m| m.revoked_at = None);
                                    }
                                    toast::error("Gagal menghapus pesan", cx);
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

    /// Open the "Forward to…" dialog for a message.
    pub fn open_forward(&mut self, chat_id: String, msg: Message, cx: &mut Context<Self>) {
        self.context_menu = None;
        self.forward_selected.clear();
        self.is_forwarding = false;
        self.forward_ids = Vec::new();
        self.forward_msg = Some((chat_id, msg));
        cx.notify();
    }

    /// Open the "Forward to…" dialog for several messages, sent in order.
    pub fn open_forward_many(&mut self, chat_id: String, msgs: Vec<Message>, cx: &mut Context<Self>) {
        let Some(first) = msgs.first().cloned() else { return };
        self.context_menu = None;
        self.forward_selected.clear();
        self.is_forwarding = false;
        self.forward_ids = msgs.iter().map(|m| m.id.clone()).collect();
        self.forward_msg = Some((chat_id, first));
        cx.notify();
    }

    /// Send the pending forward to every selected chat.
    pub fn submit_forward(&mut self, cx: &mut Context<Self>) {
        let Some((chat_id, msg)) = self.forward_msg.clone() else { return };
        if self.forward_selected.is_empty() || self.is_forwarding {
            return;
        }
        let targets: Vec<String> = self.forward_selected.iter().cloned().collect();
        let ids: Vec<String> = if self.forward_ids.is_empty() { vec![msg.id.clone()] } else { self.forward_ids.clone() };
        let base_url = Self::compose_base_url(cx);
        self.is_forwarding = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move {
                        for id in &ids {
                            client.forward_message(&chat_id, id, &targets).await.map_err(|e| e.to_string())?;
                        }
                        Ok::<_, String>(())
                    })
                    .await;
                let ok = matches!(res, Ok(Ok(_)));
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_forwarding = false;
                            if ok {
                                this.forward_msg = None;
                                this.forward_ids.clear();
                                this.forward_selected.clear();
                                this.exit_select_mode(cx);
                                toast::success("Pesan diteruskan", cx);
                            } else {
                                toast::error("Gagal meneruskan pesan", cx);
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Save a message's media to disk (image, video, sticker, audio, file).
    pub fn save_message_media(&mut self, msg: &Message, cx: &mut Context<Self>) {
        self.context_menu = None;
        let Some(media) = msg.media_url.as_deref() else { return };
        let url = resolve_media_url(media, &Self::compose_base_url(cx));
        let name = media
            .split(['?', '#'])
            .next()
            .and_then(|path| path.rsplit('/').next())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.bin", msg.id));
        let what = match msg.message_type.as_str() {
            "image" => "Gambar",
            "video" => "Video",
            "sticker" => "Stiker",
            "audio" => "Audio",
            _ => "File",
        };
        Self::save_file(url, name, what, cx);
        cx.notify();
    }

    /// Add a sticker message to the favourite stickers.
    pub fn favorite_message_sticker(&mut self, msg: &Message, cx: &mut Context<Self>) {
        self.context_menu = None;
        let Some(media) = msg.media_url.clone() else { return };
        let message_id = msg.id.clone();
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.favorite_sticker(&message_id, &media, false).await })
                    .await;
                let ok = matches!(res, Ok(Ok(_)));
                let _ = cx_handle.update(|cx: &mut App| {
                    if ok {
                        toast::success("Stiker ditambahkan ke favorit", cx);
                    } else {
                        toast::error("Gagal menambahkan stiker", cx);
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn open_viewed_by(&mut self, message: Message, cx: &mut Context<Self>) {
        let chat_id = message.chat_id.clone();
        let message_id = message.id.clone();
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        self.viewed_by_message = Some(message);
        self.viewed_by_receipts.clear();
        self.viewed_by_recipients.clear();
        self.viewed_by_closing = false;
        self.is_loading_viewed_by = true;
        self.context_menu = None;
        // Message info shares the right-hand slot with the other drawers.
        self.is_info_sheet_open = false;
        self.info_sheet_closing = false;
        self.is_search_sheet_open = false;
        self.search_sheet_closing = false;
        self.is_starred_sheet_open = false;
        self.starred_sheet_closing = false;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                // Only groups have a roster to look up; asking the backend for a
                // private chat's "group" makes it query WhatsApp until it times out.
                let is_group = chat_id.ends_with("@g.us");
                let (group_result, receipts_result) = tokio::join!(
                    async {
                        if is_group {
                            client.get_group(&chat_id).await.map(Some)
                        } else {
                            Ok(None)
                        }
                    },
                    client.get_message_receipts(&chat_id, &message_id),
                );
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Some(group)) = group_result {
                                this.group_info = Some(group);
                            }
                            if let Ok(info) = receipts_result {
                                this.viewed_by_receipts = info.receipts;
                                this.viewed_by_recipients = info.recipients;
                            }
                            this.is_loading_viewed_by = false;
                            cx.notify();
                        });
                    }
                });
            }
        }).detach();
        cx.notify();
    }

    /// Slide the message info sheet out, then drop its data.
    pub(crate) fn close_viewed_by(&mut self, cx: &mut Context<Self>) {
        if self.viewed_by_message.is_none() || self.viewed_by_closing {
            return;
        }
        self.viewed_by_closing = true;
        Self::after_sheet_exit(cx, |this| {
            // Opening another message during the exit cancels the unmount.
            if this.viewed_by_closing {
                this.viewed_by_message = None;
                this.viewed_by_receipts.clear();
                this.viewed_by_closing = false;
                this.is_loading_viewed_by = false;
            }
        });
        cx.notify();
    }

}
