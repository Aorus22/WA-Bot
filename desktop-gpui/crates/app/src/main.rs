//! WA Bot desktop shell (GPUI) - Main Entry Point.

use gpui::{
    point, px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use gpui_component::Root;
use wabot_app::{
    components::connection_banner::ConnectionState,
    router::Router,
    state::auth::{AuthState, AuthStatus},
    theme::manager::ThemeManager,
    views::root::RootGateView,
};
use wabot_backend_client::client::HttpClient;
use wabot_settings::DesktopSettings;

fn main() {
    Application::with_platform(gpui_platform::current_platform(false)).run(move |cx: &mut App| {
        // Initialize gpui-component subsystem
        gpui_component::init(cx);

        // Load persisted settings or fallback defaults
        let settings = DesktopSettings::load().unwrap_or_default();

        // Initialize and register global ThemeManager
        let theme_manager = ThemeManager::init_from_settings(&settings);
        cx.set_global(theme_manager);

        // Initialize globals
        cx.set_global(Router::default());
        cx.set_global(ConnectionState::default());

        let mut auth_state = AuthState::default();
        let base_url = settings
            .last_backend_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:3000/api".to_string());
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
            titlebar: Some(TitlebarOptions {
                title: Some("WA Bot".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.), px(12.))),
            }),
            ..Default::default()
        };

        // Open application window with Root wrapper and RootGateView
        let _window = cx.open_window(window_options, |window, cx| {
            let gate_view = cx.new(|cx| RootGateView::new(cx));
            cx.new(|cx| Root::new(gate_view, window, cx))
        });

        // Spawn initial background check for login status
        cx.spawn(async move |cx| {
            let client = HttpClient::new(&base_url);
            match client.get_status().await {
                Ok(status) => {
                    let _ = cx.update(|cx| {
                        if cx.has_global::<AuthState>() {
                            let auth = AuthState::global_mut(cx);
                            if status.is_logged_in {
                                auth.set_status(AuthStatus::Authenticated);
                            } else {
                                auth.set_status(AuthStatus::Unauthenticated);
                            }
                            cx.refresh_windows();
                        }
                    });
                }
                Err(_) => {
                    let _ = cx.update(|cx| {
                        if cx.has_global::<AuthState>() {
                            AuthState::global_mut(cx).set_status(AuthStatus::Unauthenticated);
                            cx.refresh_windows();
                        }
                    });
                }
            }
        })
        .detach();
    });
}
