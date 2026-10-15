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

    /// Open the one-to-one conversation with `jid` (a clicked mention), adding
    /// an empty row for it if there is no history with that person yet.
    pub fn open_direct_chat(&mut self, jid: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            if !store.chats.iter().any(|chat| chat.id == jid) {
                let label = if name.trim().is_empty() {
                    jid.split('@').next().unwrap_or(jid).to_string()
                } else {
                    name.to_string()
                };
                store.chats.insert(0, Chat {
                    id: jid.to_string(),
                    name: label,
                    avatar: String::new(),
                    last_msg: String::new(),
                    last_time: 0,
                    unread: 0,
                    has_unread_mention: false,
                    is_active: true,
                    is_group: false,
                    archived: false,
                    pinned_at: None,
                    mute_mode: "off".to_string(),
                    muted_until: None,
                    disappearing_secs: 0,
                });
                store.chats_version = store.chats_version.wrapping_add(1);
            }
        }
        self.archived_mode = false;
        self.select_chat(jid.to_string(), window, cx);
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
        self.sidebar_animate = true;
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.sidebar_exiting = self.sidebar_collapsed;
        if self.sidebar_collapsed {
            // Keep the list mounted while it slides shut.
            Self::after_sheet_exit(cx, |this| {
                if this.sidebar_collapsed {
                    this.sidebar_exiting = false;
                }
            });
        }
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

impl ChatView {
    /// Add the picked contacts to the group the picker was opened for.
    pub fn submit_add_members(&mut self, cx: &mut Context<Self>) {
        let Some(group_id) = self.add_members_for.clone() else {
            return;
        };
        let jids: Vec<String> = self.new_group_selected.iter().cloned().collect();
        if jids.is_empty() {
            toast::error("Pilih minimal satu kontak", cx);
            return;
        }
        self.is_creating_group = true;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let gid = group_id.clone();
                let res = TOKIO_RT
                    .spawn(async move { client.update_group_participants(&gid, "add", &jids).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_creating_group = false;
                            match res {
                                Ok(Ok(group)) => {
                                    this.is_new_group_open = false;
                                    this.add_members_for = None;
                                    this.new_group_selected.clear();
                                    if this.selected_chat_id.as_deref() == Some(group.jid.as_str()) {
                                        this.group_info = Some(group);
                                    }
                                    toast::success("Anggota ditambahkan", cx);
                                }
                                _ => toast::error("Gagal menambahkan anggota", cx),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Open the contact picker to add members to the open group.
    pub fn open_add_members(&mut self, chat_id: String, cx: &mut Context<Self>) {
        self.header_menu = None;
        self.new_group_selected.clear();
        self.add_members_for = Some(chat_id);
        self.is_new_group_open = true;
        cx.notify();
    }

    /// Mark every chat with unread messages as read.
    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        self.header_menu = None;
        let unread: Vec<String> = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).chats.iter().filter(|c| c.unread > 0).map(|c| c.id.clone()).collect()
        } else {
            Vec::new()
        };
        if unread.is_empty() {
            toast::success("Tidak ada pesan belum dibaca", cx);
        }
        for chat_id in unread {
            self.mark_chat_read_action(chat_id, cx);
        }
        cx.notify();
    }

    /// Ask for confirmation, then log out of WhatsApp.
    pub fn open_logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.header_menu = None;
        crate::components::dialogs::logout::open_logout_dialog(window, cx, move |_window, cx| {
            let base_url = if cx.has_global::<AuthState>() {
                AuthState::global(cx).base_url.clone()
            } else {
                "http://127.0.0.1:3000/api".to_string()
            };
            cx.spawn(async move |cx| {
                let client = HttpClient::new(&base_url);
                let _ = client.logout().await;
                let _ = cx.update(|cx| {
                    if cx.has_global::<AuthState>() {
                        AuthState::global_mut(cx).set_status(crate::state::auth::AuthStatus::Unauthenticated);
                    }
                    if cx.has_global::<crate::router::Router>() {
                        crate::router::Router::global_mut(cx).navigate(crate::router::AppRoute::Chat);
                    }
                    cx.refresh_windows();
                });
            })
            .detach();
        });
        cx.notify();
    }

