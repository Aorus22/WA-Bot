use crate::views::chat::*;

pub struct ChatView {
    // Text inputs (created on first render, when a window is available). Each
    // mirrors its edits into the matching `String` model below.
    pub(crate) search_field: Option<TextField>,
    pub(crate) sheet_search_field: Option<TextField>,
    pub(crate) new_group_field: Option<TextField>,
    pub(crate) join_link_field: Option<TextField>,
    /// Compose-dialog inputs by key (`poll-question`, `poll-option-2`, ...).
    pub(crate) dialog_fields: std::collections::HashMap<String, TextField>,
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
    /// Highlighted row of the @-mention list (arrow keys move it).
    pub mention_index: usize,
    pub mention_scroll: ScrollHandle,
    pub(crate) _mention_key_interceptor: Option<Subscription>,
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
    /// Text scales (interface, chat) the list's cached row heights were measured at.
    pub measured_text_scale: (u8, u8),
    /// Messages that just arrived live (id -> arrival). They play a short
    /// entrance; history, scroll-back and chat switches never do.
    pub(crate) fresh_msgs: std::collections::HashMap<String, std::time::Instant>,
    /// Typing bubble: (chat id, typist JID, name) shown last, kept for the exit animation.
    pub(crate) typing_shown: Option<(String, String, String)>,
    /// When the typing bubble started leaving (nobody is typing any more).
    pub(crate) typing_left_at: Option<std::time::Instant>,
    /// Counts typing sessions so each one replays the pop-in animation.
    pub(crate) typing_session: u32,

    // Conversation sidebar width and its live resize drag (web parity)
    pub sidebar_width: f32,
    /// Sidebar layout only: the chat list is hidden while a chat is open.
    pub sidebar_collapsed: bool,
    /// The chat list is sliding shut; it stays mounted until that ends.
    pub sidebar_exiting: bool,
    /// Set after the first toggle so the list never animates on startup.
    pub sidebar_animate: bool,
    /// `(pointer x, sidebar width)` captured when the divider drag started.
    pub sidebar_resize_drag: Option<(f32, f32)>,

    // Context menu popup on message right click
    pub context_menu: Option<MessageContextMenu>,
    /// Right-click menu for a link or selected text inside a bubble.
    pub text_menu: Option<crate::components::mention_text::TextMenuRequest>,
    pub edit_history: Option<EditHistoryPopover>,
    pub image_menu: Option<ImageMenu>,
    /// Message being forwarded: (source chat id, message).
    pub forward_msg: Option<(String, Message)>,
    pub forward_selected: HashSet<String>,
    pub is_forwarding: bool,

    // Context menu popup on sidebar chat right click
    pub chat_context_menu: Option<ChatContextMenu>,
    /// Dropdown opened from a header button.
    pub header_menu: Option<HeaderMenu>,
    /// Where the pointer went down on the header's avatar and name: a click
    /// there opens the info sheet, a drag moves the window.
    pub(crate) header_press: Option<Point<Pixels>>,
    /// The header menu a click outside it just closed, and when. The button that
    /// opened it sees that same click right after, and must not reopen it.
    pub(crate) header_menu_dismissed: Option<(HeaderMenuKind, std::time::Instant)>,
    /// When set, the contact picker adds members to this group instead of creating one.
    pub add_members_for: Option<String>,
    pub is_exporting_chat: bool,
    /// Select-messages mode: bubbles get checkboxes and the header becomes an action bar.
    pub select_mode: bool,
    pub selected_messages: HashSet<String>,
    /// Delete in the selection bar needs a second click.
    pub select_confirm_delete: bool,
    /// Messages a multi-forward sends, in order (empty for a single forward).
    pub forward_ids: Vec<String>,
    /// Select-chats mode in the chat list.
    pub chat_select_mode: bool,
    pub selected_chats: HashSet<String>,

    // Chat Info Sheet drawer (Right side)
    pub is_info_sheet_open: bool,
    pub info_sheet_tab: InfoSheetTab,
    /// The info drawer shows the Media / Docs / Links page instead of the main one.
    pub info_media_open: bool,
    /// Exit group needs a second click.
    pub info_confirm_exit: bool,
    pub info_media: Vec<Message>,
    pub info_docs: Vec<Message>,
    pub info_links: Vec<Message>,
    pub is_loading_info: bool,
    pub is_loading_info_members: bool,
    pub viewed_by_message: Option<Message>,
    pub viewed_by_receipts: Vec<MessageReceipt>,
    /// Members besides us the message info sheet's message went to.
    pub viewed_by_recipients: Vec<String>,
    pub viewed_by_closing: bool,
    pub is_loading_viewed_by: bool,

