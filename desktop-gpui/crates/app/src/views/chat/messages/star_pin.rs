use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

/// Pin lengths the context menu offers: seconds and label.
pub(crate) const PIN_DURATIONS: [(u32, &str); 3] =
    [(86_400, "24 hours"), (604_800, "7 days"), (2_592_000, "30 days")];
/// What a pin lasts unless another length is picked (WhatsApp's default).
pub(crate) const DEFAULT_PIN_SECS: u32 = 604_800;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Who a message is from, for a one-line label.
pub(crate) fn sender_label(msg: &Message) -> String {
    if msg.from == "me" || MessageBubbleHelper::is_from_me(msg, "me") {
        "You".to_string()
    } else {
        msg.sender_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| msg.from.split('@').next().unwrap_or(&msg.from).to_string())
    }
}

/// First non-empty line of a message, or its type in brackets when it has no text.
pub(crate) fn snippet_of(msg: &Message) -> String {
    let decoded = MessageBubbleHelper::decode_content(&msg.content);
    match decoded.lines().find(|line| !line.trim().is_empty()) {
        Some(line) => line.trim().to_string(),
        None => format!("[{}]", if msg.message_type.is_empty() { "Message" } else { &msg.message_type }),
    }
}

impl ChatView {
    // ========================================================================
    // STARRING
    // ========================================================================

