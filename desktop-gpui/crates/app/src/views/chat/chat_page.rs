use crate::views::chat::*;

pub struct ChatView {
    pub search_query: String,
    pub search_focus_handle: FocusHandle,
    pub filter: ChatFilter,
    pub compose_text: String,
    pub(crate) compose_input: Option<Entity<InputState>>,
    pub(crate) compose_textarea: Option<Entity<TextareaState>>,
    pub(crate) _compose_input_subscription: Option<Subscription>,
    pub(crate) _compose_textarea_subscription: Option<Subscription>,
    pub archived_mode: bool,
    pub selected_chat_id: Option<String>,
    pub reply_to: Option<Message>,
    pub editing_message: Option<Message>,
    pub mention_query: Option<String>,
    pub group_info: Option<GroupCache>,
    pub is_sending: bool,
    pub is_loading_chats: bool,
    pub is_loading_messages: bool,
    pub is_loading_more: bool,
    pub is_loading_newer: bool,
    pub has_more: bool,
    pub has_more_next: bool,

    // Cached filtered chats for zero-allocation scrolling
    pub cached_filtered_chats: Vec<Chat>,
    pub last_filter_key: (u64, String, ChatFilter, bool),
    // Last ChatStore::messages_version seen by the render cache
    pub last_messages_version: u64,

    // Cached messages for 60fps scrolling
    pub cached_chat_id: Option<String>,
    pub cached_render_messages: Vec<RenderMessage>,

    // Scroll handles for visual scrollbars (uniform_list and virtual list)
    pub chat_list_scroll_handle: UniformListScrollHandle,
    pub messages_list_state: ListState,
    /// Messages that just arrived live (id -> arrival). They play a short
    /// entrance; history, scroll-back and chat switches never do.
    pub(crate) fresh_msgs: std::collections::HashMap<String, std::time::Instant>,

    // Conversation sidebar width and its live resize drag (web parity)
    pub sidebar_width: f32,
    /// Sidebar layout only: the chat list is hidden while a chat is open.
    pub sidebar_collapsed: bool,
    /// `(pointer x, sidebar width)` captured when the divider drag started.
    pub sidebar_resize_drag: Option<(f32, f32)>,

    // Context menu popup on message right click
    pub context_menu: Option<MessageContextMenu>,
    /// Message being forwarded: (source chat id, message).
    pub forward_msg: Option<(String, Message)>,
    pub forward_selected: HashSet<String>,
    pub is_forwarding: bool,

    // Context menu popup on sidebar chat right click
    pub chat_context_menu: Option<ChatContextMenu>,

    // Chat Info Sheet drawer (Right side)
    pub is_info_sheet_open: bool,
    pub info_sheet_tab: InfoSheetTab,
    pub info_media: Vec<Message>,
    pub info_docs: Vec<Message>,
    pub info_links: Vec<Message>,
    pub is_loading_info: bool,
    pub is_loading_info_members: bool,
    pub viewed_by_message: Option<Message>,
    pub viewed_by_receipts: Vec<MessageReceipt>,
    pub is_loading_viewed_by: bool,

    // Chat Search Sheet drawer (Right side)
    pub is_search_sheet_open: bool,
    pub search_sheet_query: String,
    pub search_sheet_focus: FocusHandle,
    pub search_sheet_results: Vec<Message>,
    pub is_searching_messages: bool,

    // Right-hand drawers play an exit animation before they unmount
    pub info_sheet_closing: bool,
    pub search_sheet_closing: bool,
    pub highlighted_message_id: Option<String>,

    // New Group modal
    pub is_new_group_open: bool,
    pub new_group_name: String,
    pub new_group_selected: HashSet<String>,
    pub new_group_focus: FocusHandle,
    pub is_creating_group: bool,

    // Attach (+) menu, emoji picker and compose dialogs (web parity)
    pub attach_panel: AttachPanel,
    pub emoji_picker: EmojiPickerState,
    pub is_md_mode: bool,
    pub compose_dialog: Option<ComposeDialog>,
    pub poll_draft: PollDraft,
    pub location_draft: LocationDraft,
    pub contact_draft: ContactDraft,
    pub sticker_picker: StickerPickerState,
    // Focus handle shared by every dialog input; `active_compose_field` selects
    // which field receives the keystrokes.
    pub compose_field_focus: FocusHandle,
    pub active_compose_field: usize,
    pub is_sending_attachment: bool,

    // Full-screen image preview
    pub preview_image_url: Option<String>,
    /// Message behind the open preview: (chat id, message). Enables Forward
    /// and prev/next from the viewer; `None` for images opened from the info panel.
    pub preview_msg: Option<(String, Message)>,
    /// Viewer zoom (1.0 = fit to the window) and pan offset from the centre.
    pub preview_zoom: f32,
    pub preview_pan: (f32, f32),
    /// Pointer-drag pan in progress: (mouse x, mouse y, pan x, pan y) at the start.
    pub preview_drag: Option<(f32, f32, f32, f32)>,
    pub preview_focus: FocusHandle,
    pub preview_needs_focus: bool,
    /// Viewer bounds from the last frame (for zoom-to-cursor math).
    pub preview_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,

