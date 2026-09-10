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
//!   - Chat history sync ("Sync 50 per chat") with live progress bar and status counters.
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

use std::collections::HashMap;
use gpui::*;
use gpui_component::{Icon, IconName};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::HistorySyncStatus;
use wabot_settings::{DesktopSettings, ThemeMode as SettingsThemeMode};

use crate::components::connection_banner::{ConnectionState, ConnectionStatus};
use crate::components::dialogs::logout::open_logout_dialog;
use crate::router::{AppRoute, Router};
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::color::parse_hex_hsla;
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::theme::preset::{all_presets, ThemePreset};
use crate::TOKIO_RT;

#[derive(Clone, Debug, PartialEq)]
pub enum EditField {
    GeminiApiKey,
    AiServerUrl,
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

    // Backend settings
    pub settings_loaded: bool,
    pub is_loading_settings: bool,
    pub has_gemini_key: bool,
    pub has_fish_key: bool,
    pub gemini_api_key_input: String,
    pub ai_server_url: String,
    pub read_receipts: bool,

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

    // Asynchronous action states
    pub is_saving_ai: bool,
    pub is_saving_tts: bool,
    pub is_saving_receipts: bool,
    pub toast_message: Option<(String, bool)>, // (text, is_error)

    // Modal editing state
    pub edit_modal: Option<EditModalState>,
    pub modal_focus_handle: FocusHandle,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let modal_focus_handle = cx.focus_handle();

        // Load initial theme mode filter from settings
        let theme_mode_filter = if let Ok(settings) = DesktopSettings::load() {
            match settings.theme_mode {
                SettingsThemeMode::Dark => "dark".to_string(),
                SettingsThemeMode::Light => "light".to_string(),
                _ => "all".to_string(),
            }
        } else {
            "all".to_string()
        };

        let mut view = Self {
            theme_mode_filter,
            show_theme_mode_picker: false,

            settings_loaded: false,
            is_loading_settings: false,
            has_gemini_key: false,
            has_fish_key: false,
            gemini_api_key_input: String::new(),
            ai_server_url: "http://localhost:8981".to_string(),
            read_receipts: true,

            tts_provider: String::new(),
            show_tts_provider_picker: false,
            tts_default_voice: "en-US-JennyNeural".to_string(),
            fish_audio_key_input: String::new(),
            fish_audio_model: "fishaudio/fish-speech-1.5".to_string(),
            fish_audio_voice_id: String::new(),

            history_status: None,
            is_syncing_history: false,

            is_saving_ai: false,
            is_saving_tts: false,
            is_saving_receipts: false,
            toast_message: None,

            edit_modal: None,
            modal_focus_handle,
        };

        view.load_backend_settings(cx);
        view.load_history_status(cx);

