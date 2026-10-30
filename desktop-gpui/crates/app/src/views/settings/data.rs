//! Settings state transitions, persistence and backend flows.

use super::*;
use wabot_backend_client::dto::{BackupRequest, RestoreRequest};

impl SettingsView {
    // Backup persistence and backend work is kept here with the other data flows.

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

        let (initial_ui_scale, initial_chat_scale) = {
            let mgr = ThemeManager::global(cx);
            (mgr.ui_font_scale as f32, mgr.chat_font_scale as f32)
        };
        let scale_slider = |cx: &mut Context<Self>, initial: f32| {
            cx.new(|_| {
                SliderState::new()
                    .min(MIN_FONT_SCALE as f32)
                    .max(MAX_FONT_SCALE as f32)
                    .step(5.0)
                    .default_value(initial)
            })
        };
        let ui_font_scale_slider = scale_slider(cx, initial_ui_scale);
        let ui_font_scale_subscription = cx.subscribe(
            &ui_font_scale_slider,
            |_: &mut Self,
             _: Entity<SliderState>,
             event: &SliderEvent,
             cx: &mut Context<Self>| match event {
                SliderEvent::Change(SliderValue::Single(value)) => {
                    ThemeManager::preview_ui_font_scale(value.round() as u8, cx);
                }
                SliderEvent::Release(SliderValue::Single(value)) => {
                    ThemeManager::apply_ui_font_scale(value.round() as u8, cx);
                }
                _ => {}
            },
        );
        let chat_font_scale_slider = scale_slider(cx, initial_chat_scale);
        let chat_font_scale_subscription = cx.subscribe(
            &chat_font_scale_slider,
            |_: &mut Self,
             _: Entity<SliderState>,
             event: &SliderEvent,
             cx: &mut Context<Self>| match event {
                SliderEvent::Change(SliderValue::Single(value)) => {
                    ThemeManager::preview_chat_font_scale(value.round() as u8, cx);
                }
                SliderEvent::Release(SliderValue::Single(value)) => {
                    ThemeManager::apply_chat_font_scale(value.round() as u8, cx);
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
            show_theme_modal: false,
            show_history_depth_picker: false,
            window_transparency_slider,
            _window_transparency_subscription: transparency_subscription,
            ui_font_scale_slider,
            _ui_font_scale_subscription: ui_font_scale_subscription,
            chat_font_scale_slider,
            _chat_font_scale_subscription: chat_font_scale_subscription,
            settings_scroll_handle: ScrollHandle::new(),

            settings_loaded: false,
            is_loading_settings: false,
            has_gemini_key: false,
            has_fish_key: false,
            gemini_api_key_input: String::new(),
            ai_server_url: "http://localhost:8981".to_string(),
            ai_provider: "gemini".to_string(),
            show_ai_provider_picker: false,
            ai_base_url: String::new(),
            ai_model: String::new(),
            ai_api_key_input: String::new(),
            has_ai_key: false,
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
            media_status: None,
            is_downloading_media: false,
            media_kinds: history_media::default_kinds(),
            media_retry_unavailable: false,
            media_ask_phone: false,
            media_polling: false,
            storage_usage: None,
            is_restarting_backend: false,

            backup_folder: None,
            backup_status: None,
            backups: Vec::new(),
            is_backing_up: false,
            backup_polling: false,
            restore_status: None,
            is_restoring: false,
            restore_polling: false,
            last_was_restore: false,

            update_status: UpdateStatus::Idle,
            update_progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),

            is_saving_ai: false,
            is_saving_tts: false,
            is_saving_receipts: false,

            edit_modal: None,
            modal_focus_handle,
            modal_field: None,
        };

        view.load_backend_settings(cx);
        view.load_history_status(cx);
        view.load_media_status(cx);
        view.load_storage_usage(cx);
        view.load_backup(cx);

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
                                this.has_ai_key = settings_resp.has_ai_key.unwrap_or(false);
                                this.ai_provider = settings_resp.ai_provider().to_string();
                                this.ai_base_url = settings_resp.ai_base_url().to_string();
                                this.ai_model = settings_resp.ai_model().to_string();
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

    pub(crate) fn backend_base_url(cx: &App) -> String {
        if cx.has_global::<AuthState>() {
            AuthState::global(cx).base_url.clone()
        } else {
            "http://127.0.0.1:3000/api".to_string()
        }
    }

    /// Load the sizes of the chat and media databases via GET /api/storage/usage.
    pub fn load_storage_usage(&mut self, cx: &mut Context<Self>) {
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).get_storage_usage().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let (Some(view), Ok(Ok(usage))) = (this.upgrade(), res) {
                        view.update(cx, |this, cx| {
                            this.storage_usage = Some(usage);
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Load the media download status via GET /api/history-media/status.
    pub fn load_media_status(&mut self, cx: &mut Context<Self>) {
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).get_history_media_status().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(status)) = res {
                                this.is_downloading_media = history_media::is_active(&status.state);
                                this.media_status = Some(status);
                                this.watch_history_media(cx);
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Follow a running media download: the status is refetched every second
    /// until it ends.
    fn watch_history_media(&mut self, cx: &mut Context<Self>) {
        if self.media_polling || !self.is_downloading_media {
            return;
        }
        self.media_polling = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let mut polls = 0u32;
                loop {
                    cx_handle.background_executor().timer(std::time::Duration::from_millis(1000)).await;
                    let Some(base_url) = cx_handle.update(|cx: &mut App| {
                        cx.has_global::<AuthState>().then(|| AuthState::global(cx).base_url.clone())
                    }) else {
                        break;
                    };
                    let res = TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).get_history_media_status().await })
                        .await;
                    let Ok(Ok(status)) = res else { continue };
                    let running = history_media::is_active(&status.state);
                    let alive = cx_handle.update(|cx: &mut App| {
                        let Some(view) = this.upgrade() else { return false };
                        view.update(cx, |this, cx| {
                            this.is_downloading_media = running;
                            polls += 1;
                            if !running || polls % 5 == 0 {
                                this.load_storage_usage(cx);
                            }
                            if !running {
                                this.media_polling = false;
                                if cx.has_global::<crate::state::ChatStore>() {
                                    crate::state::ChatStore::global_mut(cx).chats_stale = true;
                                }
                                match status.state.as_str() {
                                    "failed" => toast::error("Media download failed", cx),
                                    "cancelled" => toast::info(format!("Media download stopped: {} files downloaded", status.done), cx),
                                    _ => toast::success(format!("Downloaded {} files", status.done), cx),
                                }
                            }
                            this.media_status = Some(status);
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

    /// Start the media download via POST /api/history-media, or with `resume`
    /// continue the stopped or interrupted one.
    pub fn start_history_media(&mut self, resume: bool, cx: &mut Context<Self>) {
        if self.is_downloading_media {
            return;
        }
        let request = if resume {
            HistoryMediaRequest { resume: true, ..Default::default() }
        } else {
            if self.media_kinds.is_empty() {
                return;
            }
            HistoryMediaRequest {
                types: self.media_kinds.clone(),
                retry_unavailable: self.media_retry_unavailable,
                ask_phone: self.media_ask_phone,
                resume: false,
            }
        };
        self.is_downloading_media = true;
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).start_history_media(&request).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(HistoryMediaStart::Started(status))) => {
                                    this.is_downloading_media = history_media::is_active(&status.state);
                                    this.media_status = Some(status);
                                    this.watch_history_media(cx);
                                    toast::success(if resume { "Media download resumed" } else { "Media download started" }, cx);
                                }
                                Ok(Ok(HistoryMediaStart::AlreadyRunning(status))) => {
                                    this.is_downloading_media = true;
                                    this.media_status = Some(status);
                                    this.watch_history_media(cx);
                                    toast::info("A media download is already running.", cx);
                                }
                                other => {
                                    this.is_downloading_media = false;
                                    let reason = match other {
                                        Ok(Err(ClientError::Api { message, .. })) => message,
                                        Ok(Err(e)) => e.to_string(),
                                        Err(e) => e.to_string(),
                                        Ok(Ok(_)) => String::new(),
                                    };
                                    toast::error(
                                        if reason.is_empty() {
                                            "Failed to start the media download".to_string()
                                        } else {
                                            format!("Failed to start the media download: {reason}")
                                        },
                                        cx,
                                    );
                                    this.load_media_status(cx);
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

    /// Stop the running media download via POST /api/history-media/cancel.
    pub fn cancel_history_media(&mut self, cx: &mut Context<Self>) {
        if !self.is_downloading_media {
            return;
        }
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).cancel_history_media().await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(status)) => {
                                    this.media_status = Some(status);
                                    this.watch_history_media(cx);
                                }
                                _ => toast::error("Could not stop the media download", cx),
                            }
                            cx.notify();
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

    /// Ask GitHub whether a newer stable release exists.
    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update_status, UpdateStatus::Checking | UpdateStatus::Downloading(_)) {
            return;
        }
        self.update_status = UpdateStatus::Checking;
        cx.notify();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let result = TOKIO_RT
                    .spawn_blocking(updater::check)
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |s, cx| {
                            s.update_status = match result {
                                Ok(Some(info)) => UpdateStatus::Available(info),
                                Ok(None) => UpdateStatus::UpToDate,
                                Err(e) => UpdateStatus::Error(e),
                            };
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Download the release, verify it and replace the running AppImage.
    pub fn install_update(&mut self, info: UpdateInfo, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Downloading(info.clone());
        self.update_progress.0.store(0, Ordering::Relaxed);
        self.update_progress.1.store(0, Ordering::Relaxed);
        cx.notify();
        let progress = self.update_progress.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let worker = TOKIO_RT.spawn_blocking(move || {
                    updater::download_and_install(&info, &progress.0, &progress.1)
                });
                // Repaint the percentage while the blocking download runs.
                while !worker.is_finished() {
                    TOKIO_RT.spawn(tokio::time::sleep(std::time::Duration::from_millis(200))).await.ok();
                    let alive = cx_handle.update(|cx: &mut App| match this.upgrade() {
                        Some(view) => {
                            view.update(cx, |_, cx| cx.notify());
                            true
                        }
                        None => false,
                    });
                    if !alive {
                        break;
                    }
                }
                let result = worker.await.unwrap_or_else(|e| Err(e.to_string()));
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |s, cx| {
                            s.update_status = match result {
                                Ok(()) => UpdateStatus::ReadyToRestart,
                                Err(e) => UpdateStatus::Error(e),
                            };
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Relaunch the updated AppImage once this process (and its backend) is gone.
    pub fn restart_app(&mut self, cx: &mut Context<Self>) {
        match updater::spawn_restart() {
            Ok(()) => cx.quit(),
            Err(e) => {
                self.update_status = UpdateStatus::Error(e);
                cx.notify();
            }
        }
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
        data.insert("ai_provider".to_string(), self.ai_provider.clone());
        data.insert("ai_base_url".to_string(), self.ai_base_url.trim().to_string());
        data.insert("ai_model".to_string(), self.ai_model.trim().to_string());
        if !self.ai_api_key_input.trim().is_empty() {
            data.insert("ai_api_key".to_string(), self.ai_api_key_input.trim().to_string());
        }
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
                                    this.has_ai_key = resp.has_ai_key.unwrap_or(this.has_ai_key);
                                    this.gemini_api_key_input.clear();
                                    this.ai_api_key_input.clear();
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
            EditField::AiBaseUrl => (
                "API base URL".to_string(),
                "Where requests go. Leave blank for the provider's own address.".to_string(),
                self.ai_base_url.clone(),
                false,
                if self.ai_provider == "claude" {
                    vec![("Anthropic", "https://api.anthropic.com")]
                } else {
                    vec![
                        ("OpenAI", "https://api.openai.com/v1"),
                        ("OpenRouter", "https://openrouter.ai/api/v1"),
                        ("Ollama (local)", "http://localhost:11434/v1"),
                    ]
                },
            ),
            EditField::AiApiKey => (
                "API key".to_string(),
                "Stored in the backend database. Leave blank to keep the existing key.".to_string(),
                self.ai_api_key_input.clone(),
                true,
                vec![],
            ),
            EditField::AiModel => (
                "Model".to_string(),
                "Model name sent with every request, as the provider spells it.".to_string(),
                self.ai_model.clone(),
                false,
                vec![],
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
                EditField::AiBaseUrl => self.ai_base_url = modal.value,
                EditField::AiApiKey => self.ai_api_key_input = modal.value,
                EditField::AiModel => self.ai_model = modal.value,
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

    /// Backup folder, status and list as the backend knows them.
    pub fn load_backup(&mut self, cx: &mut Context<Self>) {
        self.backup_folder = DesktopSettings::load().ok().and_then(|s| s.backup_folder);
        let base_url = Self::backend_base_url(cx);
        let target = self.backup_folder.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let status = TOKIO_RT
                    .spawn({
                        let base_url = base_url.clone();
                        async move { HttpClient::new(&base_url).get_backup_status().await }
                    })
                    .await;
                let restore = TOKIO_RT
                    .spawn({
                        let base_url = base_url.clone();
                        async move { HttpClient::new(&base_url).get_restore_status().await }
                    })
                    .await;
                let list = match target {
                    Some(target) => TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).list_backups(&target).await })
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => None,
                };
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(status)) = status {
                                this.is_backing_up = status.running;
                                this.backup_status = Some(status);
                                this.watch_backup(cx);
                            }
                            if let Ok(Ok(status)) = restore {
                                if status.running {
                                    this.is_restoring = true;
                                    this.last_was_restore = true;
                                    this.restore_status = Some(status);
                                    this.watch_restore(cx);
                                }
                            }
                            if let Some(list) = list {
                                this.backups = list;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Ask for a folder and remember it.
    pub fn choose_backup_folder(&mut self, cx: &mut Context<Self>) {
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let options = PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Use this folder for backups".into()),
                };
                let receiver = cx_handle.update(|cx: &mut App| cx.prompt_for_paths(options));
                let Ok(Ok(Some(paths))) = receiver.await else { return };
                let Some(folder) = paths.into_iter().next() else { return };
                let folder = folder.to_string_lossy().to_string();
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Ok(mut settings) = DesktopSettings::load() {
                        settings.backup_folder = Some(folder.clone());
                        let _ = settings.save();
                    }
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.backup_folder = Some(folder);
                            this.backups.clear();
                            this.load_backup(cx);
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Start a backup via POST /api/backup/run and follow it.
    pub fn start_backup(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.backup_folder.clone() else { return };
        if self.is_backing_up || self.is_restoring {
            return;
        }
        self.is_backing_up = true;
        self.last_was_restore = false;
        self.backup_status = None;
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let request = BackupRequest { target, keep: backup::KEEP_BACKUPS };
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).start_backup(&request).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(_)) => this.watch_backup(cx),
                                other => {
                                    this.is_backing_up = false;
                                    let reason = match other {
                                        Ok(Err(ClientError::Api { message, .. })) => message,
                                        Ok(Err(e)) => e.to_string(),
                                        Err(e) => e.to_string(),
                                        Ok(Ok(_)) => String::new(),
                                    };
                                    toast::error(format!("Could not start the backup: {reason}"), cx);
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

    /// Merge one backup into the running app via POST /api/backup/restore and
    /// follow it. What is already here is kept; only what is missing is added.
    pub fn start_restore(&mut self, account: String, id: String, cx: &mut Context<Self>) {
        let Some(target) = self.backup_folder.clone() else { return };
        if self.is_backing_up || self.is_restoring {
            return;
        }
        self.is_restoring = true;
        self.last_was_restore = true;
        self.restore_status = None;
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let request = RestoreRequest { target, account, id };
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).start_restore(&request).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(_)) => this.watch_restore(cx),
                                other => {
                                    this.is_restoring = false;
                                    let reason = match other {
                                        Ok(Err(ClientError::Api { message, .. })) => message,
                                        Ok(Err(e)) => e.to_string(),
                                        Err(e) => e.to_string(),
                                        Ok(Ok(_)) => String::new(),
                                    };
                                    toast::error(format!("Could not start the restore: {reason}"), cx);
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

    /// Refetch the restore status every second until the run ends.
    fn watch_restore(&mut self, cx: &mut Context<Self>) {
        if self.restore_polling || !self.is_restoring {
            return;
        }
        self.restore_polling = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                loop {
                    cx_handle.background_executor().timer(std::time::Duration::from_millis(1000)).await;
                    let Some(base_url) = cx_handle.update(|cx: &mut App| {
                        cx.has_global::<AuthState>().then(|| AuthState::global(cx).base_url.clone())
                    }) else {
                        break;
                    };
                    let res = TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).get_restore_status().await })
                        .await;
                    let Ok(Ok(status)) = res else { continue };
                    let running = status.running;
                    let alive = cx_handle.update(|cx: &mut App| {
                        let Some(view) = this.upgrade() else { return false };
                        view.update(cx, |this, cx| {
                            this.is_restoring = running;
                            if !running {
                                this.restore_polling = false;
                                if status.error.is_empty() {
                                    toast::success("Restore finished", cx);
                                } else {
                                    toast::error(format!("Restore failed: {}", status.error), cx);
                                }
                            }
                            this.restore_status = Some(status);
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

    /// Refetch the backup status every second until the run ends.
    fn watch_backup(&mut self, cx: &mut Context<Self>) {
        if self.backup_polling || !self.is_backing_up {
            return;
        }
        self.backup_polling = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                loop {
                    cx_handle.background_executor().timer(std::time::Duration::from_millis(1000)).await;
                    let Some(base_url) = cx_handle.update(|cx: &mut App| {
                        cx.has_global::<AuthState>().then(|| AuthState::global(cx).base_url.clone())
                    }) else {
                        break;
                    };
                    let res = TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).get_backup_status().await })
                        .await;
                    let Ok(Ok(status)) = res else { continue };
                    let running = status.running;
                    let alive = cx_handle.update(|cx: &mut App| {
                        let Some(view) = this.upgrade() else { return false };
                        view.update(cx, |this, cx| {
                            this.is_backing_up = running;
                            if !running {
                                this.backup_polling = false;
                                if status.error.is_empty() {
                                    toast::success("Backup finished", cx);
                                } else {
                                    toast::error(format!("Backup failed: {}", status.error), cx);
                                }
                            }
                            this.backup_status = Some(status);
                            if !running {
                                this.load_backup(cx);
                            }
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


}
