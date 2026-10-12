//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.
//!
//! `chat_page` is the entry point (state + `render`); the other modules hold the
//! pieces it assembles: `sidebar`, `conversation`, `composer`, `sheets`,
//! `dialogs`, `messages`, `image_viewer`, plus data loading and actions.

use std::collections::HashSet;
use std::ops::Range;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_component::scroll::{Scrollbar, ScrollbarMode, ScrollableElement};
use gpui_component::spinner::Spinner;
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, v_flex, Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{CallType, Chat, GroupCache, Message, MessageReceipt, ReactionEntry};
use wabot_backend_client::multipart::SendMediaBuilder;

use crate::components::{
    compose::{
        encode_markdown, looks_like_document_name, AttachPanel, ComposeDialog, ContactDraft,
        LocationDraft, MediaKind, PollDraft, StickerPickerState,
    },
    emoji_picker::{EmojiPickerState, EMOJI_CATEGORIES},
    toast,
};
use crate::components::message_bubble::{MessageBubbleHelper, MessageTicks};
use crate::components::MentionText;
use crate::icons::*;
use crate::layout_pref::{self, LayoutKind};
use crate::views::viewer_math::{
    clamp_pan, pan_after_zoom, preview_geometry, PREVIEW_MAX_ZOOM, PREVIEW_MIN_ZOOM,
    PREVIEW_TOOLBAR_H,
};
use crate::state::auth::AuthState;
use crate::state::call::CallManager;
use crate::state::chat::ChatStore;
use crate::theme::manager::{ActiveTokens, AppThemeExt, ThemeManager};
use crate::window_material;
use crate::TOKIO_RT;


mod chat_actions;
mod chat_page;
mod composer;
mod constants;
mod conversation;
mod data;
mod dialogs;
mod helpers;
mod image_viewer;
mod messages;
mod sheets;
mod sidebar;
mod state;

pub use chat_page::*;
pub use state::*;

pub(crate) use constants::*;
pub(crate) use helpers::*;
