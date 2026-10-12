use crate::views::chat::*;

/// Filter chips below search in sidebar (matching Web / WhatsApp)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatFilter {
    #[default]
    All,
    Unread,
    Groups,
    Contacts,
}

/// Tabs for Chat Info Sheet
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InfoSheetTab {
    #[default]
    Media,
    Docs,
    Links,
    Members,
}

/// Context menu popup state when right-clicking a message bubble
#[derive(Clone, Debug)]
pub struct MessageContextMenu {
    pub msg: Message,
    pub is_from_me: bool,
    pub position: Point<Pixels>,
    /// Showing the full emoji list instead of the actions (the "+" / React rows).
    pub reaction_picker: bool,
}

/// Context menu popup state when right-clicking a sidebar chat item
#[derive(Clone, Debug)]
pub struct ChatContextMenu {
    pub chat_id: String,
    pub chat_name: String,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub is_muted: bool,
    pub position: Point<Pixels>,
    pub show_mute_submenu: bool,
    pub is_group: bool,
    pub unread: u32,
    /// First "Exit group" click arms it; the second one leaves.
    pub confirm_leave: bool,
}

/// Pre-computed, cached renderable message to guarantee silky smooth 60fps scrolling
#[derive(Clone, Debug)]
pub struct RenderMessage {
    pub msg: Message,
    pub is_from_me: bool,
    pub content: String,
    pub time_str: String,
    pub ticks: MessageTicks,
    pub quoted: Option<(String, String)>, // (sender_name, first_line)
}

/// An image pasted into the composer, waiting to be sent (with the typed text
/// as its caption) when the user hits send.
#[derive(Clone)]
pub struct PendingAttachment {
    pub file_name: String,
    pub image: std::sync::Arc<Image>,
}
