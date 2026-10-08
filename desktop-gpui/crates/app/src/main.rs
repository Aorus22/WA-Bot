//! WA Bot desktop shell (GPUI) - Main Entry Point.

use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;
use gpui::{
    point, px, size, AnyWindowHandle, App, AppContext, Application, Bounds, Styled, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use gpui_reqwest_client::ReqwestClient;
use wabot_app::{
    components::{connection_banner::ConnectionState, toast::ToastBus},
    popover,
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
    // `--toggle` / `--popover`: resident panel-popover instance (see popover.rs).
    let args: Vec<String> = std::env::args().collect();
    let want_toggle = args.iter().any(|a| a == "--toggle");
    let popover_mode = want_toggle || args.iter().any(|a| a == "--popover");
    if want_toggle && popover::signal_running() {
        return;
    }
    let toggle_rx = if popover_mode {
        match popover::listen() {
            Some(rx) => Some(rx),
            None => {
                eprintln!("[popover] another instance is already running");
                return;
            }
        }
    } else {
        None
    };

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

            // Follow the desktop theme live: poll the GTK config for changes.
            cx.spawn(async move |cx| loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                cx.update(|cx| ThemeManager::sync_gtk(cx));
            })
            .detach();

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

            if popover_mode {
                cx.set_global(popover::PopoverMode);
                // The resident instance outlives its (hidden) windows.
                cx.set_quit_mode(gpui::QuitMode::Explicit);
            }

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

            // Opens the UI window: a normal window, or (popover mode) a
            // layer-shell surface next to the panel button.
            let open_ui = move |cx: &mut App, placement: Option<popover::Placement>| -> Option<AnyWindowHandle> {
                let window_options = if let Some(placement) = placement {
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(0.), px(0.)),
                            size: size(px(popover::POPOVER_WIDTH), px(popover::POPOVER_HEIGHT)),
                        })),
                        titlebar: None,
                        app_id: Some("dev.wabot.popover".to_string()),
                        window_background: WindowBackgroundAppearance::Transparent,
                        kind: WindowKind::LayerShell(popover::card_layer_options(&placement)),
                        ..Default::default()
                    }
                } else {
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        window_min_size: Some(size(px(800.0), px(540.0))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("WA Bot Desktop".into()),
                            appears_transparent: true,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                };

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

                let popover_ui = placement.is_some();
                let gate_view = cx.new(|cx| {
                    let view = RootGateView::new(cx);
                    if popover_ui {
                        // Close when focus moves elsewhere, but only after the
                        // surface has actually been focused once.
                        let was_active = Rc::new(std::cell::Cell::new(false));
                        cx.observe_window_activation(window, move |_, window, cx| {
                            if window.is_window_active() {
                                was_active.set(true);
                            } else if was_active.get() {
                                window.defer(cx, |_, cx| popover::close_all(cx));
                            }
                        })
                        .detach();
                    }
                    view
                });
                if popover_ui {
                    // The popover draws its own rounded frame on a transparent
                    // surface; no CSD border/shadow inset and no root fill.
                    let framed = cx.new(|_| popover::PopoverFrame { child: gate_view.into() });
                    cx.new(|cx| {
                        gpui_component::Root::new(framed, window, cx)
                            .bordered(false)
                            .bg(gpui::transparent_black())
                    })
                } else {
                    cx.new(|cx| gpui_component::Root::new(gate_view, window, cx))
                }
                });

                match window_res {
                    Ok(window) => {
                        cx.global_mut::<ToastBus>().set_window(window.into());
                        eprintln!("[UI] Window opened successfully!");
                        Some(window.into())
                    }
                    Err(e) => {
                        eprintln!("[UI ERROR] Failed to open window: {e:?}");
                        None
                    }
                }
            };

            if let Some(rx) = toggle_rx {
                // Resident popover instance: show/hide on every `--toggle`.
                let open_ui = Rc::new(open_ui);
                let toggle = {
                    let open_ui = open_ui.clone();
                    move |cx: &mut App| {
                        if popover::is_open(cx) {
                            popover::close_all(cx);
                            return;
                        }
                        let open_ui = open_ui.clone();
                        popover::open(
                            cx,
                            Rc::new(move |cx, placement| match open_ui(cx, Some(placement)) {
                                Some(handle) => popover::set_card(cx, handle),
                                None => popover::close_all(cx),
                            }),
                        );
                    }
                };
                if want_toggle {
                    toggle(cx);
                }
                cx.spawn(async move |cx| {
                    while rx.recv_async().await.is_ok() {
                        let _ = cx.update(|cx| toggle(cx));
                    }
                })
                .detach();
            } else if open_ui(cx, None).is_none() {
                std::process::exit(1);
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

/// True when `port` answers `GET /api/status` like the WA Bot backend does
/// (`{"isLoggedIn": ...}`). An open port alone is not enough: other local
/// servers (a Caddy on 8080, say) accept connections and answer with HTML.
fn is_wabot_backend(port: u16) -> bool {
    use std::io::{Read, Write};
    use std::time::Duration;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(100)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    let request = format!("GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = Vec::new();
    let _ = stream.take(8192).read_to_end(&mut response);
    looks_like_wabot_status(&String::from_utf8_lossy(&response))
}

fn looks_like_wabot_status(response: &str) -> bool {
    response.starts_with("HTTP/1.1 200") && response.contains("\"isLoggedIn\"")
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
        if is_wabot_backend(port) {
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


#[cfg(test)]
mod tests {
    use super::looks_like_wabot_status;

    #[test]
    fn accepts_the_backend_status_json() {
        assert!(looks_like_wabot_status(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"isLoggedIn\":false}"
        ));
    }

    #[test]
    fn rejects_other_servers() {
        // Caddy serving an SPA answers 200 with HTML for every path.
        assert!(!looks_like_wabot_status("HTTP/1.1 200 OK\r\nServer: Caddy\r\n\r\n<!doctype html>"));
        assert!(!looks_like_wabot_status("HTTP/1.1 404 Not Found\r\n\r\n{\"isLoggedIn\":false}"));
        assert!(!looks_like_wabot_status(""));
    }
}