    // Starred messages drawer (Right side): every starred message, all chats
    pub is_starred_sheet_open: bool,
    pub starred_sheet_closing: bool,
    pub starred_messages: Vec<Message>,
    pub starred_has_more: bool,
    pub is_loading_starred: bool,
    /// Last `ChatStore::starred_version` seen, to reload the list on a change made elsewhere.
    pub starred_version_seen: u64,
    /// `(chat id, message id)` to scroll to once that chat has loaded (opened from the starred list).
    pub(crate) pending_jump: Option<(String, String)>,

    // Pinned messages of the open chat, newest first, and the banner's cursor
    pub pinned_messages: Vec<PinnedMessage>,
    pub pin_banner_index: usize,

    // Link preview card above the composer, and what feeds it
    pub(crate) compose_link: Option<ComposeLinkPreview>,
    /// Previews fetched so far by URL; `None` is a URL nothing could be scraped from.
    pub(crate) link_preview_cache: std::collections::HashMap<String, Option<LinkPreview>>,
    /// URLs whose card was dismissed: they go out as plain text.
    pub(crate) dismissed_link_previews: HashSet<String>,
    /// Bumped on every edit of the text, so a debounced fetch can tell it is stale.
    pub(crate) link_preview_seq: u64,

    // Voice note being recorded
    pub(crate) voice: Option<VoiceDraft>,
    pub(crate) voice_seq: u64,

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
    /// Pasted image staged in the composer until send.
    pub pending_attachment: Option<PendingAttachment>,

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
    /// Full-screen video player, while one is open.
    pub(crate) video_view: Option<super::video_viewer::VideoView>,
    pub(crate) video_focus: FocusHandle,
    pub(crate) video_needs_focus: bool,
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
        if cx.has_global::<ChatStore>() {
            ChatStore::global_mut(cx).active_chat_id = None;
        }
        let mut view = Self {
            search_field: None,
            sheet_search_field: None,
            new_group_field: None,
            join_link_field: None,
            dialog_fields: std::collections::HashMap::new(),
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
            mention_index: 0,
            mention_scroll: ScrollHandle::new(),
            _mention_key_interceptor: None,
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
            // At the bottom the list follows new messages by itself; scrolling
            // up pauses that until the user returns to the bottom.
            messages_list_state: {
                let state = ListState::new(0, ListAlignment::Top, px(200.0));
                state.set_follow_mode(FollowMode::Tail);
                state
            },
            measured_text_scale: (100, 100),
            fresh_msgs: std::collections::HashMap::new(),
            typing_shown: None,
            typing_left_at: None,
            typing_session: 0,

            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            sidebar_resize_drag: None,
            sidebar_collapsed: false,
            sidebar_exiting: false,
            sidebar_animate: false,

            context_menu: None,
            text_menu: None,
            edit_history: None,
            image_menu: None,
            forward_msg: None,
            forward_selected: HashSet::new(),
            is_forwarding: false,
            chat_context_menu: None,
            header_menu: None,
            header_press: None,
            header_menu_dismissed: None,
            add_members_for: None,
            is_exporting_chat: false,
            select_mode: false,
            selected_messages: HashSet::new(),
            select_confirm_delete: false,
            forward_ids: Vec::new(),
            chat_select_mode: false,
            selected_chats: HashSet::new(),

            is_info_sheet_open: false,
            info_sheet_tab: InfoSheetTab::Media,
            info_media_open: false,
            info_confirm_exit: false,
            info_media: Vec::new(),
            info_docs: Vec::new(),
            info_links: Vec::new(),
            is_loading_info: false,
            is_loading_info_members: false,
            viewed_by_message: None,
            viewed_by_receipts: Vec::new(),
            viewed_by_recipients: Vec::new(),
            viewed_by_closing: false,
            is_loading_viewed_by: false,

            is_starred_sheet_open: false,
            starred_sheet_closing: false,
            starred_messages: Vec::new(),
            starred_has_more: false,
            is_loading_starred: false,
            starred_version_seen: 0,
            pending_jump: None,

            pinned_messages: Vec::new(),
            pin_banner_index: 0,

            compose_link: None,
            link_preview_cache: std::collections::HashMap::new(),
            dismissed_link_previews: HashSet::new(),
            link_preview_seq: 0,

            voice: None,
            voice_seq: 0,

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
            video_view: None,
            video_focus: cx.focus_handle(),
            video_needs_focus: false,
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
            pending_attachment: None,
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
        self.ensure_text_fields(window, cx);
        if self.preview_needs_focus && self.preview_image_url.is_some() {
            // Keys (Esc, arrows, +/-) go to the viewer as soon as it opens.
            self.preview_focus.focus(window, cx);
            self.preview_needs_focus = false;
        }
        if self.video_needs_focus && self.video_view.is_some() {
            // Space, arrows and Esc go to the player as soon as it opens.
            self.video_focus.focus(window, cx);
            self.video_needs_focus = false;
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
        let layout = layout_pref::effective_for_width(cx, f32::from(window.viewport_size().width), self.sidebar_width);
        let pages = layout == LayoutKind::Pages;
        let has_selection = selected_chat.is_some();
        let show_list = if pages {
            !has_selection
        } else {
            !(self.sidebar_collapsed && has_selection) || self.sidebar_exiting
        };
        let show_right = if pages { has_selection } else { true };
        let starred_overlay = pages && !has_selection;

        // The backend re-resolved names (or the chat list changed): fetch again
        // what is on screen, since nothing else would.
        let (reload_chats, reload_names) = if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            (std::mem::take(&mut store.chats_stale), std::mem::take(&mut store.names_changed))
        } else {
            (false, false)
        };
        if reload_chats {
            self.load_chats(cx);
        }
        // Pins changed elsewhere, or a message starred on another device.
        let (stale_pins, starred_version) = if cx.has_global::<ChatStore>() {
            let store = ChatStore::global_mut(cx);
            (store.take_stale_pins(), store.starred_version)
        } else {
            (Vec::new(), 0)
        };
        if let Some(chat) = selected_chat.as_ref() {
            if stale_pins.contains(&chat.id) {
                self.load_pins(&chat.id, cx);
            }
        }
        if starred_version != self.starred_version_seen {
            self.starred_version_seen = starred_version;
            if self.is_starred_sheet_open {
                self.load_starred(false, cx);
            }
        }
        if reload_names {
            if let Some(chat) = selected_chat.as_ref() {
                self.refresh_messages_in_place(&chat.id, cx);
                if chat.is_group {
                    self.load_group_info(&chat.id, cx);
                }
            }
        }

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
            .children(if (self.is_starred_sheet_open || self.starred_sheet_closing) && !starred_overlay {
                Some(Self::animated_sheet(
                    "starred-sheet-in",
                    "starred-sheet-out",
                    self.starred_sheet_closing,
                    self.render_starred_sheet(false, cx),
                ))
            } else if (self.is_info_sheet_open || self.info_sheet_closing)
                && selected_chat.is_some()
            {
                let chat = selected_chat.clone().unwrap();

                // The pages layout has no column to spare: the info page covers the window.
                if pages {
                    // `occlude`: without it a click on the sheet's close button also reached the header's buttons underneath.
                    Some(div().absolute().inset_0().occlude().child(self.render_info_sheet(window, cx, &chat, true)).into_any_element())
                } else {
                    Some(self.render_info_sheet(window, cx, &chat, false))
                }
            } else if (self.viewed_by_message.is_some() || self.viewed_by_closing)
                && selected_chat.is_some()
            {
                Some(Self::animated_sheet(
                    "message-info-sheet-in",
                    "message-info-sheet-out",
                    self.viewed_by_closing,
                    self.render_message_info_sheet(window, cx),
                ))
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
            // The chat list page has no column to spare for a drawer, so the
            // starred messages cover it instead.
            .children((starred_overlay && (self.is_starred_sheet_open || self.starred_sheet_closing)).then(|| {
                div().absolute().inset_0().occlude().child(self.render_starred_sheet(true, cx))
            }))
            // ====================================================
            // CONTEXT MENU POPOVER (RIGHT-CLICK ON BUBBLE)
            // ====================================================
            .children(self.render_message_context_menu(window, cx))
            .children(self.render_text_menu(window, cx))
            .children(self.render_edit_history(window, cx))
            .children(self.render_image_menu(window, cx))
            // ====================================================
            // CHAT SIDEBAR CONTEXT MENU POPOVER (RIGHT-CLICK ON CHAT)
            // ====================================================
            .children(self.render_chat_context_menu(window, cx))
            .children(self.render_header_menu(window, cx))
            // ====================================================
            // MODAL: NEW GROUP DIALOG
            // ====================================================
            // ====================================================
            // MODAL: POLL / LOCATION / CONTACT COMPOSE DIALOGS
            // ====================================================
            .children(self.render_compose_dialog(
                window,
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
            .children(self.render_video_viewer(window, cx))
    }
}
