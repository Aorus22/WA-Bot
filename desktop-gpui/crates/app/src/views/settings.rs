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

use std::collections::HashMap;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use gpui_component::{h_flex, Icon, IconName};
use gpui_component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_component::switch::Switch;
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::{HistorySyncRequest, HistorySyncStart, HistorySyncStatus};
use wabot_backend_client::ClientError;
use super::history_sync::{self, SyncDepth};
use wabot_settings::{
    ChatLayout, DesktopSettings, MAX_WINDOW_TRANSPARENCY, ThemeMode as SettingsThemeMode,
};

use crate::components::connection_banner::{ConnectionState, ConnectionStatus};
use crate::components::{toast, TextField, TextFieldEvent};
use crate::components::dialogs::logout::open_logout_dialog;
use crate::router::{AppRoute, Router};
use crate::state::auth::{AuthState, AuthStatus};
use crate::theme::color::parse_hex_hsla;
use crate::theme::manager::{AppThemeExt, ThemeManager};
use crate::window_material;
use std::borrow::Cow;

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
    window_transparency_slider: Entity<SliderState>,
    _window_transparency_subscription: Subscription,
    settings_scroll_handle: ScrollHandle,

    // Backend settings
    pub settings_loaded: bool,
    pub is_loading_settings: bool,
    pub has_gemini_key: bool,
    pub has_fish_key: bool,
    pub gemini_api_key_input: String,
    pub ai_server_url: String,
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
    pub is_restarting_backend: bool,

    // Asynchronous action states
    pub is_saving_ai: bool,
    pub is_saving_tts: bool,
    pub is_saving_receipts: bool,

    // Modal editing state
    pub edit_modal: Option<EditModalState>,
    pub modal_focus_handle: FocusHandle,
    pub modal_field: Option<TextField>,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let modal_focus_handle = cx.focus_handle();
        let window_transparency = ThemeManager::global(cx).window_transparency;
        let window_transparency_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(MAX_WINDOW_TRANSPARENCY as f32)
                .step(1.0)
                .default_value(window_transparency as f32)
        });
        let transparency_subscription = cx.subscribe(
            &window_transparency_slider,
            |_: &mut Self,
             _: Entity<SliderState>,
             event: &SliderEvent,
             cx: &mut Context<Self>| match event {
                SliderEvent::Change(SliderValue::Single(value)) => {
                    ThemeManager::preview_window_transparency(value.round() as u8, cx);
                }
                SliderEvent::Release(SliderValue::Single(value)) => {
                    ThemeManager::apply_window_transparency(value.round() as u8, cx);
                }
                _ => {}
            },
        );

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
            window_transparency_slider,
            _window_transparency_subscription: transparency_subscription,
            settings_scroll_handle: ScrollHandle::new(),

            settings_loaded: false,
            is_loading_settings: false,
            has_gemini_key: false,
            has_fish_key: false,
            gemini_api_key_input: String::new(),
            ai_server_url: "http://localhost:8981".to_string(),
            read_receipts: true,
            contact_name_mode: "profile".to_string(),

            tts_provider: String::new(),
            show_tts_provider_picker: false,
            tts_default_voice: "en-US-JennyNeural".to_string(),
            fish_audio_key_input: String::new(),
            fish_audio_model: "fishaudio/fish-speech-1.5".to_string(),
            fish_audio_voice_id: String::new(),

            history_status: None,
            is_syncing_history: false,
            history_depth: SyncDepth::PerChat50,
            history_retry_finished: false,
            history_polling: false,
            is_restarting_backend: false,

            is_saving_ai: false,
            is_saving_tts: false,
            is_saving_receipts: false,

            edit_modal: None,
            modal_focus_handle,
            modal_field: None,
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
                                this.contact_name_mode = settings_resp.contact_name_mode().to_string();
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
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(status)) = res {
                                this.is_syncing_history = history_sync::is_active(&status.state);
                                this.history_status = Some(status);
                                this.watch_history_sync(cx);
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Restart the local backend, then pick the session back up: the same port
    /// when it can be reused, a fresh status check, and a chat list reload.
    pub fn restart_backend(&mut self, cx: &mut Context<Self>) {
        if self.is_restarting_backend {
            return;
        }
        self.is_restarting_backend = true;
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let outcome = TOKIO_RT
                    .spawn(async move {
                        let url = crate::backend::restart(&base_url).await?;
                        // The session reconnects on its own; wait for the API to say so.
                        let client = HttpClient::new(&url);
                        let mut status = None;
                        for _ in 0..30 {
                            if let Ok(found) = client.get_status().await {
                                status = Some(found);
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                        Ok::<_, String>((url, status))
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));

                let _ = cx_handle.update(|cx: &mut App| {
                    let message = match &outcome {
                        Ok((url, status)) => {
                            if cx.has_global::<AuthState>() {
                                let auth = AuthState::global_mut(cx);
                                auth.base_url = url.clone();
                                if let Some(status) = status {
                                    auth.set_status(if status.is_logged_in {
                                        AuthStatus::Authenticated
                                    } else {
                                        AuthStatus::Unauthenticated
                                    });
                                }
                            }
                            if cx.has_global::<crate::state::ChatStore>() {
                                crate::state::ChatStore::global_mut(cx).chats_stale = true;
                            }
                            None
                        }
                        Err(error) => Some(format!("Backend restart failed: {error}")),
                    };
                    match (&outcome, message) {
                        (Ok(_), None) => toast::success("Backend restarted", cx),
                        (Ok(_), Some(note)) => toast::info(note, cx),
                        (Err(_), Some(error)) => toast::error(error, cx),
                        (Err(_), None) => {}
                    }
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            this.is_restarting_backend = false;
                            cx.notify();
                        });
                    }
                    cx.refresh_windows();
                });
            }
        })
        .detach();
    }

    /// Follow a running history sync: the status is refetched every couple of
    /// seconds until it ends, so the card shows progress instead of freezing.
    fn watch_history_sync(&mut self, cx: &mut Context<Self>) {
        if self.history_polling || !self.is_syncing_history {
            return;
        }
        self.history_polling = true;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                loop {
                    cx_handle.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                    let Some(base_url) = cx_handle.update(|cx: &mut App| {
                        cx.has_global::<AuthState>().then(|| AuthState::global(cx).base_url.clone())
                    }) else {
                        break;
                    };
                    let res = TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).get_history_sync_status().await })
                        .await;
                    let Ok(Ok(status)) = res else { continue };
                    let running = history_sync::is_active(&status.state);
                    let alive = cx_handle
                        .update(|cx: &mut App| {
                            let Some(view) = this.upgrade() else { return false };
                            view.update(cx, |this, cx| {
                                this.is_syncing_history = running;
                                if !running {
                                    this.history_polling = false;
                                    if cx.has_global::<crate::state::ChatStore>() {
                                        let store = crate::state::ChatStore::global_mut(cx);
                                        store.chats_stale = true;
                                        store.names_changed = true;
                                    }
                                    match status.state.as_str() {
                                        "failed" => toast::error("History sync failed", cx),
                                        "cancelled" => toast::info(format!("History sync stopped: {} messages added", status.messages_added), cx),
                                        _ => toast::success(format!("History sync added {} messages", status.messages_added), cx),
                                    }
                                }
                                this.history_status = Some(status);
                                cx.notify();
                            });
                            true
                        });
                    if !alive || !running {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    /// Start chat history sync via POST /api/history-sync, or with `resume`
    /// continue the stopped or interrupted one where it left off.
    pub fn start_history_sync(&mut self, resume: bool, cx: &mut Context<Self>) {
        if self.is_syncing_history {
            return;
        }
        self.is_syncing_history = true;
        let request = if resume {
            HistorySyncRequest { resume: true, ..Default::default() }
        } else {
            HistorySyncRequest { retry_finished: self.history_retry_finished, ..self.history_depth.request() }
        };

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
                let res = TOKIO_RT.spawn(async move { client.start_history_sync(&request).await }).await;

                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(HistorySyncStart::Started(status))) => {
                                    this.is_syncing_history = history_sync::is_active(&status.state);
                                    this.history_status = Some(status);
                                    this.watch_history_sync(cx);
                                    toast::success(if resume { "History sync resumed" } else { "History sync started" }, cx);
                                }
                                Ok(Ok(HistorySyncStart::AlreadyRunning(status))) => {
                                    // Follow the one that is running instead of failing.
                                    this.is_syncing_history = true;
                                    this.history_status = Some(status);
                                    this.watch_history_sync(cx);
                                    toast::info("A sync is already running. Stop it first to start a different depth.", cx);
                                }
                                other => {
                                    this.is_syncing_history = false;
                                    // Say what the backend said, not just that it failed.
                                    let reason = match other {
                                        Ok(Err(ClientError::Api { message, .. })) => message,
                                        Ok(Err(e)) => e.to_string(),
                                        Err(e) => e.to_string(),
                                        Ok(Ok(_)) => String::new(),
                                    };
                                    let what = if resume { "resume" } else { "start" };
                                    toast::error(
                                        if reason.is_empty() {
                                            format!("Failed to {what} history sync")
                                        } else {
                                            format!("Failed to {what} history sync: {reason}")
                                        },
                                        cx,
                                    );
                                    // Whatever went wrong, show what the backend says it is doing.
                                    this.load_history_status(cx);
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

    /// Stop the running history sync via POST /api/history-sync/cancel.
    pub fn cancel_history_sync(&mut self, cx: &mut Context<Self>) {
        if !self.is_syncing_history {
            return;
        }
        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let view_weak = this;
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).cancel_history_sync().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(status)) => {
                                    this.history_status = Some(status);
                                    this.watch_history_sync(cx);
                                }
                                _ => toast::error("Could not stop the history sync", cx),
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
                                    toast::success("AI configuration saved", cx);
                                }
                                _ => {
                                    toast::error("Failed to save AI configuration", cx);
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
    /// Choose how other people's names are shown ("saved" or "profile").
    pub fn set_contact_name_mode(&mut self, mode: &'static str, cx: &mut Context<Self>) {
        if self.contact_name_mode == mode {
            return;
        }
        let previous = std::mem::replace(&mut self.contact_name_mode, mode.to_string());
        cx.notify();

        let base_url = if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        };
        let mut data = HashMap::new();
        data.insert("contact_name_mode".to_string(), mode.to_string());

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let client = HttpClient::new(&base_url);
                let res = TOKIO_RT.spawn(async move { client.update_settings(&data).await }).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(resp)) => this.contact_name_mode = resp.contact_name_mode().to_string(),
                                _ => {
                                    this.contact_name_mode = previous;
                                    toast::error("Failed to update contact names", cx);
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
                                    toast::success(msg, cx);
                                }
                                _ => {
                                    toast::error("Failed to update read receipts", cx);
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
                                    toast::success("Call TTS settings saved", cx);
                                }
                                _ => {
                                    toast::error("Failed to save TTS settings", cx);
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
            value: initial_val.clone(),
            is_secret,
            presets,
        });

        let on_event = |this: &mut Self, event: TextFieldEvent, _: &mut Window, cx: &mut Context<Self>| match event {
            TextFieldEvent::Change(value) => {
                if let Some(modal) = this.edit_modal.as_mut() {
                    modal.value = value;
                }
                cx.notify();
            }
            TextFieldEvent::Enter => this.save_modal_value(cx),
        };
        let placeholder = "Click to type or select preset below...";
        let input = if is_secret {
            TextField::secret(placeholder, window, cx, on_event)
        } else {
            TextField::new(placeholder, window, cx, on_event)
        };
        input.set_value(initial_val, window, cx);
        input.focus(window, cx);
        self.modal_field = Some(input);
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

