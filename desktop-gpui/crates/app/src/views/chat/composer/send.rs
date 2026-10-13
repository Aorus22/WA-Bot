use crate::views::chat::*;

impl ChatView {
    /// Send a sticker by its media URL (web's `handleStickerSelect`).
    pub fn send_sticker(&mut self, media_url: String, is_animated: bool, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        self.attach_panel = AttachPanel::None;
        let base_url = Self::compose_base_url(cx);
        let temp_id = self.push_optimistic_attachment(
            cx,
            &chat_id,
            "[Sticker]".to_string(),
            "sticker",
            Some(media_url.clone()),
        );

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let target = chat_id.clone();
                let url = media_url.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.send_sticker(&target, &url, is_animated).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(id_res)) => {
                                    this.finish_optimistic_attachment(
                                        cx,
                                        &chat_id,
                                        &temp_id,
                                        Some(id_res.id),
                                    );
                                }
                                other => {
                                    let reason = match other {
                                        Ok(Err(error)) => error.to_string(),
                                        _ => "request interrupted".to_string(),
                                    };
                                    toast::error(format!("Gagal mengirim stiker: {reason}"), cx);
                                    this.finish_optimistic_attachment(cx, &chat_id, &temp_id, None);
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

    /// Open the native file picker for `kind` and send the picked file.
    pub fn pick_attachment(&mut self, kind: MediaKind, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        self.attach_panel = AttachPanel::None;
        self.is_sending_attachment = true;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let options = PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(kind.prompt().into()),
                };
                let receiver = cx_handle.update(|cx: &mut App| cx.prompt_for_paths(options));
                let picked = match receiver.await {
                    Ok(Ok(Some(paths))) => paths.into_iter().next(),
                    _ => None,
                };
                let Some(path) = picked else {
                    // Cancelled: nothing to send.
                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending_attachment = false;
                                cx.notify();
                            });
                        }
                    });
                    return;
                };

                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                let media_type = kind.media_type(&file_name).to_string();

                let read_path = path.clone();
                let read = TOKIO_RT
                    .spawn_blocking(move || std::fs::read(read_path))
                    .await;
                let Ok(Ok(bytes)) = read else {
                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = this.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending_attachment = false;
                                toast::error("Gagal membaca file", cx);
                                cx.notify();
                            });
                        }
                    });
                    return;
                };

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.send_attachment_bytes(
                                chat_id.clone(),
                                file_name.clone(),
                                media_type.clone(),
                                bytes,
                                String::new(),
                                None,
                                kind,
                                cx,
                            );
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Send `bytes` as a media message with an optimistic local bubble. Shared by the
    /// file picker and image paste; `caption` rides along with the media.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn send_attachment_bytes(
        &mut self,
        chat_id: String,
        file_name: String,
        media_type: String,
        bytes: Vec<u8>,
        caption: String,
        mime_type: Option<String>,
        kind: MediaKind,
        cx: &mut Context<Self>,
    ) {
        let base_url = Self::compose_base_url(cx);
        self.is_sending_attachment = true;
        let content = if caption.is_empty() {
            kind.optimistic_content(&file_name, &media_type)
        } else {
            caption.clone()
        };
        let msg_type = kind.optimistic_type(&media_type).to_string();
        let temp_id = self.push_optimistic_attachment(cx, &chat_id, content, &msg_type, None);
        if temp_id.is_empty() {
            self.is_sending_attachment = false;
            return;
        }

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let target = chat_id.clone();
                let file_for_send = file_name.clone();
                let media_for_send = media_type.clone();
                let res = TOKIO_RT
                    .spawn(async move {
                        if media_for_send == "audio" {
                            client
                                .send_audio(&target, bytes, &file_for_send, false, None, None)
                                .await
                        } else {
                            let mut builder = SendMediaBuilder::new(
                                &target,
                                media_for_send,
                                bytes,
                                file_for_send,
                            )
                            .caption(caption);
                            if let Some(mime) = mime_type {
                                builder = builder.mime_type(mime);
                            }
                            client.send_media(builder).await
                        }
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(id_res)) => {
                                    this.finish_optimistic_attachment(
                                        cx,
                                        &chat_id,
                                        &temp_id,
                                        Some(id_res.id),
                                    );
                                    toast::success("Lampiran terkirim", cx);
                                }
                                _ => {
                                    this.finish_optimistic_attachment(cx, &chat_id, &temp_id, None);
                                    toast::error("Gagal mengirim lampiran", cx);
                                }
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

    /// Insert the local pending bubble for an outgoing attachment.
    pub(crate) fn push_optimistic_attachment(
        &mut self,
        cx: &mut Context<Self>,
        chat_id: &str,
        content: String,
        msg_type: &str,
        media_url: Option<String>,
    ) -> String {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let temp_id = format!("temp-{}-{}", msg_type, now_ms);
        let msg = Message {
            id: temp_id.clone(),
            chat_id: chat_id.to_string(),
            from: "me".to_string(),
            to: chat_id.to_string(),
            content: content.clone(),
            timestamp: now_ms,
            status: "pending".to_string(),
            message_type: msg_type.to_string(),
            media_url,
            is_automatic: None,
            notification_suppressed: false,
            sender_name: Some("You".to_string()),
            reply_to_id: self.reply_to.as_ref().map(|r| r.id.clone()),
            forwarded: None,
            reactions: None,
            extra: None,
        };

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.upsert_message(chat_id, msg.clone());
            if let Some(c) = store.chats.iter_mut().find(|c| c.id == chat_id) {
                c.last_msg = content.clone();
                c.last_time = now_ms;
            }
        }

        let old_items = self.messages_list_state.item_count();
        self.cached_render_messages.push(RenderMessage {
            msg,
            is_from_me: true,
            content,
            time_str: Self::format_chat_time(now_ms),
            ticks: MessageTicks::Sent,
            quoted: None,
        });
        let total_items = self.cached_render_messages.len() + 1;
        self.messages_list_state.splice(old_items..old_items, total_items.saturating_sub(old_items));
        self.messages_list_state.scroll_to_end();
        temp_id
    }

    /// Reconcile the optimistic attachment bubble with the server response.
    pub(crate) fn finish_optimistic_attachment(
        &mut self,
        cx: &mut Context<Self>,
        chat_id: &str,
        temp_id: &str,
        real_id: Option<String>,
    ) {
        let Some(real_id) = real_id else {
            // Failure: keep the bubble around marked as failed.
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(chat_id, temp_id, |m| {
                    m.status = "failed".to_string();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == temp_id) {
                rm.msg.status = "failed".to_string();
                rm.ticks = MessageTicks::Failed;
            }
            self.messages_list_state.remeasure();
            return;
        };

        // The authoritative WS broadcast may have landed first; then the temp
        // bubble is a duplicate and gets dropped (web does the same).
        let already_arrived = cx.has_global::<ChatStore>()
            && ChatStore::global(cx)
                .messages_by_chat
                .get(chat_id)
                .map(|entry| entry.messages.iter().any(|m| m.id == real_id))
                .unwrap_or(false);

        if already_arrived {
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).delete_message(chat_id, temp_id);
            }
            self.cached_render_messages.retain(|m| m.msg.id != temp_id);
        } else {
            let rid = real_id.clone();
            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(chat_id, temp_id, move |m| {
                    m.id = rid.clone();
                    m.status = "sent".to_string();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == temp_id) {
                rm.msg.id = real_id;
                rm.msg.status = "sent".to_string();
            }
        }
        self.messages_list_state.remeasure();
    }

    /// Send the poll dialog contents (web's `PollDialog`).
    pub fn send_poll(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (question, options, multi_select) = match self.poll_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move {
                        client.send_poll(&chat_id, &question, &options, multi_select).await
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.poll_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Poll terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim poll", cx),
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

    /// Send the location dialog contents (web's `LocationDialog`).
    pub fn send_location(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (latitude, longitude, name, address, live) = match self.location_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move {
                        client
                            .send_location(&chat_id, latitude, longitude, Some(&name), Some(&address), Some(live))
                            .await
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.location_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Lokasi terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim lokasi", cx),
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

    /// Send the contact dialog contents (web's `ContactDialog`).
    pub fn send_contact(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else {
            return;
        };
        let (name, phone) = match self.contact_draft.validated() {
            Ok(parts) => parts,
            Err(msg) => {
                toast::error(msg, cx);
                return;
            }
        };
        if self.is_sending_attachment {
            return;
        }
        self.is_sending_attachment = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.send_contact(&chat_id, &name, &phone, None).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending_attachment = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.contact_draft.reset();
                                    this.close_compose_dialog(cx);
                                    toast::success("Kontak terkirim", cx);
                                }
                                _ => toast::error("Gagal mengirim kontak", cx),
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

    /// Send current message in compose bar (supports regular send, reply, and edit)
    pub fn send_current_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.compose_text.trim().to_string();
        if self.pending_attachment.is_some() && self.editing_message.is_none() {
            self.send_pending_attachment(window, cx);
            return;
        }
        if text.is_empty() || self.is_sending {
            return;
        }
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        self.set_compose_text(String::new(), window, cx);
        self.is_sending = true;

        // Markdown mode wraps the outgoing text in the `{{md:…}}` wire format
        // the renderers understand; the optimistic bubble keeps the plain text.
        let wire_text = if self.is_md_mode {
            encode_markdown(&text)
        } else {
            text.clone()
        };

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        // Case 1: Editing existing message
        if let Some(edit_msg) = self.editing_message.take() {
            self.update_compose_placeholder(window, cx);
            let edit_id = edit_msg.id.clone();
            let sent_text = wire_text.clone();
            let target_chat = chat_id.clone();

            if cx.has_global::<ChatStore>() {
                ChatStore::global_mut(cx).patch_message(&target_chat, &edit_id, |m| {
                    m.content = sent_text.clone();
                });
            }
            if let Some(rm) = self.cached_render_messages.iter_mut().find(|m| m.msg.id == edit_id) {
                rm.content = text.clone();
                rm.msg.content = sent_text.clone();
            }

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let view_weak = this;
                let cx_handle = cx.clone();
                async move {
                    let client = HttpClient::new(&base_url);
                    let res = TOKIO_RT
                        .spawn(async move { client.edit_message(&target_chat, &edit_id, &sent_text).await })
                        .await;

                    let _ = cx_handle.update(|cx: &mut App| {
                        if let Some(view) = view_weak.upgrade() {
                            view.update(cx, |this, cx| {
                                this.is_sending = false;
                                match res {
                                    Ok(Ok(_)) => {
                                        toast::success("Pesan berhasil diedit", cx);
                                    }
                                    _ => {
                                        toast::error("Gagal mengedit pesan", cx);
                                    }
                                }
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
            return;
        }

        // Case 2: Replying to message or Case 3: Regular send
        let (mention_text, mention_jids) = prepare_mention_text(&text, self.group_info.as_ref());
        let wire_text = if self.is_md_mode {
            encode_markdown(&mention_text)
        } else {
            mention_text
        };
        let reply_to_id = self.reply_to.take().map(|m| m.id);
        self.update_compose_placeholder(window, cx);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let temp_id = format!("temp-{}", now_ms);

        let opt_msg = Message {
            id: temp_id.clone(),
            chat_id: chat_id.clone(),
            from: "me".to_string(),
            to: chat_id.clone(),
            content: wire_text.clone(),
            timestamp: now_ms,
            status: "pending".to_string(),
            message_type: "text".to_string(),
            media_url: None,
            is_automatic: None,
            notification_suppressed: false,
            sender_name: Some("You".to_string()),
            reply_to_id: reply_to_id.clone(),
            forwarded: None,
            reactions: None,
            extra: if mention_jids.is_empty() {
                None
            } else {
                Some(wabot_backend_client::dto::MessageExtra {
                    edits: Vec::new(),
                    edited_at: None,
                    poll: None,
                    location: None,
                    contact: None,
                    view_once: None,
                    gif: None,
                    link_preview: None,
                    mentions: mention_jids.clone(),
                    mention_names: Default::default(),
                    mention_chats: Default::default(),
                    mentions_me: false,
                })
            },
        };

        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            store.upsert_message(&chat_id, opt_msg.clone());
            if let Some(c) = store.chats.iter_mut().find(|c| c.id == chat_id) {
                c.last_msg = text.clone();
                c.last_time = now_ms;
            }
        }

        // Optimistically add to cached render messages
        let old_items = self.messages_list_state.item_count();
        self.cached_render_messages.push(RenderMessage {
            msg: opt_msg.clone(),
            is_from_me: true,
            content: text.clone(),
            time_str: Self::format_chat_time(now_ms),
            ticks: MessageTicks::Sent,
            quoted: None,
        });
        let total_items = self.cached_render_messages.len() + 1;
        if total_items > old_items {
            self.messages_list_state.splice(old_items..old_items, total_items - old_items);
        } else {
            self.messages_list_state.reset(total_items);
        }
        self.messages_list_state.scroll_to_end();

        let target_chat = chat_id.clone();
        let sent_text = wire_text.clone();
        let opt_id_clone = temp_id.clone();
        let rep_id = reply_to_id.clone();
        let sent_mentions = mention_jids.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let c_target = target_chat.clone();
                let s_text = sent_text.clone();

                let res = if let Some(r_id) = rep_id {
                    let rep_id_clone = r_id.clone();
                    TOKIO_RT
                        .spawn(async move {
                            client.reply_message(&c_target, &rep_id_clone, &s_text, &sent_mentions).await
                                .map(|_| wabot_backend_client::dto::IdResult {
                                    status: "success".to_string(),
                                    id: format!("rep-{}", now_ms),
                                })
                        })
                        .await
                } else {
                    TOKIO_RT
                        .spawn(async move { client.send_message(&c_target, &s_text, &sent_mentions).await })
                        .await
                };

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_sending = false;
                            match res {
                                Ok(Ok(id_res)) => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_message(
                                            &target_chat,
                                            &opt_id_clone,
                                            |m| {
                                                m.id = id_res.id.clone();
                                                m.status = "sent".to_string();
                                            },
                                        );
                                    }
                                    if let Some(rm) = this.cached_render_messages.iter_mut().find(|m| m.msg.id == opt_id_clone) {
                                        rm.msg.id = id_res.id.clone();
                                        rm.msg.status = "sent".to_string();
                                    }
                                }
                                _ => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_message(
                                            &target_chat,
                                            &opt_id_clone,
                                            |m| {
                                                m.status = "error".to_string();
                                            },
                                        );
                                    }
                                    toast::error("Failed to send message", cx);
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

}
