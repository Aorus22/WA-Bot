use crate::views::chat::*;

impl ChatView {
    /// Initiate voice or video call matching Web client parity
    pub fn start_call(&mut self, is_video: bool, cx: &mut Context<Self>) {
        let chat_id = match &self.selected_chat_id {
            Some(id) => id.clone(),
            None => return,
        };

        let is_group = chat_id.ends_with("@g.us") || if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().find(|c| c.id == chat_id).map(|c| c.is_group).unwrap_or(false)
        } else {
            false
        };

        let chat_name = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().find(|c| c.id == chat_id).map(|c| c.name.clone()).unwrap_or_else(|| chat_id.clone())
        } else {
            chat_id.clone()
        };

        let call_type = match (is_group, is_video) {
            (true, true) => CallType::GroupVideo,
            (true, false) => CallType::GroupAudio,
            (false, true) => CallType::Video,
            (false, false) => CallType::Audio,
        };

        // Show the full-screen call overlay immediately (Web does the same with
        // its optimistic `setActiveCall`); the API response then replaces it
        // with the authoritative backend state.
        let group_jid = if is_group { Some(chat_id.clone()) } else { None };
        if cx.has_global::<CallManager>() {
            CallManager::global_mut(cx).initiate_call(&chat_id, call_type.clone(), group_jid);
        }
        let _ = chat_name;
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let cid = chat_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move {
                    if is_group {
                        client.create_group_call(&cid, &[], call_type).await
                    } else {
                        client.create_call(&cid, call_type).await
                    }
                }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            match res {
                                Ok(Ok(call_state)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).adopt(call_state);
                                    }
                                }
                                Ok(Err(e)) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(None);
                                    }
                                    toast::error(format!("Failed to start call: {e}"), cx);
                                }
                                Err(_) => {
                                    if cx.has_global::<CallManager>() {
                                        CallManager::global_mut(cx).end_local(None);
                                    }
                                    toast::error("Failed to start call", cx);
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

    /// Toggle pin state for a chat
    pub fn toggle_pin(&mut self, chat_id: String, is_pinned: bool, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let new_state = !is_pinned;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.pin_chat(&target_chat, new_state).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            if let Ok(Ok(cs)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).patch_chat_state(&cs);
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

    /// Pages layout: leave the open chat and return to the chat list.
    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        self.selected_chat_id = None;
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).active_chat_id = None;
        }
        self.context_menu = None;
        self.chat_context_menu = None;
        self.preview_image_url = None;
        cx.notify();
    }

    /// Sidebar layout: hide or show the chat list.
    pub fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    /// Mark a chat as read from the context menu (server + local unread badge).
    pub fn mark_chat_read_action(&mut self, chat_id: String, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).mark_chat_read(&chat_id);
        }
        let target = chat_id;
        cx.spawn(move |_this: WeakEntity<Self>, _cx: &mut AsyncApp| async move {
            let client = HttpClient::new(&base_url);
            let _ = TOKIO_RT
                .spawn(async move { client.mark_as_read(&target).await })
                .await;
        })
        .detach();
        cx.notify();
    }

    /// Leave a group, then reload the chat list.
    pub fn leave_group_action(&mut self, chat_id: String, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let target = chat_id;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.leave_group(&target).await })
                    .await;
                let ok = matches!(res, Ok(Ok(_)));
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            if ok {
                                this.load_chats(cx);
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Toggle archive state for a chat
    pub fn toggle_archive(&mut self, chat_id: String, is_archived: bool, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let new_state = !is_archived;
        let success_message = if new_state {
            "Chat berhasil diarsipkan"
        } else {
            "Chat dikeluarkan dari arsip"
        };
        let error_message = if new_state {
            "Gagal mengarsipkan chat"
        } else {
            "Gagal mengeluarkan chat dari arsip"
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.archive_chat(&target_chat, new_state).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(state)) => {
                                    if cx.has_global::<ChatStore>() {
                                        ChatStore::global_mut(cx).patch_chat_state(&state);
                                    }
                                    toast::success(success_message, cx);
                                }
                                Ok(Err(error)) => {
                                    toast::error(format!("{error_message}: {error}"), cx);
                                    this.load_chats(cx);
                                }
                                Err(error) => {
                                    toast::error(format!("{error_message}: {error}"), cx);
                                    this.load_chats(cx);
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

    /// Set mute mode for a chat ("off", "8h", "1w", "forever")
    pub fn set_mute(&mut self, chat_id: String, mode: String, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let target_chat = chat_id.clone();
        let target_mode = mode.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT
                    .spawn(async move { client.mute_chat(&target_chat, &target_mode).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_this, cx| {
                            if let Ok(Ok(cs)) = res {
                                if cx.has_global::<ChatStore>() {
                                    ChatStore::global_mut(cx).patch_chat_state(&cs);
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

    /// Create new group
    pub fn submit_create_group(&mut self, cx: &mut Context<Self>) {
        let name = self.new_group_name.trim().to_string();
        if name.is_empty() {
            toast::error("Nama grup wajib diisi", cx);
            cx.notify();
            return;
        }

        let participants: Vec<String> = self.new_group_selected.iter().cloned().collect();
        self.is_creating_group = true;

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
                let res = TOKIO_RT
                    .spawn(async move { client.create_group(&name, &participants).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_creating_group = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.is_new_group_open = false;
                                    this.new_group_name.clear();
                                    this.new_group_selected.clear();
                                    toast::success("Grup berhasil dibuat", cx);
                                    this.load_chats(cx);
                                }
                                _ => {
                                    toast::error("Gagal membuat grup", cx);
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

    /// Join group by link
    pub fn submit_join_group(&mut self, cx: &mut Context<Self>) {
        let link = self.join_group_link.trim().to_string();
        if link.is_empty() {
            toast::error("Tautan undangan wajib diisi", cx);
            cx.notify();
            return;
        }

        self.is_joining_group = true;
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
                let res = TOKIO_RT
                    .spawn(async move { client.join_group(&link).await })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_joining_group = false;
                            match res {
                                Ok(Ok(_)) => {
                                    this.is_join_group_open = false;
                                    this.join_group_link.clear();
                                    toast::success("Berhasil bergabung ke grup", cx);
                                    this.load_chats(cx);
                                }
                                _ => {
                                    toast::error("Gagal bergabung ke grup", cx);
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

    pub(crate) fn format_chat_time(timestamp: i64) -> String {
        if timestamp == 0 {
            return String::new();
        }
        let (hours, mins) = crate::localtime::hour_minute(timestamp);
        let period = if hours >= 12 { "PM" } else { "AM" };
        let h12 = if hours % 12 == 0 { 12 } else { hours % 12 };
        format!("{:02}:{:02} {}", h12, mins, period)
    }

}