    // Join Group modal
    pub is_join_group_open: bool,
    pub join_group_link: String,
    pub join_group_focus: FocusHandle,
    pub is_joining_group: bool,
}

impl ChatView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            search_query: String::new(),
            search_focus_handle: cx.focus_handle(),
            filter: ChatFilter::All,
            compose_text: String::new(),
            compose_input: None,
            compose_textarea: None,
            _compose_input_subscription: None,
            _compose_textarea_subscription: None,
            archived_mode: false,
            selected_chat_id: None,
            reply_to: None,
            editing_message: None,
            mention_query: None,
            group_info: None,
            is_sending: false,
            is_loading_chats: false,
            is_loading_messages: false,
            is_loading_more: false,
            is_loading_newer: false,
            has_more: true,
            has_more_next: false,

            cached_filtered_chats: Vec::new(),
            last_filter_key: (u64::MAX, String::new(), ChatFilter::All, false),
            last_messages_version: u64::MAX,

            cached_chat_id: None,
            cached_render_messages: Vec::new(),

            chat_list_scroll_handle: UniformListScrollHandle::new(),
            messages_list_state: ListState::new(0, ListAlignment::Top, px(200.0)),
            fresh_msgs: std::collections::HashMap::new(),

            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            sidebar_resize_drag: None,
            sidebar_collapsed: false,

            context_menu: None,
            forward_msg: None,
            forward_selected: HashSet::new(),
            is_forwarding: false,
            chat_context_menu: None,

            is_info_sheet_open: false,
            info_sheet_tab: InfoSheetTab::Media,
            info_media: Vec::new(),
            info_docs: Vec::new(),
            info_links: Vec::new(),
            is_loading_info: false,
            is_loading_info_members: false,
            viewed_by_message: None,
            viewed_by_receipts: Vec::new(),
            is_loading_viewed_by: false,

            is_search_sheet_open: false,
            search_sheet_query: String::new(),
            search_sheet_focus: cx.focus_handle(),
            search_sheet_results: Vec::new(),
            is_searching_messages: false,

            info_sheet_closing: false,
            search_sheet_closing: false,
            highlighted_message_id: None,

            is_new_group_open: false,
            new_group_name: String::new(),
            new_group_selected: HashSet::new(),
            new_group_focus: cx.focus_handle(),
            is_creating_group: false,

            preview_image_url: None,
            preview_msg: None,
            preview_zoom: 1.0,
            preview_pan: (0.0, 0.0),
            preview_drag: None,
            preview_focus: cx.focus_handle(),
            preview_needs_focus: false,
            preview_bounds: std::rc::Rc::new(std::cell::Cell::new(Bounds::default())),

            is_join_group_open: false,

            attach_panel: AttachPanel::None,
            emoji_picker: EmojiPickerState::new(),
            is_md_mode: false,
            compose_dialog: None,
            poll_draft: PollDraft::new(),
            location_draft: LocationDraft::new(),
            contact_draft: ContactDraft::new(),
            sticker_picker: StickerPickerState::new(),
            compose_field_focus: cx.focus_handle(),
            active_compose_field: 0,
            is_sending_attachment: false,
            join_group_link: String::new(),
            join_group_focus: cx.focus_handle(),
            is_joining_group: false,
        };
        view.load_chats(cx);
        view
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_composer_inputs(window, cx);
        if self.preview_needs_focus && self.preview_image_url.is_some() {
            // Keys (Esc, arrows, +/-) go to the viewer as soon as it opens.
            self.preview_focus.focus(window, cx);
            self.preview_needs_focus = false;
        }
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;

        let (chats_version, archived_count) = if cx.has_global::<ChatStore>() {
            let store = ChatStore::global(cx);
            (store.chats_version, store.chats.iter().filter(|c| c.archived).count())
        } else {
            (0, 0)
        };

        // Cache filtered and sorted chats to avoid expensive allocations during 60fps scrolling
        let filter_key = (chats_version, self.search_query.clone(), self.filter, self.archived_mode);
        if self.last_filter_key != filter_key {
            let empty_chats = Vec::new();
            let all_chats = if cx.has_global::<ChatStore>() {
                &ChatStore::global(cx).chats
            } else {
                &empty_chats
            };

            let query = self.search_query.trim().to_lowercase();
            let archived_mode = self.archived_mode;
            let current_filter = self.filter;

            let mut filtered: Vec<Chat> = all_chats
                .iter()
                .filter(|c| c.archived == archived_mode)
                .filter(|c| match current_filter {
                    ChatFilter::All => true,
                    ChatFilter::Unread => c.unread > 0,
                    ChatFilter::Groups => c.is_group,
                    ChatFilter::Contacts => !c.is_group,
                })
                .filter(|c| {
                    if query.is_empty() {
                        true
                    } else {
                        c.name.to_lowercase().contains(&query)
                            || c.id.to_lowercase().contains(&query)
                            || c.last_msg.to_lowercase().contains(&query)
                    }
                })
                .cloned()
                .collect();

            // Sort: pinned first, then last_time descending
            filtered.sort_by(|a, b| {
                match (a.pinned_at, b.pinned_at) {
                    (Some(pa), Some(pb)) => pb.cmp(&pa),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => b.last_time.cmp(&a.last_time),
                }
            });

            self.cached_filtered_chats = filtered;
            self.last_filter_key = filter_key;
        }
        let pinned_count = self.cached_filtered_chats.iter().filter(|chat| chat.pinned_at.is_some()).count();

        let selected_chat = self
            .selected_chat_id
            .as_ref()
            .and_then(|id| {
                if cx.has_global::<ChatStore>() {
                    ChatStore::global(cx).chats.iter().find(|c| c.id == *id).cloned()
                } else {
                    None
                }
            });

        // Layout: pages (list OR conversation) or sidebar (list beside it).
        let layout = layout_pref::effective(cx);
        let pages = layout == LayoutKind::Pages;
        let has_selection = selected_chat.is_some();
        let show_list = if pages { !has_selection } else { !(self.sidebar_collapsed && has_selection) };
        let show_right = if pages { has_selection } else { true };

        // Ensure cached messages match currently selected chat, and rebuild
        // when the store reports any change (new/edited/deleted messages,
        // status ticks, reactions) so live WS updates repaint the bubbles.
        let messages_version = if cx.has_global::<ChatStore>() {
            ChatStore::global(cx).messages_version
        } else {
            0
        };
        if let Some(ref sc) = selected_chat {
            if self.cached_chat_id.as_deref() != Some(&sc.id) {
                if cx.has_global::<ChatStore>() {
                    if let Some(entry) = ChatStore::global(cx).messages_by_chat.get(&sc.id) {
                        self.rebuild_message_cache(&sc.id, &entry.messages);
                    }
                }
            } else if self.last_messages_version != messages_version {
                if cx.has_global::<ChatStore>() {
                    if let Some(entry) = ChatStore::global(cx).messages_by_chat.get(&sc.id) {
                        self.rebuild_message_cache_preserve_scroll(&sc.id, &entry.messages);
                    }
                }
            }
        }
        self.last_messages_version = messages_version;

        h_flex()
            .size_full()
            .overflow_hidden()
            .relative()
            // ====================================================
            // LEFT SIDEBAR: CONVERSATIONS LIST
            // ====================================================
            .child(self.render_sidebar(cx, pages, show_list, has_selection, archived_count, pinned_count))
            // ====================================================
            // RIGHT PANE: ACTIVE CHAT CONVERSATION OR EMPTY STATE
            // ====================================================
            .child(self.render_conversation_pane(window, cx, &selected_chat, pages, show_right))
            // ====================================================
            // RIGHT DRAWER: CHAT INFO SHEET
            // ====================================================
            .children(if (self.is_info_sheet_open || self.info_sheet_closing)
                && selected_chat.is_some()
            {
                let chat = selected_chat.clone().unwrap();

                Some(self.render_info_sheet(window, cx, &chat))
            } else if (self.is_search_sheet_open || self.search_sheet_closing)
                && selected_chat.is_some()
            {
                let chat = selected_chat.clone().unwrap();
                Some(Self::animated_sheet(
                    "chat-search-sheet-in",
                    "chat-search-sheet-out",
                    self.search_sheet_closing,
                    self.render_search_sheet(&chat, theme, border_color, bg_color, card_bg, primary_color, text_color, muted_text, cx),
                ))
            } else {
                None
            })
            // ====================================================
            // CONTEXT MENU POPOVER (RIGHT-CLICK ON BUBBLE)
            // ====================================================
            .children(self.render_message_context_menu(window, cx))
            // ====================================================
            // CHAT SIDEBAR CONTEXT MENU POPOVER (RIGHT-CLICK ON CHAT)
            // ====================================================
            .children(self.render_chat_context_menu(window, cx))
            // ====================================================
            // MODAL: NEW GROUP DIALOG
            // ====================================================
            // ====================================================
            // MODAL: POLL / LOCATION / CONTACT COMPOSE DIALOGS
            // ====================================================
            .children(self.render_compose_dialog(
                theme,
                border_color,
                card_bg,
                primary_color,
                text_color,
                muted_text,
                cx,
            ))
            .children(self.render_new_group_dialog(cx))
            // ====================================================
            // MODAL: FORWARD MESSAGE
            // ====================================================
            .children(self.render_forward_dialog(cx))
            // ====================================================
            // MODAL: JOIN GROUP DIALOG
            // ====================================================
            .children(self.render_join_group_dialog(cx))
            // ====================================================
            // CONVERSATION SIDEBAR RESIZE HANDLE (web parity)
            // ====================================================
            .child(self.render_sidebar_resize_handle(cx, pages, show_list))
            // While dragging, a full-screen catcher keeps the pointer glued to
            // the divider however fast it moves (and off the chat underneath).
            .children(self.render_sidebar_resize_overlay(cx))
            // Full-screen image viewer: fit-to-window, wheel zoom around the
            // cursor, drag to pan, double-click to zoom, Esc / click outside to close.
            .children(self.render_image_viewer(window, cx))
            .children(self.render_viewed_by_dialog(cx))
    }
}