        view
    }

    /// Load server-backed settings via GET /api/settings.
    pub fn load_backend_settings(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_settings {
            return;
        }
        self.is_loading_settings = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_settings().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, _cx| {
                            if let Ok(Ok(settings_resp)) = res {
                                this.has_gemini_key = settings_resp.has_gemini_key.unwrap_or(false);
                                this.has_fish_key = settings_resp.has_fish_key.unwrap_or(false);
                                this.ai_server_url = settings_resp.ai_server_url().to_string();
                                this.read_receipts = settings_resp.read_receipts();
                                this.tts_provider = settings_resp.call_tts_provider().to_string();
                                this.tts_default_voice = settings_resp.call_tts_default_voice().to_string();
                                this.fish_audio_model = settings_resp.call_tts_fish_audio_model().to_string();
                                this.fish_audio_voice_id = settings_resp.call_tts_fish_audio_voice_id().to_string();
                                this.settings_loaded = true;
                                this.is_loading_settings = false;
                            } else {
                                this.is_loading_settings = false;
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load history sync status via GET /api/history-sync/status.
    pub fn load_history_status(&mut self, cx: &mut Context<Self>) {
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.get_history_sync_status().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, _cx| {
                            if let Ok(Ok(status)) = res {
                                this.is_syncing_history = status.state == "running";
                                this.history_status = Some(status);
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Start chat history sync via POST /api/history-sync.
    pub fn start_history_sync(&mut self, cx: &mut Context<Self>) {
        if self.is_syncing_history {
            return;
        }
        self.is_syncing_history = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.start_history_sync().await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(status)) => {
                                    this.is_syncing_history = status.state == "running";
                                    this.history_status = Some(status);
                                    this.toast_message = Some(("History sync started".into(), false));
                                }
                                _ => {
                                    this.is_syncing_history = false;
                                    this.toast_message = Some(("Failed to start history sync".into(), true));
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Save AI settings via PUT /api/settings.
    pub fn save_ai_settings(&mut self, cx: &mut Context<Self>) {
        if self.is_saving_ai {
            return;
        }
        self.is_saving_ai = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let mut data = HashMap::new();
        data.insert("ai_server_url".to_string(), self.ai_server_url.trim().to_string());
        if !self.gemini_api_key_input.trim().is_empty() {
            data.insert("gemini_api_key".to_string(), self.gemini_api_key_input.trim().to_string());
        }

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.update_settings(&data).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_saving_ai = false;
                            match res {
                                Ok(Ok(resp)) => {
                                    this.has_gemini_key = resp.has_gemini_key.unwrap_or(this.has_gemini_key);
                                    this.gemini_api_key_input.clear();
                                    this.toast_message = Some(("AI configuration saved".into(), false));
                                }
                                _ => {
                                    this.toast_message = Some(("Failed to save AI configuration".into(), true));
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Toggle Read Receipts switch via PUT /api/settings.
    pub fn toggle_read_receipts(&mut self, cx: &mut Context<Self>) {
        if self.is_saving_receipts {
            return;
        }
        self.is_saving_receipts = true;
        let new_value = !self.read_receipts;
        self.read_receipts = new_value;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let mut data = HashMap::new();
        data.insert("read_receipts".to_string(), if new_value { "true" } else { "false" }.to_string());

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.update_settings(&data).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_saving_receipts = false;
                            match res {
                                Ok(Ok(resp)) => {
                                    this.read_receipts = resp.read_receipts();
                                    let msg = if this.read_receipts { "Read receipts enabled" } else { "Read receipts disabled" };
                                    this.toast_message = Some((msg.into(), false));
                                }
                                _ => {
                                    this.toast_message = Some(("Failed to update read receipts".into(), true));
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Save TTS configuration via PUT /api/settings.
    pub fn save_tts_settings(&mut self, cx: &mut Context<Self>) {
        if self.is_saving_tts {
            return;
        }
        self.is_saving_tts = true;

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        let mut data = HashMap::new();
        data.insert("call_tts_provider".to_string(), self.tts_provider.clone());
        data.insert("call_tts_default_voice".to_string(), self.tts_default_voice.trim().to_string());
        data.insert("call_tts_fish_audio_model".to_string(), self.fish_audio_model.trim().to_string());
        data.insert("call_tts_fish_audio_voice_id".to_string(), self.fish_audio_voice_id.trim().to_string());
        if !self.fish_audio_key_input.trim().is_empty() {
            data.insert("call_tts_fish_audio_key".to_string(), self.fish_audio_key_input.trim().to_string());
        }

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.update_settings(&data).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_saving_tts = false;
                            match res {
                                Ok(Ok(resp)) => {
                                    this.has_fish_key = resp.has_fish_key.unwrap_or(this.has_fish_key);
                                    this.fish_audio_key_input.clear();
                                    this.toast_message = Some(("Call TTS settings saved".into(), false));
                                }
                                _ => {
                                    this.toast_message = Some(("Failed to save TTS settings".into(), true));
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Open editing modal for a specific field.
    pub fn open_edit_modal(&mut self, field: EditField, window: &mut Window, cx: &mut Context<Self>) {
        let (title, description, initial_val, is_secret, presets) = match field {
            EditField::GeminiApiKey => (
                "Gemini API Key".to_string(),
                "Enter your Google Gemini API key. Stored securely in backend database.".to_string(),
                self.gemini_api_key_input.clone(),
                true,
                vec![],
            ),
            EditField::AiServerUrl => (
                "AI Server URL".to_string(),
                "HTTP endpoint for external AI services.".to_string(),
                self.ai_server_url.clone(),
                false,
                vec![("Default Local", "http://localhost:8981")],
            ),
            EditField::TtsDefaultVoice => (
                "Default Voice".to_string(),
                "Voice name used for incoming call speech synthesis.".to_string(),
                self.tts_default_voice.clone(),
                false,
                vec![
                    ("US Jenny (Edge)", "en-US-JennyNeural"),
                    ("US Guy (Edge)", "en-US-GuyNeural"),
                    ("ID Gadis (Edge)", "id-ID-GadisNeural"),
                    ("ID Ardi (Edge)", "id-ID-ArdiNeural"),
                ],
            ),
            EditField::FishAudioKey => (
                "Fish Audio API Key".to_string(),
                "API key for Fish Audio TTS provider. Leave blank to keep existing key.".to_string(),
                self.fish_audio_key_input.clone(),
                true,
                vec![],
            ),
            EditField::FishAudioModel => (
                "Fish Audio Model".to_string(),
                "Model identifier on Fish Audio platform.".to_string(),
                self.fish_audio_model.clone(),
                false,
                vec![("Speech 1.5", "fishaudio/fish-speech-1.5")],
            ),
            EditField::FishAudioVoiceId => (
                "Fish Audio Voice ID".to_string(),
                "Custom reference Voice ID from your Fish Audio library.".to_string(),
                self.fish_audio_voice_id.clone(),
                false,
                vec![],
            ),
        };

        self.edit_modal = Some(EditModalState {
            field,
            title,
            description,
            value: initial_val,
            is_secret,
            presets,
        });

        self.modal_focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Save modal value back to view state.
    pub fn save_modal_value(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = self.edit_modal.take() {
            match modal.field {
                EditField::GeminiApiKey => self.gemini_api_key_input = modal.value,
                EditField::AiServerUrl => self.ai_server_url = modal.value,
                EditField::TtsDefaultVoice => self.tts_default_voice = modal.value,
                EditField::FishAudioKey => self.fish_audio_key_input = modal.value,
                EditField::FishAudioModel => self.fish_audio_model = modal.value,
                EditField::FishAudioVoiceId => self.fish_audio_voice_id = modal.value,
            }
            cx.notify();
        }
    }

    /// Set theme mode filter ("all", "dark", "light") and auto-match active theme.
    pub fn set_theme_mode_filter(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_mode_filter = mode.to_string();
        self.show_theme_mode_picker = false;

        let theme_mgr = ThemeManager::global(cx);
        let current_name = theme_mgr.active_preset.name.clone();
        let current_is_dark = theme_mgr.active_preset.is_dark();

        let need_switch = match mode {
            "dark" if !current_is_dark => true,
            "light" if current_is_dark => true,
            _ => false,
        };

        if need_switch {
            let base = current_name
                .trim_end_matches("-dark")
                .trim_end_matches("-light");
            let target_name = if mode == "dark" {
                format!("{}-dark", base)
            } else {
                format!("{}-light", base)
            };

            ThemeManager::apply_preset(&target_name, Some(window), cx);
        }

        // Persist theme mode preference to DesktopSettings
        let settings_mode = match mode {
            "dark" => SettingsThemeMode::Dark,
            "light" => SettingsThemeMode::Light,
            _ => SettingsThemeMode::System,
        };
        ThemeManager::apply_mode(settings_mode, Some(window), cx);

        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.app_theme();
        let card_bg = theme.card;
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let primary_color = theme.primary;
        let secondary_bg = theme.secondary;
        let tag_bg = theme.muted;

        let theme_mgr = ThemeManager::global(cx);
        let active_preset_name = theme_mgr.active_preset.name.clone();

        // Filter theme presets
        let mode_filter = self.theme_mode_filter.clone();
        let filtered_presets: Vec<&'static ThemePreset> = all_presets()
            .iter()
            .filter(|p| match mode_filter.as_str() {
                "dark" => p.is_dark(),
                "light" => !p.is_dark(),
                _ => true,
            })
            .collect();

        let is_connected = cx.has_global::<ConnectionState>()
            && ConnectionState::global(cx).status == ConnectionStatus::Connected;
        let is_logged_in = cx.has_global::<AuthState>()
            && AuthState::global(cx).status == AuthStatus::Authenticated;

        let show_theme_picker = self.show_theme_mode_picker;
        let show_tts_picker = self.show_tts_provider_picker;

        let theme_mode_label = match mode_filter.as_str() {
            "dark" => "Dark",
            "light" => "Light",
            _ => "All themes",
        };

        let tts_provider_label = match self.tts_provider.as_str() {
            "fish" => "Fish Audio",
            "edge" => "Edge TTS",
            _ => "Disabled",
        };

        div()
            .id("settings-scroll-area")
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .size_full()
            .overflow_y_scroll()
            .pt(px(40.0))
            .pb(px(60.0))
            .px_4()
            .child(
                div()
                    .w_full()
                    .max_w(px(672.0))
                    .flex()
                    .flex_col()
                    .gap(px(32.0))
                    .pb_12()
                    // Page Title
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child("Customize your application appearance, credentials, and bot configurations"),
                            ),
                    )
                    // Toast message notification
                    .children(self.toast_message.as_ref().map(|(msg, is_err)| {
                        div()
                            .px_4()
                            .py_2p5()
                            .rounded_lg()
                            .border_1()
                            .border_color(if *is_err { rgb(0xef4444).into() } else { primary_color })
                            .bg(if *is_err { rgba(0xef444420) } else { rgba(0x10b98120) })
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if *is_err { rgb(0xef4444).into() } else { text_color })
                                    .child(msg.clone()),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child("✕")
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.toast_message = None;
                                        cx.notify();
                                    })),
                            )
                    }))
                    // ====================================================
                    // 1. APPEARANCE SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("APPEARANCE"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    // Row 1: Theme Mode Filter
                                    .child(
                                        div()
                                            .relative()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Inbox).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Theme Mode"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Filter by dark or light appearance"),
                                                            ),
                                                    ),
                                            )
                                            // Mode Selector Dropdown Button
                                            .child(
                                                div()
                                                    .w(px(110.0))
                                                    .h(px(32.0))
                                                    .px_3()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .justify_between()
                                                    .cursor_pointer()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(text_color)
                                                            .child(theme_mode_label),
                                                    )
                                                    .child(Icon::new(IconName::ChevronDown).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                                        this.show_theme_mode_picker = !this.show_theme_mode_picker;
                                                        this.show_tts_provider_picker = false;
                                                        cx.notify();
                                                    })),
                                            )
                                            // Dropdown Popover
                                            .children(if show_theme_picker {
                                                Some(
                                                    div()
                                                        .absolute()
                                                        .top(px(46.0))
                                                        .right(px(16.0))
                                                        .w(px(110.0))
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(card_bg)
                                                        .shadow_lg()
                                                        .py_1()
                                                        .children(["all", "dark", "light"].into_iter().map(|m| {
                                                            let is_selected = mode_filter == m;
                                                            let label = match m {
                                                                "dark" => "Dark",
                                                                "light" => "Light",
                                                                _ => "All themes",
                                                            };
                                                            div()
                                                                .px_3()
                                                                .py_1p5()
                                                                .text_xs()
                                                                .text_color(if is_selected { primary_color } else { text_color })
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(tag_bg))
                                                                .child(label)
                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                                    this.set_theme_mode_filter(m, window, cx);
                                                                }))
                                                        })),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row 2: Color Theme Cards Grid
                                    .child(
                                        div()
                                            .px_4()
                                            .py_4()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Color Theme"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child(format!("{} themes available", filtered_presets.len())),
                                                    ),
                                            )
                                            // 3 Columns Swatch Grid
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .flex_wrap()
                                                    .gap(px(12.0))
                                                    .children(filtered_presets.into_iter().map(|preset| {
                                                        let is_active = preset.name == active_preset_name;
                                                        let preset_name = preset.name.clone();
                                                        let p_bg = parse_hex_hsla(&preset.colors.background);
                                                        let p_primary = parse_hex_hsla(&preset.colors.primary);
                                                        let p_accent = parse_hex_hsla(&preset.colors.accent);
                                                        let p_destructive = parse_hex_hsla(&preset.colors.destructive);
                                                        let p_fg = parse_hex_hsla(&preset.colors.foreground);
                                                        let p_muted_fg = parse_hex_hsla(&preset.colors.muted_foreground);
                                                        let clean_label = preset.label.clone();

                                                        div()
                                                            .relative()
                                                            .w(px(204.0))
                                                            .p_2()
                                                            .rounded_lg()
                                                            .border_2()
                                                            .border_color(if is_active {
                                                                primary_color
                                                            } else {
                                                                border_color
                                                            })
                                                            .bg(if is_active {
                                                                tag_bg
                                                            } else {
                                                                secondary_bg
                                                            })
                                                            .cursor_pointer()
                                                            .hover(|s| s.border_color(theme.muted_foreground))
                                                            // Mini preview box
                                                            .child(
                                                                div()
                                                                    .h(px(56.0))
                                                                    .w_full()
                                                                    .rounded_md()
                                                                    .p_2()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .justify_between()
                                                                    .bg(p_bg)
                                                                    // 3 colored dots
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_row()
                                                                            .gap_1()
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_primary))
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_accent))
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_destructive)),
                                                                    )
                                                                    // 2 preview bars
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_row()
                                                                            .items_end()
                                                                            .gap_1()
                                                                            .child(div().w(px(110.0)).h(px(4.0)).rounded_sm().bg(p_fg))
                                                                            .child(div().w(px(40.0)).h(px(4.0)).rounded_sm().bg(p_muted_fg)),
                                                                    ),
                                                            )
                                                            // Label
                                                            .child(
                                                                div()
                                                                    .mt_1p5()
                                                                    .px_1()
                                                                    .flex()
                                                                    .flex_row()
                                                                    .items_center()
                                                                    .justify_between()
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(if is_active { text_color } else { muted_text })
                                                                            .child(clean_label),
                                                                    ),
                                                            )
                                                            // Active Checkmark icon
                                                            .children(if is_active {
                                                                Some(
                                                                    div()
                                                                        .absolute()
                                                                        .top(px(8.0))
                                                                        .right(px(8.0))
                                                                        .child(Icon::new(IconName::Check).text_color(primary_color)),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, window, cx| {
                                                                ThemeManager::apply_preset(&preset_name, Some(window), cx);
                                                            }))
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 2. ACCOUNT SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("ACCOUNT"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    // Row 1: Connection & Logout
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .size(px(10.0))
                                                            .rounded_full()
                                                            .bg(if is_connected { rgb(0x10b981) } else { rgb(0xef4444) }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child(if is_logged_in {
                                                                        if is_connected { "Connected" } else { "Reconnecting..." }
                                                                    } else {
                                                                        "Not connected"
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child(if is_logged_in {
                                                                        "WhatsApp bot session is active"
                                                                    } else {
                                                                        "No WhatsApp account linked"
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                if is_logged_in {
                                                    div()
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .bg(rgba(0xef444420))
                                                        .border_1()
                                                        .border_color(rgba(0xef444460))
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(rgba(0xef444440)))
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(rgb(0xef4444))
                                                                .child("Logout"),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, window, cx| {
                                                            open_logout_dialog(window, cx, move |_window, cx| {
                                                                let base_url = if cx.has_global::<AuthState>() {
                                                                    AuthState::global(cx).base_url.clone()
                                                                } else {
                                                                    "http://127.0.0.1:3000/api".to_string()
                                                                };
                                                                cx.spawn(async move |cx| {
                                                                    let client = HttpClient::new(&base_url);
                                                                    let _ = client.logout().await;
                                                                    let _ = cx.update(|cx| {
                                                                        if cx.has_global::<AuthState>() {
                                                                            AuthState::global_mut(cx).set_status(AuthStatus::Unauthenticated);
                                                                        }
                                                                        if cx.has_global::<Router>() {
                                                                            Router::global_mut(cx).navigate(AppRoute::Chat);
                                                                        }
                                                                        cx.refresh_windows();
                                                                    });
                                                                })
                                                                .detach();
                                                            });
                                                        }))
                                                } else {
                                                    div()
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .bg(primary_color)
                                                        .cursor_pointer()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.primary_foreground)
                                                                .child("Login"),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _, cx| {
                                                            if cx.has_global::<AuthState>() {
                                                                AuthState::global_mut(cx).set_status(AuthStatus::Unauthenticated);
                                                                cx.refresh_windows();
                                                            }
                                                        }))
                                                },
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row 2: Chat History Sync
                                    .child(
                                        div()
                                            .px_4()
                                            .py_4()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_start()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_0p5()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Chat history"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Add up to 50 older messages per chat. Existing messages are never removed."),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_3()
                                                            .py_1p5()
                                                            .rounded_md()
                                                            .bg(tag_bg)
                                                            .border_1()
                                                            .border_color(border_color)
                                                            .cursor_pointer()
                                                            .hover(|s| s.bg(secondary_bg))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child(if self.is_syncing_history { "Syncing..." } else { "Sync 50 per chat" }),
                                                            )
                                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                                this.start_history_sync(cx);
                                                            })),
                                                    ),
                                            )
                                            .children(self.history_status.as_ref().map(|st| {
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .gap_4()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child(format!("Status: {}", st.state))
                                                            .child(format!("Staged: {} msgs in {} chats", st.pending_messages, st.pending_chats))
                                                            .child(format!("Added: {}", st.messages_added)),
                                                    )
                                                    .children(if st.state == "running" {
                                                        Some(
                                                            div()
                                                                .w_full()
                                                                .h(px(4.0))
                                                                .rounded_full()
                                                                .bg(tag_bg)
                                                                .overflow_hidden()
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(px(140.0))
                                                                        .bg(primary_color),
                                                                ),
                                                        )
                                                    } else {
                                                        None
                                                    })
                                            })),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 3. AI CONFIGURATION SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("AI CONFIGURATION"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    // Header row
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .child(Icon::new(IconName::Bot).text_color(muted_text))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Gemini + AI Server"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Connect the bot to Gemini and your AI backend"),
                                                    ),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Field 1: Gemini API Key
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Gemini API Key"),
                                                    )
                                                    .child(
                                                        div().text_xs().text_color(muted_text).child(if self.has_gemini_key {
                                                            "•••••••••••••••• (Configured in backend)"
                                                        } else {
                                                            "Not configured (click to set)"
                                                        }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .child("Edit Key")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                        this.open_edit_modal(EditField::GeminiApiKey, window, cx);
                                                    })),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Field 2: AI Server URL
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("AI Server URL"),
                                                    )
                                                    .child(
                                                        div().text_xs().text_color(muted_text).child(self.ai_server_url.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .child("Edit URL")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                        this.open_edit_modal(EditField::AiServerUrl, window, cx);
                                                    })),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Save Button Row
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .child(
                                                div()
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(primary_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.primary_foreground)
                                                            .child(if self.is_saving_ai { "Saving..." } else { "Save AI Config" }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.save_ai_settings(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 4. PRIVACY SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("PRIVACY"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::CircleCheck).text_color(primary_color))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Read receipts"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Send blue ticks to WhatsApp when you open a chat. Off keeps reads local only."),
                                                            ),
                                                    ),
                                            )
                                            // Switch Toggle Button matching web-term
                                            .child(
                                                div()
                                                    .w(px(38.0))
                                                    .h(px(22.0))
                                                    .rounded_full()
                                                    .p_0p5()
                                                    .cursor_pointer()
                                                    .bg(if self.read_receipts {
                                                        primary_color
                                                    } else {
                                                        border_color
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .size(px(18.0))
                                                            .rounded_full()
                                                            .bg(rgb(0xffffff))
                                                            .ml(if self.read_receipts { px(16.0) } else { px(1.0) }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.toggle_read_receipts(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 5. CALL TTS SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("CALL TTS"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    // Header
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .child(Icon::new(IconName::Bell).text_color(muted_text))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Text-to-Speech Provider"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Choose voice synthesis provider for incoming calls"),
                                                    ),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Provider Dropdown Row
                                    .child(
                                        div()
                                            .relative()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Provider"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(120.0))
                                                    .h(px(30.0))
                                                    .px_3()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .justify_between()
                                                    .cursor_pointer()
                                                    .child(
                                                        div().text_xs().text_color(text_color).child(tts_provider_label),
                                                    )
                                                    .child(Icon::new(IconName::ChevronDown).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.show_tts_provider_picker = !this.show_tts_provider_picker;
                                                        this.show_theme_mode_picker = false;
                                                        cx.notify();
                                                    })),
                                            )
                                            .children(if show_tts_picker {
                                                Some(
                                                    div()
                                                        .absolute()
                                                        .top(px(46.0))
                                                        .right(px(16.0))
                                                        .w(px(120.0))
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(border_color)
                                                        .bg(card_bg)
                                                        .shadow_lg()
                                                        .py_1()
                                                        .children([("", "Disabled"), ("edge", "Edge TTS"), ("fish", "Fish Audio")].into_iter().map(|(val, lbl)| {
                                                            let is_selected = self.tts_provider == val;
                                                            div()
                                                                .px_3()
                                                                .py_1p5()
                                                                .text_xs()
                                                                .text_color(if is_selected { primary_color } else { text_color })
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(tag_bg))
                                                                .child(lbl)
                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                    this.tts_provider = val.to_string();
                                                                    this.show_tts_provider_picker = false;
                                                                    cx.notify();
                                                                }))
                                                        })),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Default Voice
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Default Voice"),
                                                    )
                                                    .child(
                                                        div().text_xs().text_color(muted_text).child(self.tts_default_voice.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .child("Edit Voice")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                        this.open_edit_modal(EditField::TtsDefaultVoice, window, cx);
                                                    })),
                                            ),
                                    )
                                    // Fish Audio Fields (shown when fish is selected)
                                    .children(if self.tts_provider == "fish" {
                                        Some(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(div().h(px(1.0)).bg(border_color).w_full())
                                                .child(
                                                    div()
                                                        .px_4()
                                                        .py_3()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(
                                                                    div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Fish Audio Key"),
                                                                )
                                                                .child(
                                                                    div().text_xs().text_color(muted_text).child(if self.has_fish_key {
                                                                        "•••••••••••••••• (Configured)"
                                                                    } else {
                                                                        "Not configured"
                                                                    }),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .px_3()
                                                                .py_1()
                                                                .rounded_md()
                                                                .bg(tag_bg)
                                                                .border_1()
                                                                .border_color(border_color)
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(secondary_bg))
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child("Edit Key")
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                                    this.open_edit_modal(EditField::FishAudioKey, window, cx);
                                                                })),
                                                        ),
                                                )
                                                .child(div().h(px(1.0)).bg(border_color).w_full())
                                                .child(
                                                    div()
                                                        .px_4()
                                                        .py_3()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(
                                                                    div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Fish Audio Model"),
                                                                )
                                                                .child(
                                                                    div().text_xs().text_color(muted_text).child(self.fish_audio_model.clone()),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .px_3()
                                                                .py_1()
                                                                .rounded_md()
                                                                .bg(tag_bg)
                                                                .border_1()
                                                                .border_color(border_color)
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(secondary_bg))
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child("Edit Model")
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                                    this.open_edit_modal(EditField::FishAudioModel, window, cx);
                                                                })),
                                                        ),
                                                ),
                                        )
                                    } else {
                                        None
                                    })
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Save Button
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .child(
                                                div()
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(primary_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.primary_foreground)
                                                            .child(if self.is_saving_tts { "Saving..." } else { "Save TTS Settings" }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.save_tts_settings(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 6. CONNECTION & INFO SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(muted_text)
                                    .child("CONNECTION INFO"),
                            )
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border_color)
                                    .bg(card_bg)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Backend Endpoint"),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted_text).child(
                                                    if cx.has_global::<AuthState>() {
                                                        AuthState::global(cx).base_url.clone()
                                                    } else {
                                                        "http://127.0.0.1:3000/api".to_string()
                                                    },
                                                ),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Platform Framework"),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted_text).child("GPUI (DirectX / DirectWrite Native)"),
                                            ),
                                    ),
                            ),
                    ),
            )
            // ====================================================
            // 7. EDIT MODAL DIALOG
            // ====================================================
            .children(if let Some(modal) = &self.edit_modal {
                let title = modal.title.clone();
                let desc = modal.description.clone();
                let val_display = if modal.is_secret && !modal.value.is_empty() {
                    "●".repeat(modal.value.len())
                } else if modal.value.is_empty() {
                    "Click to type or select preset below...".to_string()
                } else {
                    modal.value.clone()
                };
                let presets = modal.presets.clone();

                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000088))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.edit_modal = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(px(500.0))
                                .rounded_xl()
                                .border_1()
                                .border_color(border_color)
                                .bg(card_bg)
                                .p_6()
                                .shadow_2xl()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .track_focus(&self.modal_focus_handle)
                                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                    if let Some(ref mut m) = this.edit_modal {
                                        match ev.keystroke.key.as_str() {
                                            "backspace" => {
                                                m.value.pop();
                                                cx.notify();
                                            }
                                            "enter" => {
                                                this.save_modal_value(cx);
                                            }
                                            "escape" => {
                                                this.edit_modal = None;
                                                cx.notify();
                                            }
                                            k if k.len() == 1 => {
                                                m.value.push_str(k);
                                                cx.notify();
                                            }
                                            _ => {}
                                        }
                                    }
                                }))
                                .on_mouse_down(MouseButton::Left, |_, _, _| {}) // stop propagation
                                // Modal Header
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(text_color)
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .text_sm()
                                                .text_color(muted_text)
                                                .child("✕")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.edit_modal = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .child(desc),
                                )
                                // Input Display Box
                                .child(
                                    div()
                                        .w_full()
                                        .min_h(px(40.0))
                                        .p_2p5()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(primary_color)
                                        .bg(theme.background)
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if modal.value.is_empty() { muted_text } else { text_color })
                                                .child(val_display),
                                        )
                                        .children(if !modal.value.is_empty() {
                                            Some(
                                                div()
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(muted_text)
                                                    .child("Clear")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(ref mut m) = this.edit_modal {
                                                            m.value.clear();
                                                            cx.notify();
                                                        }
                                                    })),
                                            )
                                        } else {
                                            None
                                        }),
                                )
                                // Preset Chips (if any)
                                .children(if !presets.is_empty() {
                                    Some(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .flex_wrap()
                                            .gap_1p5()
                                            .children(presets.into_iter().map(|(lbl, val)| {
                                                div()
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .child(lbl)
                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                        if let Some(ref mut m) = this.edit_modal {
                                                            m.value = val.to_string();
                                                            cx.notify();
                                                        }
                                                    }))
                                            })),
                                    )
                                } else {
                                    None
                                })
                                // Actions
                                .child(
                                    div()
                                        .mt_2()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_4()
                                                .py_1p5()
                                                .rounded_md()
                                                .bg(tag_bg)
                                                .border_1()
                                                .border_color(border_color)
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(text_color)
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.edit_modal = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .px_4()
                                                .py_1p5()
                                                .rounded_md()
                                                .bg(primary_color)
                                                .cursor_pointer()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.primary_foreground)
                                                .child("Done")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.save_modal_value(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            } else {
                None
            })
    }
}
