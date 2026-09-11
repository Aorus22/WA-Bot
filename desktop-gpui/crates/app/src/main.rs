//! WA Bot desktop shell (GPUI) - Main Entry Point.

use std::borrow::Cow;
use std::sync::Arc;
use gpui::{
    point, px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use gpui_reqwest_client::ReqwestClient;
use wabot_app::{
    components::{connection_banner::ConnectionState, toast::ToastBus},
    router::Router,
    state::auth::{AuthState, AuthStatus},
    state::call::CallManager,
    state::chat::ChatStore,
    theme::manager::ThemeManager,
    views::root::RootGateView,
    TOKIO_RT,
};
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::ws::WsClient;
use wabot_settings::DesktopSettings;

fn main() {
    let _tokio_guard = TOKIO_RT.enter();

    let http_client = Arc::new(ReqwestClient::user_agent("wabot/1.0").expect("Failed to create HTTP client"));

    Application::with_platform(gpui_platform::current_platform(false))
        .with_http_client(http_client)
        .with_assets(gpui_kit_assets::Assets)
        .run(move |cx: &mut App| {
            // Register bundled monospace font asset
            let font_bytes: &'static [u8] =
                include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
            let _ = cx.text_system().add_fonts(vec![Cow::Borrowed(font_bytes)]);

            // Initialize gpui-component subsystem
            gpui_component::init(cx);

            // Load persisted settings or fallback defaults
            let settings = DesktopSettings::load().unwrap_or_default();

            // Initialize and register global ThemeManager
            let theme_manager = ThemeManager::init_from_settings(&settings);
            let is_dark = theme_manager.is_dark();
            cx.set_global(theme_manager);

            let gpui_mode = if is_dark {
                gpui_component::ThemeMode::Dark
            } else {
                gpui_component::ThemeMode::Light
            };
            gpui_component::Theme::change(gpui_mode, None, cx);

            // Toasts appear top-center like the web app's
            // `<Toaster position="top-center" />` (sonner).
            gpui_component::Theme::global_mut(cx).notification.placement =
                gpui::Anchor::TopCenter;

            // Global toast bus so any view/task can raise toasts with just `&mut App`.
            cx.set_global(ToastBus::default());

            // Initialize globals
            cx.set_global(Router::default());
            cx.set_global(ConnectionState::default());
            cx.set_global(ChatStore::new());
            cx.set_global(CallManager::new());

            let mut auth_state = AuthState::default();
            let base_url = detect_backend_url(&settings);
            eprintln!("[GPUI] Connecting to backend at: {base_url}");
            auth_state.base_url = base_url.clone();
            cx.set_global(auth_state);

            // Configure window bounds
            let window_bounds = settings
                .window_state
                .and_then(|w| w.normalized())
                .map(|w| {
                    let x = w.x.unwrap_or(100) as f32;
                    let y = w.y.unwrap_or(100) as f32;
                    let width = w.width.unwrap_or(1100) as f32;
                    let height = w.height.unwrap_or(720) as f32;
                    WindowBounds::Windowed(Bounds {
                        origin: point(px(x), px(y)),
                        size: size(px(width), px(height)),
                    })
                })
                .unwrap_or_else(|| {
                    WindowBounds::Windowed(Bounds {
                        origin: point(px(100.), px(100.)),
                        size: size(px(1100.), px(720.)),
                    })
                });

            let window_options = WindowOptions {
                window_bounds: Some(window_bounds),
                window_min_size: Some(size(px(800.0), px(540.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WA Bot Desktop".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            };

            // Open application window with Root wrapper and RootGateView
            let window_res = cx.open_window(window_options, |window, cx| {
                #[cfg(target_os = "windows")]
                {
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    if let Ok(handle) = HasWindowHandle::window_handle(window) {
                        if let RawWindowHandle::Win32(h) = handle.as_raw() {
                            let hwnd = h.hwnd.get();
                            extern "system" {
                                fn DwmSetWindowAttribute(
                                    hwnd: isize,
                                    dwAttribute: u32,
                                    pvAttribute: *const std::ffi::c_void,
                                    cbAttribute: u32,
                                ) -> i32;
                                fn ShowWindow(hwnd: isize, nCmdShow: i32) -> i32;
                                fn BringWindowToTop(hwnd: isize) -> i32;
                                fn SetForegroundWindow(hwnd: isize) -> i32;
                            }
                            let dark: i32 = 1;
                            unsafe {
                                DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as _, 4);
                                DwmSetWindowAttribute(hwnd, 19, &dark as *const _ as _, 4);
                                ShowWindow(hwnd, 5);
                                BringWindowToTop(hwnd);
                                SetForegroundWindow(hwnd);
                            }
                        }
                    }
                }

                let gate_view = cx.new(|cx| RootGateView::new(cx));
                cx.new(|cx| gpui_component::Root::new(gate_view, window, cx))
            });

            match window_res.as_ref() {
                Ok(window) => {
                    cx.global_mut::<ToastBus>().set_window((*window).into());
                    eprintln!("[UI] Window opened successfully!");
                }
                Err(e) => {
                    eprintln!("[UI ERROR] Failed to open window: {e:?}");
                    std::process::exit(1);
                }
            }

            // WebSocket client for live updates (QR codes, auth success, messages)
            let ws_url = if base_url.starts_with("https://") {
                format!("{}/ws", base_url.trim_end_matches("/api").replace("https://", "wss://"))
            } else {
                format!("{}/ws", base_url.trim_end_matches("/api").replace("http://", "ws://"))
            };

            let ws_client = WsClient::from_static_url(ws_url, None);
            ws_client.start();
            let ws_rx = ws_client.subscribe();

            cx.spawn(async move |cx| {
                while let Ok(event) = ws_rx.recv_async().await {
                    let _ = cx.update(|cx| {
                        let mut needs_refresh = false;
                        if cx.has_global::<AuthState>() {
                            if AuthState::global_mut(cx).handle_ws_event(&event) {
                                needs_refresh = true;
                            }
                        }
                        if cx.has_global::<ChatStore>() {
                            if ChatStore::global_mut(cx).handle_ws_event(&event) {
                                needs_refresh = true;
                            }
                        }
                        // call.* events drive the full-screen call overlay
                        if cx.has_global::<CallManager>() {
                            if CallManager::global_mut(cx).handle_ws_event(&event) {
                                needs_refresh = true;
                            }
                        }
                        if needs_refresh {
                            cx.refresh_windows();
                        }
                    });
                }
            })
            .detach();

            // Pick up a call that was already running (started by the API or a
            // previous session) so the overlay shows up on launch.
            let active_call_url = base_url.clone();
            cx.spawn(async move |cx| {
                let res = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&active_call_url);
                        client.get_active_call().await
                    })
                    .await;

                let _ = cx.update(|cx| {
                    if let Ok(Ok(Some(call))) = res {
                        if cx.has_global::<CallManager>() {
                            CallManager::global_mut(cx).adopt(call);
                            cx.refresh_windows();
                        }
                    }
                });
            })
            .detach();

            // Spawn initial background check for login status and QR code
            let check_url = base_url.clone();
            cx.spawn(async move |cx| {
                let res = TOKIO_RT
                    .spawn(async move {
                        let client = HttpClient::new(&check_url);
                        let status = client.get_status().await;
                        let qr = if let Ok(ref s) = status {
                            if !s.is_logged_in {
                                client.get_qr_code().await.ok()
                            } else {
                                None
                            }
                        } else {
                            client.get_qr_code().await.ok()
                        };
                        (status, qr)
                    })
                    .await;

                let (status_res, qr_res) = match res {
                    Ok(pair) => pair,
                    _ => (
                        Err(wabot_backend_client::ClientError::Custom("failed".into())),
                        None,
                    ),
                };

                let _ = cx.update(|cx| {
                    if cx.has_global::<AuthState>() {
                        let auth = AuthState::global_mut(cx);
                        match status_res {
                            Ok(status) => {
                                if status.is_logged_in {
                                    auth.set_status(AuthStatus::Authenticated);
                                } else {
                                    auth.set_status(AuthStatus::Unauthenticated);
                                    if let Some(qr) = qr_res {
                                        if !qr.code.is_empty() {
                                            auth.set_qr_code(Some(qr.code));
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                auth.set_status(AuthStatus::Unauthenticated);
                                if let Some(qr) = qr_res {
                                    if !qr.code.is_empty() {
                                        auth.set_qr_code(Some(qr.code));
                                    }
                                }
                            }
                        }
                        cx.refresh_windows();
                    }
                });
            })
            .detach();
        });
}

/// Detects the backend URL using CLI flags, env vars, active port probes, settings, or .env fallback.
fn detect_backend_url(settings: &DesktopSettings) -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--url" || args[i] == "--backend-url") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
        if args[i].starts_with("--url=") {
            return args[i].trim_start_matches("--url=").to_string();
        }
        if (args[i] == "--port" || args[i] == "-p") && i + 1 < args.len() {
            return format!("http://127.0.0.1:{}/api", args[i + 1]);
        }
        if args[i].starts_with("--port=") {
            return format!("http://127.0.0.1:{}/api", args[i].trim_start_matches("--port="));
        }
    }

    if let Ok(url) = std::env::var("BACKEND_URL") {
        if !url.trim().is_empty() {
            return url.trim().to_string();
        }
    }
    if let Ok(port) = std::env::var("BACKEND_PORT") {
        let p = port.trim().trim_start_matches(':');
        if !p.is_empty() {
            return format!("http://127.0.0.1:{p}/api");
        }
    }

    // Probe running loopback listeners: 3090 (.env default), 3000 (compiled default), 8080 (external)
    for &port in &[3090, 3000, 8080] {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(50)).is_ok() {
            return format!("http://127.0.0.1:{port}/api");
        }
    }

    if let Some(ref url) = settings.last_backend_url {
        if !url.trim().is_empty() {
            return url.clone();
        }
    }

    for env_path in &[std::path::Path::new(".env"), std::path::Path::new("../.env"), std::path::Path::new("../../.env")] {
        if let Ok(content) = std::fs::read_to_string(env_path) {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("PORT=") {
                    let p = val.trim().trim_matches('"').trim_matches('\'').trim_start_matches(':');
                    if !p.is_empty() {
                        return format!("http://127.0.0.1:{p}/api");
                    }
                }
            }
        }
    }

    "http://127.0.0.1:3090/api".to_string()
}

