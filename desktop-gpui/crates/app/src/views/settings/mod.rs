//! Settings view matching Web SettingsPage.tsx and web-term GPUI reference.
//!
//! Features:
//! - Max-w-2xl centered scrollable layout.
//! - APPEARANCE:
//!   - Theme mode filter dropdown (All themes, Dark, Light) with auto-matching.
//!   - Color theme 3-column swatch cards for all 81 web presets with live preview and instant switching.
//! - ACCOUNT:
//!   - WhatsApp connection indicator and status.
//!   - Logout button with confirmation dialog.
//!   - Chat history sync with a depth choice (per chat / last N days / everything),
//!     live progress (chats, current chat, ETA), and cancel.
//! - AI CONFIGURATION:
//!   - Gemini API Key (masked status + modal editing).
//!   - AI Server URL with default presets.
//!   - Asynchronous save to backend `/api/settings`.
//! - PRIVACY:
//!   - Read receipts toggle switch (blue ticks on/off).
//! - CALL TTS:
//!   - TTS Provider picker (Disabled, Fish Audio, Edge TTS).
//!   - Default voice, Fish Audio API key, model, and voice ID.
//! - CONNECTION INFO:
//!   - Active backend API endpoint and settings status.

mod ai;
mod backup;
mod controls;
mod data;
mod history;
mod history_media;
mod history_sync;
mod ui;
use ui::settings_section_heading;

use std::collections::HashMap;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::Disableable;
use crate::theme::shape::{ShapeExt, ShapeTokens};
use crate::theme::surface::SurfaceExt;
use controls::Palette;
use history_sync::SyncDepth;
use crate::components::ui::{
    ButtonVariant, Icon, IconName, Scrollbar, ScrollbarMode, Slider, SliderEvent, SliderState,
    SliderValue, Switch, action_button, card as card_surface, card_group, settings_group, h_flex,
    dropdown_menu::{dropdown_list, DropdownMenuStyle},
};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{
    BackupEntry, BackupStatus, HistoryMediaRequest, HistoryMediaStart, HistoryMediaStatus, HistorySyncRequest, HistorySyncStart, HistorySyncStatus,
    StorageUsage,
};
use wabot_backend_client::ClientError;
use wabot_settings::{
    ChatLayout, DesktopSettings, MAX_FONT_SCALE, MAX_WINDOW_TRANSPARENCY, MIN_FONT_SCALE, ThemeMode as SettingsThemeMode,
};

use crate::components::connection_banner::{ConnectionState, ConnectionStatus};
use crate::components::ui::{TextField, TextFieldEvent};
use crate::components::toast;
use crate::components::dialogs::logout::open_logout_dialog;
use crate::router::{AppRoute, Router};
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::color::parse_hex_hsla;
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;
use std::borrow::Cow;

use crate::theme::preset::{all_presets, ThemePreset};
use crate::TOKIO_RT;
use crate::updater::{self, InstallKind, UpdateInfo};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;


/// State of the manual "Check for updates" flow.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(UpdateInfo),
    Downloading(UpdateInfo),
    ReadyToRestart,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditField {
    GeminiApiKey,
    AiServerUrl,
    AiBaseUrl,
    AiApiKey,
    AiModel,
    TtsDefaultVoice,
    FishAudioKey,
    FishAudioModel,
    FishAudioVoiceId,
}

#[derive(Clone, Debug)]
pub struct EditModalState {
    pub field: EditField,
    pub title: String,
    pub description: String,
    pub value: String,
    pub is_secret: bool,
    pub presets: Vec<(&'static str, &'static str)>,
}

pub struct SettingsView {
    pub theme_mode_filter: String,
    pub show_theme_mode_picker: bool,
    pub show_theme_modal: bool,
    pub show_history_depth_picker: bool,
    window_transparency_slider: Entity<SliderState>,
    _window_transparency_subscription: Subscription,
    ui_font_scale_slider: Entity<SliderState>,
    _ui_font_scale_subscription: Subscription,
    chat_font_scale_slider: Entity<SliderState>,
    _chat_font_scale_subscription: Subscription,
    settings_scroll_handle: ScrollHandle,

    // Backend settings
    pub settings_loaded: bool,
    pub is_loading_settings: bool,
    pub has_gemini_key: bool,
    pub has_fish_key: bool,
    pub gemini_api_key_input: String,
    pub ai_server_url: String,
    /// "gemini", "openai" (OpenAI-compatible) or "claude" (Claude-compatible).
    pub ai_provider: String,
    pub show_ai_provider_picker: bool,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_api_key_input: String,
    pub has_ai_key: bool,
    pub read_receipts: bool,
    pub contact_name_mode: String,

    // TTS settings
    pub tts_provider: String,
    pub show_tts_provider_picker: bool,
    pub tts_default_voice: String,
    pub fish_audio_key_input: String,
    pub fish_audio_model: String,
    pub fish_audio_voice_id: String,

    // Account & sync state
    pub history_status: Option<HistorySyncStatus>,
    pub is_syncing_history: bool,
    /// How far back the next sync goes.
    pub history_depth: SyncDepth,
    /// Also ask again for chats already synced to their start.
    pub history_retry_finished: bool,
    /// A loop is already polling the sync status.
    history_polling: bool,
    // Download of the media of history messages
    pub media_status: Option<HistoryMediaStatus>,
    pub is_downloading_media: bool,
    /// The kinds of media the next download fetches (backend names).
    pub media_kinds: Vec<String>,
    pub media_retry_unavailable: bool,
    /// Ask the phone to upload media again that WhatsApp's servers lost.
    pub media_ask_phone: bool,
    media_polling: bool,
    /// Sizes of the chat and media databases.
    pub storage_usage: Option<StorageUsage>,
    pub is_restarting_backend: bool,

    // Backup
    pub backup_folder: Option<String>,
    pub backup_status: Option<BackupStatus>,
    pub backups: Vec<BackupEntry>,
    pub is_backing_up: bool,
    pub backup_polling: bool,
    pub restore_status: Option<BackupStatus>,
    pub is_restoring: bool,
    pub restore_polling: bool,
    /// Which run the status area shows: the latest restore or the latest backup.
    pub last_was_restore: bool,

    // App updates (GitHub Releases)
    pub update_status: UpdateStatus,
    /// (bytes downloaded, total bytes) of the running download.
    update_progress: Arc<(AtomicU64, AtomicU64)>,

    // Asynchronous action states
    pub is_saving_ai: bool,
    pub is_saving_tts: bool,
    pub is_saving_receipts: bool,

    // Modal editing state
    pub edit_modal: Option<EditModalState>,
    pub modal_focus_handle: FocusHandle,
    pub modal_field: Option<TextField>,
}
