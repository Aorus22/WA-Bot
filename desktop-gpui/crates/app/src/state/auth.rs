//! Authentication and session lifecycle state.

use gpui::{App, Global};
use wabot_backend_client::ws::WsEvent;

/// Overall authentication status for gating the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthStatus {
    #[default]
    Checking,
    Unauthenticated,
    Authenticated,
}

/// Active tab on login screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginTab {
    #[default]
    Qr,
    Phone,
}

/// Global authentication state.
#[derive(Debug, Clone)]
pub struct AuthState {
    pub status: AuthStatus,
    pub active_tab: LoginTab,
    pub qr_code: Option<String>,
    pub qr_loading: bool,
    pub qr_error: Option<String>,
    pub base_url: String,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            status: AuthStatus::Unauthenticated,
            active_tab: LoginTab::Qr,
            qr_code: None,
            qr_loading: false,
            qr_error: None,
            base_url: "http://127.0.0.1:3000/api".to_string(),
        }
    }
}

impl Global for AuthState {}

impl AuthState {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn set_status(&mut self, status: AuthStatus) {
        self.status = status;
        if status == AuthStatus::Authenticated {
            self.qr_code = None;
            self.qr_loading = false;
            self.qr_error = None;
        }
    }

    pub fn set_tab(&mut self, tab: LoginTab) {
        self.active_tab = tab;
    }

    pub fn set_qr_code(&mut self, code: Option<String>) {
        self.qr_code = code;
        self.qr_loading = false;
        self.qr_error = None;
    }

    pub fn set_qr_loading(&mut self, loading: bool) {
        self.qr_loading = loading;
    }

    pub fn set_qr_error(&mut self, err: Option<String>) {
        self.qr_error = err;
        self.qr_loading = false;
    }

    /// Process incoming WebSocket event to update auth state.
    pub fn handle_ws_event(&mut self, event: &WsEvent) -> bool {
        match event {
            WsEvent::AuthSuccess => {
                self.set_status(AuthStatus::Authenticated);
                true
            }
            WsEvent::QrCode(code) => {
                if self.status != AuthStatus::Authenticated {
                    self.set_status(AuthStatus::Unauthenticated);
                    self.set_qr_code(Some(code.clone()));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_transitions() {
        let mut state = AuthState::default();
        assert_eq!(state.status, AuthStatus::Unauthenticated);

        // WS QrCode event transitions to Unauthenticated with code
        let changed = state.handle_ws_event(&WsEvent::QrCode("qr-pairing-123".into()));
        assert!(changed);
        assert_eq!(state.status, AuthStatus::Unauthenticated);
        assert_eq!(state.qr_code.as_deref(), Some("qr-pairing-123"));

        // WS AuthSuccess transitions to Authenticated and clears QR
        let changed = state.handle_ws_event(&WsEvent::AuthSuccess);
        assert!(changed);
        assert_eq!(state.status, AuthStatus::Authenticated);
        assert_eq!(state.qr_code, None);
    }
}