impl SettingsView {
    #[allow(clippy::too_many_arguments)]
    fn render_history_card(
        &mut self,
        cx: &mut Context<Self>,
        text_color: Hsla,
        muted_text: Hsla,
        primary_color: Hsla,
        secondary_bg: Hsla,
        tag_bg: Hsla,
        border_color: Hsla,
    ) -> Div {
        let state = self.history_status.as_ref().map(|st| st.state.clone()).unwrap_or_default();
        let active = history_sync::is_active(&state) || self.is_syncing_history;
        let stopping = state == "cancelling";
        let resumable = !active && self.history_status.as_ref().is_some_and(|st| st.resumable);
        let depth = self.history_depth;
        let retry_finished = self.history_retry_finished;

        // The depth choices.
        let chips = SyncDepth::ALL.into_iter().map(|choice| {
            let selected = choice == depth;
            div()
                .px_2p5()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(if selected { primary_color } else { border_color })
                .bg(if selected { tag_bg } else { gpui::transparent_black() })
                .text_xs()
                .text_color(if selected { primary_color } else { text_color })
                .when(!active, |d| d.cursor_pointer().hover(|s| s.bg(secondary_bg)))
                .child(choice.label())
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    if !this.is_syncing_history {
                        this.history_depth = choice;
                        cx.notify();
                    }
                }))
        });

        // Start, or stop while one runs; a stopped one can be resumed.
        let button = |label: &'static str, primary: bool| {
            div()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if primary { primary_color } else { tag_bg })
                .border_1()
                .border_color(if primary { primary_color } else { border_color })
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if primary { rgb(0xffffff).into() } else { text_color })
                        .child(label),
                )
        };
        let action = if active {
            div()
                .px_3()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(rgba(0xef444460))
                .when(!stopping, |d| d.cursor_pointer().hover(|s| s.bg(rgba(0xef444440))))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xef4444))
                        .child(if stopping { "Stopping…" } else { "Stop sync" }),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.cancel_history_sync(cx);
                }))
        } else if resumable {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(button("Start over", false).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.start_history_sync(false, cx);
                })))
                .child(button("Resume", true).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.start_history_sync(true, cx);
                })))
        } else {
            div().child(button("Sync history", false).on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.start_history_sync(false, cx);
            })))
        };

        let mut card = div()
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
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
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
                                    .child("Load older messages from your phone. Existing messages are never removed."),
                            ),
                    )
                    .child(div().flex_none().child(action)),
            )
            .child(div().flex().flex_row().flex_wrap().gap_1p5().children(chips))
            .child(div().text_xs().text_color(muted_text).child(depth.hint()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_2()
                    .when(!active, |d| d.cursor_pointer())
                    .child(
                        div()
                            .mt_0p5()
                            .size(px(14.0))
                            .flex_none()
                            .rounded_sm()
                            .border_1()
                            .border_color(if retry_finished { primary_color } else { border_color })
                            .bg(if retry_finished { primary_color } else { gpui::transparent_black() })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(retry_finished, |d| {
                                d.child(div().text_color(rgb(0xffffff)).text_size(px(10.0)).child("✓"))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_color)
                                    .child("Also retry chats that are already synced"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .child(history_sync::finished_chats_text(self.history_status.as_ref())),
                            ),
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        if !this.is_syncing_history {
                            this.history_retry_finished = !this.history_retry_finished;
                            cx.notify();
                        }
                    })),
            );

        let Some(st) = self.history_status.clone() else {
            return card;
        };
        let ran = st.started_at.is_some();
        if !active && !ran {
            return card;
        }

        // Headline: state and chats done; then a bar with the chats being worked
        // on drawn after the finished ones; then how long it ran and how long is left.
        let fraction = history_sync::progress_fraction(&st);
        let working = history_sync::in_progress_fraction(&st);
        let mut headline = vec![history_sync::state_label(&st.state).to_string()];
        headline.extend(history_sync::chats_text(&st));
        let timing: Vec<String> = history_sync::elapsed_text(&st)
            .into_iter()
            .chain(history_sync::eta_text(&st))
            .collect();

        let mut detail = div().flex().flex_col().gap_1p5().child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_color)
                .child(headline.join(" · ")),
        );
        if active || st.chats_total > 0 {
            detail = detail.child(
                div()
                    .w_full()
                    .h(px(6.0))
                    .flex()
                    .flex_row()
                    .rounded_full()
                    .bg(tag_bg)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(relative(if active && fraction < 0.01 { 0.01 } else { fraction }))
                            .bg(primary_color),
                    )
                    .child(div().h_full().w(relative(working)).bg(primary_color.opacity(0.4))),
            );
        }
        if !timing.is_empty() {
            detail = detail.child(div().text_xs().text_color(muted_text).child(timing.join(" · ")));
        }
        if active {
            for current in st.current.iter().take(3) {
                detail = detail.child(
                    div()
                        .text_xs()
                        .text_color(muted_text)
                        .child(format!("Now: {}", history_sync::current_text(current))),
                );
            }
            if st.current.is_empty() && st.chats_total > 0 {
                detail = detail.child(div().text_xs().text_color(muted_text).child("Now: picking the next chat…"));
            }
        }
        if let Some(note) = history_sync::paused_text(&st) {
            detail = detail.child(div().text_xs().text_color(rgb(0xd97706)).child(note));
        }
        detail = detail.child(div().text_xs().text_color(muted_text).child(history_sync::result_text(&st)));
        if let Some(note) = history_sync::staged_text(&st) {
            detail = detail.child(div().text_xs().text_color(muted_text).child(note));
        }
        if let Some(error) = history_sync::error_text(&st) {
            detail = detail.child(div().text_xs().text_color(rgb(0xef4444)).child(error));
        }
        card = card.child(detail);
        card
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.app_theme();
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let card_bg = theme.card;
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let primary_color = theme.primary;
        let secondary_bg = theme.secondary;
        let tag_bg = theme.muted;

        // Chat history card: depth choice, start/stop, and what the sync is doing.
        let history_card = self.render_history_card(
            cx, text_color, muted_text, primary_color, secondary_bg, tag_bg, border_color,
        );

        let layout_pref_now = crate::layout_pref::preference(cx);
        let notifications_now = crate::notifications::enabled(cx);
        let format_now = crate::text_format::enabled(cx);
        let theme_mgr = ThemeManager::global(cx);
        let active_preset_name = theme_mgr.active_preset.name.clone();
        let window_transparency = theme_mgr.window_transparency;
        let window_blur_enabled = theme_mgr.window_blur_enabled;

        // Filter theme presets
        let mode_filter = self.theme_mode_filter.clone();
        // "Desktop (GTK)" comes first and ignores the dark/light filter: it
        // follows whatever the desktop is currently using.
        let mut filtered_presets: Vec<Cow<'static, ThemePreset>> = theme_mgr
            .gtk_preset
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
        filtered_presets.extend(
            all_presets()
                .iter()
                .filter(|p| match mode_filter.as_str() {
                    "dark" => p.is_dark(),
                    "light" => !p.is_dark(),
                    _ => true,
                })
                .map(Cow::Borrowed),
        );

        let is_connected = cx.has_global::<ConnectionState>()
            && ConnectionState::global(cx).status == ConnectionStatus::Connected;
        let is_logged_in = cx.has_global::<AuthState>()
            && AuthState::global(cx).status == AuthStatus::Authenticated;
        let linked_phone = if cx.has_global::<AuthState>() {
            AuthState::global(cx).phone.clone()
        } else {
            None
        };

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
            .id("settings-root")
            .relative()
            .size_full()
            .bg(window_material::glass_fill(theme.background, window_transparency))
            .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
            .child(
                div()
            .id("settings-scroll-area")
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.settings_scroll_handle)
            .bg(window_material::glass_fill(theme.background, window_transparency))
            .when(framed, |s| s.rounded_br(px(window_material::WINDOW_CORNER_RADIUS)))
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
                                                        .occlude()
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
                                                        }))
                                                    .with_animation(
                                                        "settings-dropdown-in-0",
                                                        crate::components::motion::enter(crate::components::motion::FAST_MS),
                                                        |el, d| el.opacity(d),
                                                    ),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Window transparency slider.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_3()
                                                            .child(Icon::new(IconName::Eye).text_color(muted_text))
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(text_color)
                                                                            .child("Window transparency"),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(muted_text)
                                                                            .child("Adjust how much of the desktop shows through"),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .w(px(280.0))
                                                            .items_center()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .w(px(42.0))
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .text_align(TextAlign::Right)
                                                                    .child(format!("{window_transparency}%")),
                                                            )
                                                            .child(
                                                                Slider::new(&self.window_transparency_slider)
                                                                    .flex_1(),
                                                            ),
                                                    ),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Blur uses the compositor's default strength.
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
                                                    .child(Icon::new(IconName::Eye).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Backdrop blur"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Blur strength is controlled by COSMIC"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("window-backdrop-blur")
                                                    .checked(window_blur_enabled)
                                                    .on_change(|enabled, _, cx| {
                                                        ThemeManager::apply_window_blur_enabled(*enabled, cx);
                                                    }),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row: Chat layout (pages vs sidebar)
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
                                                    .child(Icon::new(IconName::PanelLeft).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Chat Layout"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Auto: pages in the popover, sidebar in a window"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .gap_1()
                                                    .p_0p5()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .children([
                                                        (ChatLayout::Auto, "Auto"),
                                                        (ChatLayout::Pages, "Detail"),
                                                        (ChatLayout::Sidebar, "Sidebar"),
                                                    ].into_iter().map(|(value, label)| {
                                                        let active = layout_pref_now == value
                                                            || (value == ChatLayout::Auto && layout_pref_now == ChatLayout::Unknown);
                                                        div()
                                                            .id(label)
                                                            .cursor_pointer()
                                                            .px_3()
                                                            .py_1()
                                                            .rounded_md()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .bg(if active { primary_color } else { rgba(0x00000000).into() })
                                                            .text_color(if active { theme.primary_foreground } else { text_color })
                                                            .child(label)
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                                                                crate::layout_pref::set(cx, value);
                                                            }))
                                                    })),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row: OS notifications for incoming messages
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
                                                                    .child("Notifications"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Show a desktop notification for new messages while the app is not focused"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("os-notifications")
                                                    .checked(notifications_now)
                                                    .on_change(|enabled, _, cx| {
                                                        crate::notifications::set_enabled(cx, *enabled);
                                                    }),
                                            ),
                                    )
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row: WhatsApp text formatting in message bubbles
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
                                                    .child(Icon::new(IconName::FileText).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Format message text"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Show *bold*, _italic_, ~strikethrough~ and `monospace` like WhatsApp"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("format-messages")
                                                    .checked(format_now)
                                                    .on_change(|enabled, _, cx| {
                                                        crate::text_format::set_enabled(cx, *enabled);
                                                    }),
                                            ),
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
                                                                        match linked_phone.as_deref() {
                                                                            Some(phone) => format!("Linked account: +{phone}"),
                                                                            None => "WhatsApp bot session is active".to_string(),
                                                                        }
                                                                    } else {
                                                                        "No WhatsApp account linked".to_string()
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
                                    .child(history_card)
                                    .child(div().h(px(1.0)).bg(border_color).w_full())
                                    // Row 3: restart the local backend
                                    .child(
                                        div()
                                            .px_4()
                                            .py_4()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Backend"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Restart the local WhatsApp backend if messages stop arriving or it stops responding. Your session is kept."),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id("restart-backend")
                                                    .flex_none()
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .when(!self.is_restarting_backend, |d| {
                                                        d.cursor_pointer().hover(|s| s.bg(secondary_bg))
                                                    })
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child(if self.is_restarting_backend { "Restarting..." } else { "Restart backend" }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.restart_backend(cx);
                                                    })),
                                            ),
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
                    // CONTACT NAMES SECTION
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
                                    .child("CONTACT NAMES"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .children([
                                        ("saved", "Saved contact name", "The name from your phone's contacts. Contacts you haven't saved show their phone number, like WhatsApp."),
                                        ("profile", "Profile name", "The name each person chose for themselves."),
                                    ].into_iter().map(|(mode, title, description)| {
                                        let selected = self.contact_name_mode == mode;
                                        div()
                                            .id(SharedString::from(format!("contact-names-{mode}")))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(if selected { primary_color } else { border_color })
                                            .cursor_pointer()
                                            .hover(|s| s.bg(tag_bg))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(18.0))
                                                    // A long description must not squeeze the circle into an oval.
                                                    .flex_none()
                                                    .rounded_full()
                                                    .border_2()
                                                    .border_color(if selected { primary_color } else { muted_text })
                                                    .children(selected.then(|| div().size(px(8.0)).rounded_full().bg(primary_color))),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(text_color).child(title))
                                                    .child(div().text_xs().text_color(muted_text).child(description)),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_contact_name_mode(mode, cx);
                                            }))
                                    })),
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
                                                        .occlude()
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
                                                        }))
                                                    .with_animation(
                                                        "settings-dropdown-in-1",
                                                        crate::components::motion::enter(crate::components::motion::FAST_MS),
                                                        |el, d| el.opacity(d),
                                                    ),
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
            )
            .child(
                Scrollbar::vertical(&self.settings_scroll_handle)
                    .mode(ScrollbarMode::Always)
                    .styles(|s| {
                        s.track(|t| t.bg(rgba(0x00000000).into()))
                            .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                            .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                            .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                    }),
            )
            // ====================================================
            // 7. EDIT MODAL DIALOG
            // ====================================================
            .children(if let Some(modal) = &self.edit_modal {
                let title = modal.title.clone();
                let desc = modal.description.clone();
                let presets = modal.presets.clone();

                Some(crate::components::motion::modal_in(
                    div()
                        .absolute()
                        .occlude()
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
                                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                    if ev.keystroke.key.as_str() == "escape" {
                                        this.edit_modal = None;
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()) // keep clicks inside the card from closing the modal
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
                                // Value input
                                .children(self.modal_field.as_ref().map(|field| field.element(&theme, None)))
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
                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                        if let Some(field) = this.modal_field.clone() {
                                                            field.set_value(val, window, cx);
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
                        "settings-edit-in",
                    ))
            } else {
                None
            })
    }
}
