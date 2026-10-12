//! The app's text input.
//!
//! Every editable single-line field goes through [`TextField`], so each one
//! gets the same editing behaviour for free (selection, Ctrl+A, clipboard, IME,
//! cursor movement) instead of re-implementing it from raw key presses. Views
//! own a `TextField`, render it with [`TextField::element`] and learn about
//! edits through the event closure given to [`TextField::new`].

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::h_flex;

use crate::theme::manager::ActiveTokens;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldEvent {
    /// The text changed; carries the new value.
    Change(String),
    /// Enter was pressed.
    Enter,
}

#[derive(Clone)]
pub struct TextField {
    state: Entity<InputState>,
}

impl TextField {
    pub fn new<V: 'static>(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl Fn(&mut V, TextFieldEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        Self::build(placeholder.into(), false, window, cx, on_event)
    }

    /// A field whose text is hidden (API keys, passwords).
    pub fn secret<V: 'static>(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl Fn(&mut V, TextFieldEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        Self::build(placeholder.into(), true, window, cx, on_event)
    }

    fn build<V: 'static>(
        placeholder: SharedString,
        masked: bool,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl Fn(&mut V, TextFieldEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).masked(masked));
        cx.subscribe_in(&state, window, move |view, state, event: &InputEvent, window, cx| {
            match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    on_event(view, TextFieldEvent::Change(value), window, cx);
                }
                InputEvent::PressEnter { .. } => on_event(view, TextFieldEvent::Enter, window, cx),
                _ => {}
            }
        })
        .detach();
        Self { state }
    }

    pub fn value(&self, cx: &App) -> String {
        self.state.read(cx).value().to_string()
    }

    pub fn is_empty(&self, cx: &App) -> bool {
        self.state.read(cx).value().is_empty()
    }

    /// Replace the text; this also emits a `Change` event.
    pub fn set_value(&self, value: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let value = value.into();
        self.state.update(cx, |state, cx| state.set_value(value, window, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }

    /// The field, styled like the rest of the app: rounded, muted fill, an
    /// optional leading icon and a clear button while it has text.
    pub fn element(&self, theme: &ActiveTokens, leading: Option<&'static [u8]>) -> Div {
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .rounded_xl()
            .border_1()
            .border_color(theme.border.opacity(0.3))
            .bg(theme.muted.opacity(0.5))
            .cursor_text()
            .when_some(leading, |el, icon| {
                el.child(svg().data(icon).size(px(14.0)).flex_shrink_0().text_color(theme.muted_foreground))
            })
            .child(
                Input::new(&self.state)
                    .appearance(false)
                    .bordered(false)
                    .cleanable(true)
                    .flex_1()
                    .text_sm()
                    .text_color(theme.foreground),
            )
    }
}

/// Multi-line counterpart of [`TextField`]. Enter inserts a newline. It is
/// returned unstyled so the caller decides how it looks.
#[derive(Clone)]
pub struct TextArea {
    state: Entity<gpui_component::input::TextareaState>,
}

impl TextArea {
    pub fn new<V: 'static>(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<V>,
        on_change: impl Fn(&mut V, String, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        let placeholder = placeholder.into();
        let state = cx.new(|cx| gpui_component::input::TextareaState::new(window, cx).placeholder(placeholder));
        cx.subscribe_in(&state, window, move |view, state, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                let value = state.read(cx).value().to_string();
                on_change(view, value, window, cx);
            }
        })
        .detach();
        Self { state }
    }

    pub fn value(&self, cx: &App) -> String {
        self.state.read(cx).value().to_string()
    }

    pub fn set_value(&self, value: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let value = value.into();
        self.state.update(cx, |state, cx| state.set_value(value, window, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn element(&self) -> gpui_component::input::Textarea {
        gpui_component::input::Textarea::new(&self.state).appearance(false).bordered(false)
    }
}