    /// Star or unstar `msg`. The star shows at once; a failed request puts it back.
    pub fn toggle_star(&mut self, msg: Message, cx: &mut Context<Self>) {
        self.context_menu = None;
        let starred = !msg.starred;
        self.apply_star(&msg, starred, cx);

        let base_url = Self::compose_base_url(cx);
        let (chat_id, msg_id) = (msg.chat_id.clone(), msg.id.clone());
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let (cid, mid) = (chat_id.clone(), msg_id.clone());
                let res = TOKIO_RT.spawn(async move { client.star_message(&cid, &mid, starred).await }).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if matches!(res, Ok(Ok(_))) {
                        return;
                    }
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            let mut original = msg.clone();
                            original.starred = !starred;
                            this.apply_star(&original, !starred, cx);
                            toast::error(
                                if starred { "Gagal memberi bintang" } else { "Gagal melepas bintang" },
                                cx,
                            );
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Reflect a star change in the store (bubbles) and the starred list.
    fn apply_star(&mut self, msg: &Message, starred: bool, cx: &mut Context<Self>) {
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).patch_message(&msg.chat_id, &msg.id, |m| m.starred = starred);
        }
        if starred {
            if !self.starred_messages.iter().any(|m| m.id == msg.id) {
                let mut row = msg.clone();
                row.starred = true;
                row.starred_at = Some(now_ms());
                row.chat_name = cx
                    .has_global::<ChatStore>()
                    .then(|| ChatStore::global(cx).chats.iter().find(|c| c.id == msg.chat_id).map(|c| c.name.clone()))
                    .flatten();
                self.starred_messages.insert(0, row);
            }
        } else {
            self.starred_messages.retain(|m| m.id != msg.id);
        }
        cx.notify();
    }

    // ========================================================================
    // PINNING
    // ========================================================================

    /// Pin `msg` for `duration` seconds, or unpin it (`duration` is then unused).
    pub fn set_message_pinned(&mut self, msg: Message, pinned: bool, duration: u32, cx: &mut Context<Self>) {
        self.context_menu = None;
        let chat_id = msg.chat_id.clone();
        if pinned {
            let now = now_ms();
            self.pinned_messages.retain(|p| p.message_id != msg.id);
            self.pinned_messages.insert(0, PinnedMessage {
                message_id: msg.id.clone(),
                pinned_by: "me".to_string(),
                pinned_at: now,
                expires_at: now + duration as i64 * 1000,
                message: Some(msg.clone()),
            });
        } else {
            self.pinned_messages.retain(|p| p.message_id != msg.id);
        }
        self.pin_banner_index = 0;
        cx.notify();

        let base_url = Self::compose_base_url(cx);
        let msg_id = msg.id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let (cid, mid) = (chat_id.clone(), msg_id);
                let res = TOKIO_RT
                    .spawn(async move { client.pin_message(&cid, &mid, pinned, duration).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return };
                    view.update(cx, |this, cx| {
                        match res {
                            Ok(Ok(_)) => toast::success(
                                if pinned { "Pesan disematkan" } else { "Sematan dilepas" },
                                cx,
                            ),
                            Ok(Err(error)) => {
                                let reason = match error {
                                    wabot_backend_client::error::ClientError::Api { message, .. } => message,
                                    other => other.to_string(),
                                };
                                toast::error(
                                    format!("{}: {reason}", if pinned { "Gagal menyematkan pesan" } else { "Gagal melepas sematan" }),
                                    cx,
                                );
                            }
                            Err(_) => toast::error("Permintaan terputus", cx),
                        }
                        // The server is the authority on what is pinned (it caps
                        // the number of pins), so settle on its answer either way.
                        this.load_pins(&chat_id, cx);
                    });
                });
            }
        })
        .detach();
    }

    /// Fetch the open chat's active pins. A missing endpoint or an error leaves
    /// the banner as it was.
    pub(crate) fn load_pins(&mut self, chat_id: &str, cx: &mut Context<Self>) {
        let base_url = Self::compose_base_url(cx);
        let chat_id = chat_id.to_string();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let cid = chat_id.clone();
                let res = TOKIO_RT.spawn(async move { client.get_chat_pins(&cid).await }).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    let (Ok(Ok(pins)), Some(view)) = (res, this.upgrade()) else { return };
                    view.update(cx, |this, cx| {
                        // The user may have moved on while this was in flight.
                        if this.selected_chat_id.as_deref() != Some(chat_id.as_str()) {
                            return;
                        }
                        this.pinned_messages = pins.into_iter().filter(|p| p.message.is_some()).collect();
                        this.pin_banner_index = 0;
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    /// Pins that are still in force, newest first.
    fn active_pins(&self) -> Vec<&PinnedMessage> {
        let now = now_ms();
        self.pinned_messages
            .iter()
            .filter(|p| p.message.is_some() && (p.expires_at <= 0 || p.expires_at > now))
            .collect()
    }

    /// Whether `msg_id` is pinned in the open chat.
    pub(crate) fn is_pinned(&self, msg_id: &str) -> bool {
        self.active_pins().iter().any(|p| p.message_id == msg_id)
    }

    /// Banner above the message list: the current pin (pin icon, sender,
    /// snippet). Clicking jumps to it and moves on to the next pin.
    pub(crate) fn render_pinned_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pins = self.active_pins();
        let count = pins.len();
        if count == 0 {
            return None;
        }
        let index = self.pin_banner_index % count;
        let pin = pins[index];
        let message = pin.message.as_ref()?;
        let (sender, snippet) = (sender_label(message), snippet_of(message));
        let target = pin.message_id.clone();

        let theme = *cx.app_theme();
        let shape = cx.shape();
        let surface = cx.surface();
        let primary_color = theme.primary;
        let title = if count > 1 { format!("{count} pinned messages · {sender}") } else { sender };

        Some(
            h_flex()
                .id("pinned-banner")
                .w_full()
                .px_4()
                .py_2()
                .when(shape.styled, |row| {
                    row.px(px(shape.spacing_unit)).py(px(shape.spacing_unit))
                })
                .gap_3()
                .items_center()
                .cursor_pointer()
                .border_b_1()
                .border_color(theme.border.opacity(0.4))
                .bg(surface.pinned_banner)
                .hover(|s| s.bg(surface.pinned_banner_hover))
                .tooltip(move |window, cx| Tooltip::new("Go to pinned message").build(window, cx))
                // A segment per pin, the current one lit, like WhatsApp's.
                .when(count > 1, |row| {
                    row.child(v_flex().gap(px(2.0)).h(px(28.0)).children((0..count.min(4)).map(|i| {
                        div()
                            .flex_1()
                            .w(px(3.0))
                            .rounded_full()
                            .bg(if i == index % 4 { primary_color } else { theme.muted_foreground.opacity(0.35) })
                    })))
                })
                .child(svg().data(PIN_SVG).size(px(16.0)).flex_shrink_0().text_color(theme.muted_foreground))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(primary_color)
                                .overflow_hidden()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .when(shape.styled, |text| text.font_weight(FontWeight::BOLD))
                                .text_color(theme.foreground)
                                .overflow_hidden()
                                .line_clamp(1)
                                .child(snippet),
                        ),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    this.navigate_to_message(&target, cx);
                    this.pin_banner_index = this.pin_banner_index.wrapping_add(1);
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    // ========================================================================
    // BUBBLE FOOTER MARKS
    // ========================================================================

    /// Small icons that precede the time in a bubble's footer: pinned, starred
    /// and disappearing. `tone` is the footer's own text colour.
    pub(crate) fn footer_marks(msg: &Message, pinned: bool, tone: Hsla) -> Vec<AnyElement> {
        let mut marks: Vec<AnyElement> = Vec::new();
        if let Some(meta) = msg.extra.as_ref().and_then(|extra| extra.disappearing.as_ref()) {
            let (expired, tip) = Self::disappearing_note(meta, now_ms());
            let tip = SharedString::from(tip);
            marks.push(
                div()
                    .id(SharedString::from(format!("disappearing-{}", msg.id)))
                    .flex()
                    .items_center()
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    .child(
                        svg()
                            .data(HOURGLASS_SVG)
                            .size(px(11.0))
                            .text_color(tone.opacity(if expired { 0.45 } else { 0.75 })),
                    )
                    .into_any_element(),
            );
        }
        if pinned {
            marks.push(svg().data(PIN_SVG).size(px(11.0)).text_color(tone.opacity(0.75)).into_any_element());
        }
        if msg.starred {
            marks.push(svg().data(STAR_FILLED_SVG).size(px(11.0)).text_color(tone.opacity(0.75)).into_any_element());
        }
        marks
    }
}