    /// Save the whole conversation as a text file in the Downloads folder.
    pub fn export_chat(&mut self, chat_id: String, chat_name: String, cx: &mut Context<Self>) {
        self.header_menu = None;
        if self.is_exporting_chat {
            return;
        }
        self.is_exporting_chat = true;
        toast::success("Mengekspor chat…", cx);
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let result = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&base_url);
                        let mut all: Vec<Message> = Vec::new();
                        let mut before: Option<i64> = None;
                        // Pages come back newest-first by cursor; stop at the start or a sane cap.
                        for _ in 0..500 {
                            let page = client.get_messages(&chat_id, Some(200), before, None).await.map_err(|e| e.to_string())?;
                            let Some(oldest) = page.first().map(|m| m.timestamp) else { break };
                            let done = page.len() < 200;
                            let mut page = page;
                            page.extend(all);
                            all = page;
                            if done {
                                break;
                            }
                            before = Some(oldest);
                        }
                        let text = export_chat_text(&chat_name, &all);
                        let dir = std::env::var_os("HOME")
                            .or_else(|| std::env::var_os("USERPROFILE"))
                            .map(std::path::PathBuf::from)
                            .map(|home| home.join("Downloads"))
                            .filter(|d| d.is_dir())
                            .unwrap_or_else(std::env::temp_dir);
                        let safe: String = chat_name
                            .chars()
                            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                            .collect();
                        let path = dir.join(format!("WhatsApp chat - {}.txt", safe.trim()));
                        std::fs::write(&path, text).map_err(|e| e.to_string())?;
                        Ok::<_, String>((path, all.len()))
                    })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_exporting_chat = false;
                            match result {
                                Ok(Ok((path, count))) => {
                                    toast::success(format!("{count} pesan diekspor ke {}", path.display()), cx)
                                }
                                _ => toast::error("Gagal mengekspor chat", cx),
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

/// The exported text: one line per message, `[date, time] sender: text`.
pub(crate) fn export_chat_text(chat_name: &str, messages: &[Message]) -> String {
    format!("Chat with {chat_name}\n\n{}", message_lines(messages))
}

/// `[date, time] sender: text`, one line per message.
pub(crate) fn message_lines(messages: &[Message]) -> String {
    use chrono::TimeZone;
    let mut out = String::new();
    for m in messages {
        let when = chrono::Local
            .timestamp_millis_opt(m.timestamp)
            .single()
            .map(|t| t.format("%d/%m/%Y, %H.%M").to_string())
            .unwrap_or_default();
        let who = if m.from == "me" {
            "You".to_string()
        } else {
            m.sender_name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| m.from.split('@').next().unwrap_or(&m.from).to_string())
        };
        let content = MessageBubbleHelper::decode_content(&m.content);
        let placeholder = content.trim().is_empty() || (content.starts_with('[') && content.ends_with(']'));
        let body = if placeholder { crate::views::chat::helpers::quote_media_label(m) } else { content };
        let deleted = if m.revoked_at.is_some() { " (deleted for everyone)" } else { "" };
        out.push_str(&format!("[{when}] {who}: {body}{deleted}\n"));
    }
    out
}

#[cfg(test)]
mod export_tests {
    // Not `super::*`: that brings gpui's own `test` attribute into scope.
    use super::export_chat_text;
    use wabot_backend_client::dto::Message;

    fn message(from: &str, content: &str, message_type: &str) -> Message {
        Message {
            from: from.into(),
            content: content.into(),
            message_type: message_type.into(),
            timestamp: 1_700_000_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn export_has_one_line_per_message_and_names_media() {
        let mut deleted = message("me", "oops", "text");
        deleted.revoked_at = Some(1);
        let text = export_chat_text(
            "Budi",
            &[
                message("628111@s.whatsapp.net", "halo", "text"),
                message("628111@s.whatsapp.net", "[Image]", "image"),
                deleted,
            ],
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Chat with Budi");
        assert_eq!(lines.len(), 5, "{text}");
        assert!(lines[2].ends_with("628111: halo"), "{}", lines[2]);
        assert!(lines[3].ends_with("628111: 📷 Photo"), "{}", lines[3]);
        assert!(lines[4].ends_with("You: oops (deleted for everyone)"), "{}", lines[4]);
    }
}
