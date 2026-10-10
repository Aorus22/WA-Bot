use crate::views::chat::*;

impl ChatView {
    pub(crate) fn ensure_composer_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose_input.is_some() {
            return;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a message..."));
        let textarea = cx.new(|cx| TextareaState::new(window, cx).placeholder("Write markdown..."));

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
        let textarea_subscription = cx.subscribe(
            &textarea,
            |this, textarea: Entity<TextareaState>, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.compose_text = textarea.read(cx).value().to_string();
                    this.refresh_mention_query();
                    cx.notify();
                }
            },
        );

        self.compose_input = Some(input);
        self.compose_textarea = Some(textarea);
        self._compose_input_subscription = Some(input_subscription);
        self._compose_textarea_subscription = Some(textarea_subscription);
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
            self.mention_query = Some(query.to_string());
        }
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
            textarea.update(cx, |state, cx| {
                state.set_value(value.clone(), window, cx);
                state.set_placeholder(placeholder, window, cx);
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
