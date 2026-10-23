//! Chat view matching Web ChatPage, ChatSidebar, and ChatArea with 1:1 parity.
//!
//! `chat_page` is the entry point (state + `render`); the other modules hold the
//! pieces it assembles: `sidebar`, `conversation`, `composer`, `sheets`,
//! `dialogs`, `messages`, and chat media, plus data loading and actions.

use std::collections::HashSet;
use std::ops::Range;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{CallType, Chat, GroupCache, LinkPreview, Message, MessageReceipt, PinnedMessage, ReactionEntry};
use wabot_backend_client::multipart::SendMediaBuilder;

use crate::components::{emoji_picker::{EmojiPickerState, EMOJI_CATEGORIES}, toast};
use crate::components::message_bubble::{MessageBubbleHelper, MessageTicks};
use crate::components::ui::{
    h_flex, v_flex, Icon, IconName, InputEvent, InputState, Scrollbar, ScrollbarMode, Spinner,
    TextField, TextFieldEvent, Textarea, TextareaState, Tooltip,
};
use crate::icons::*;
use crate::state::auth::AuthState;
use crate::state::call::CallManager;
use crate::state::chat::ChatStore;
use crate::theme::manager::{ActiveTokens, AppThemeExt, ChatTextExt, ThemeManager};
use crate::theme::shape::ShapeExt;
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
pub mod layout_pref;
pub mod media;
mod messages;
mod sheets;
mod sidebar;
mod state;

pub(crate) use helpers::render_message_mentions_with_spans;
use layout_pref::LayoutKind;
pub(crate) use media::viewer_math::*;
pub use composer::*;
pub use conversation::{MentionText, TextMenuRequest};
pub use chat_page::*;
pub use state::*;

pub(crate) use constants::*;
pub(crate) use helpers::*;
