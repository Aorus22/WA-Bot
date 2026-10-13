use crate::views::chat::*;

impl ChatView {
    pub(crate) fn ensure_composer_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose_input.is_some() {
            return;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a message..."));
        // Enter sends and Shift+Enter inserts a newline, as in WhatsApp Web.
        // Markdown mode flips that: plain Enter is a newline, Ctrl+Enter sends.
        let textarea = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Type a message...")
                .submit_on_enter(true)
        });

        let input_subscription = cx.subscribe(
            &input,
            |this, input: Entity<InputState>, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.compose_text = input.read(cx).value().to_string();
                    this.refresh_mention_query();
                    cx.notify();
                }
            },
        );
        let textarea_subscription = cx.subscribe_in(
            &textarea,
            window,
            |this, textarea: &Entity<TextareaState>, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.compose_text = textarea.read(cx).value().to_string();
                    this.refresh_mention_query();
                    cx.notify();
                }
                InputEvent::PressEnter { secondary, shift } => {
                    let send = if this.is_md_mode { *secondary } else { !*shift };
                    if send {
                        this.compose_text = textarea.read(cx).value().to_string();
                        this.send_current_message(window, cx);
                    }
                }
                _ => {}
            },
        );

        // Key bindings (Enter sends, arrows move the caret) run before any key
        // listener on an element, so the @-list has to intercept them here.
        let view = cx.entity().downgrade();
        self._mention_key_interceptor = Some(cx.intercept_keystrokes(move |event, window, cx| {
            let key = &event.keystroke;
            // Copy for text selected in the conversation. The window's own copy
            // action only fires with focus inside the root, which a click on a
            // bubble does not give it.
            if key.modifiers.secondary() && !key.modifiers.shift && !key.modifiers.alt && key.key == "c" {
                let selected = gpui_base::TextSelection::selected_text(window, cx);
                if !selected.trim().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.trim().to_string()));
                    cx.stop_propagation();
                }
                return;
            }
            if key.modifiers.shift || key.modifiers.control || key.modifiers.alt || key.modifiers.platform {
                return;
            }
            let Some(view) = view.upgrade() else { return };
            let consumed = view.update(cx, |this, cx| {
                let focused = this
                    .compose_textarea
                    .as_ref()
                    .is_some_and(|textarea| textarea.read(cx).focus_handle(cx).is_focused(window));
                focused && this.handle_mention_key(key.key.as_str(), window, cx)
            });
            if consumed {
                cx.stop_propagation();
            }
        }));

        self.compose_input = Some(input);
        self.compose_textarea = Some(textarea);
        self._compose_input_subscription = Some(input_subscription);
        self._compose_textarea_subscription = Some(textarea_subscription);
    }

    /// Create the single-line inputs on first use and push model changes made
    /// elsewhere (a cleared form, a new chat) back into them.
    pub(crate) fn ensure_text_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_field.is_none() {
            self.search_field = Some(TextField::new("Search conversations...", window, cx, |this, event, _, cx| {
                if let TextFieldEvent::Change(value) = event {
                    this.search_query = value;
                    cx.notify();
                }
            }));
            self.sheet_search_field = Some(TextField::new("Search messages...", window, cx, |this, event, _, cx| {
                match event {
                    TextFieldEvent::Change(value) => {
                        this.search_sheet_query = value;
                        this.run_search_messages(cx);
                    }
                    TextFieldEvent::Enter => this.run_search_messages(cx),
                }
            }));
            self.new_group_field = Some(TextField::new("Group name...", window, cx, |this, event, _, cx| {
                if let TextFieldEvent::Change(value) = event {
                    this.new_group_name = value;
                    cx.notify();
                }
            }));
            self.join_link_field = Some(TextField::new("https://chat.whatsapp.com/...", window, cx, |this, event, _, cx| {
                match event {
                    TextFieldEvent::Change(value) => {
                        this.join_group_link = value;
                        cx.notify();
                    }
                    TextFieldEvent::Enter => this.submit_join_group(cx),
                }
            }));
        }
        for (field, model) in [
            (&self.search_field, &self.search_query),
            (&self.sheet_search_field, &self.search_sheet_query),
            (&self.new_group_field, &self.new_group_name),
            (&self.join_link_field, &self.join_group_link),
        ] {
            if let Some(field) = field {
                if field.value(cx) != *model {
                    field.set_value(model.clone(), window, cx);
                }
            }
        }
    }

    pub(crate) fn compose_placeholder(&self) -> &'static str {
        if self.editing_message.is_some() {
            "Edit message..."
        } else if self.reply_to.is_some() {
            "Type reply..."
        } else if self.is_md_mode {
            "Write markdown..."
        } else {
            "Type a message..."
        }
    }

    pub(crate) fn refresh_mention_query(&mut self) {
        if self.editing_message.is_some()
            || !self.selected_chat_id.as_deref().is_some_and(|id| id.ends_with("@g.us"))
        {
            self.mention_query = None;
            return;
        }
        let Some(at) = self.compose_text.rfind('@') else {
            self.mention_query = None;
            return;
        };
        let prefix = &self.compose_text[..at];
        if prefix.chars().last().is_some_and(|ch| !ch.is_whitespace()) {
            self.mention_query = None;
            return;
        }
        let query = &self.compose_text[at + 1..];
        if query.chars().any(char::is_whitespace) {
            self.mention_query = None;
        } else {
            if self.mention_query.as_deref() != Some(query) {
                self.mention_index = 0;
            }
            self.mention_query = Some(query.to_string());
        }
    }

    /// Members the @-list offers for the current query: (jid, label, is_admin).
    pub(crate) fn mention_candidates(&self) -> Vec<(String, String, bool)> {
        let (Some(query), Some(group)) = (self.mention_query.as_ref(), self.group_info.as_ref()) else {
            return Vec::new();
        };
        let needle = query.to_lowercase();
        group.participants.iter()
            .filter_map(|participant| {
                let name = participant.name.as_deref()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| participant.jid.split('@').next().unwrap_or(&participant.jid));
                if !needle.is_empty()
                    && !name.to_lowercase().contains(&needle)
                    && !participant.jid.to_lowercase().contains(&needle)
                {
                    return None;
                }
                Some((participant.jid.clone(), name.to_string(), participant.is_admin))
            })
            .take(8)
            .collect()
    }

    /// Arrow keys, Tab/Enter and Escape drive the @-list while it is open.
    /// Returns whether the key was consumed.
    pub(crate) fn handle_mention_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let candidates = self.mention_candidates();
        if candidates.is_empty() {
            return false;
        }
        let count = candidates.len();
        match key {
            "down" => self.mention_index = (self.mention_index + 1) % count,
            "up" => self.mention_index = (self.mention_index + count - 1) % count,
            "enter" | "tab" => {
                let jid = candidates[self.mention_index.min(count - 1)].0.clone();
                self.insert_group_mention(&jid, window, cx);
                return true;
            }
            "escape" => self.mention_query = None,
            _ => return false,
        }
        self.mention_scroll.scroll_to_item(self.mention_index);
        cx.notify();
        true
    }

    pub(crate) fn insert_group_mention(&mut self, jid: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(member) = self.group_info.as_ref()
            .and_then(|group| group.participants.iter().find(|participant| participant.jid == jid))
            .cloned()
        else {
            return;
        };
        let Some(at) = self.compose_text.rfind('@') else {
            return;
        };
        let label = member.name.filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| member.jid.split('@').next().unwrap_or(&member.jid).to_string());
        self.compose_text.replace_range(at.., &format!("@{label} "));
        self.mention_query = None;
        let value = self.compose_text.clone();
        self.set_compose_text(value, window, cx);
        self.focus_composer(window, cx);
        cx.notify();
    }

    pub(crate) fn set_compose_text(&mut self, value: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) {
        let value = value.into();
        let placeholder = self.compose_placeholder();
        self.compose_text = value.clone();

        if let Some(input) = self.compose_input.clone() {
            input.update(cx, |state, cx| {
                state.set_value(value.clone(), window, cx);
                state.set_placeholder(placeholder, window, cx);
            });
        }
        if let Some(textarea) = self.compose_textarea.clone() {
            // set_value leaves the caret at the start; typing continues at the end.
            let end = value.len();
            textarea.update(cx, |state, cx| {
                state.set_value(value.clone(), window, cx);
                state.set_placeholder(placeholder, window, cx);
                state.set_selected_range(end..end, cx);
            });
        }
    }

    pub(crate) fn update_compose_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = self.compose_placeholder();
        if let Some(input) = self.compose_input.clone() {
            input.update(cx, |state, cx| state.set_placeholder(placeholder, window, cx));
        }
        if let Some(textarea) = self.compose_textarea.clone() {
            textarea.update(cx, |state, cx| state.set_placeholder(placeholder, window, cx));
        }
    }

    pub(crate) fn focus_composer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(textarea) = self.compose_textarea.clone() {
            textarea.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    pub(crate) fn compose_base_url(cx: &App) -> String {
        if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        }
    }

    /// Toggle markdown mode (web's "Markdown (on/off)" item).
    pub fn toggle_markdown_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_md_mode = !self.is_md_mode;
        let submit_on_enter = !self.is_md_mode;
        if let Some(textarea) = self.compose_textarea.clone() {
            textarea.update(cx, |state, cx| state.set_submit_on_enter(submit_on_enter, cx));
        }
        self.attach_panel = AttachPanel::None;
        let text = self.compose_text.clone();
        self.set_compose_text(text, window, cx);
        self.focus_composer(window, cx);
        if self.is_md_mode {
            toast::info("Markdown aktif untuk pesan berikutnya", cx);
        } else {
            toast::info("Markdown nonaktif", cx);
        }
        cx.notify();
    }

    /// Append an emoji to the compose box (web's `addEmoji`).
    pub fn insert_emoji(&mut self, emoji: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.compose_text.push_str(emoji);
        let text = self.compose_text.clone();
        self.set_compose_text(text, window, cx);
        cx.notify();
    }

}
