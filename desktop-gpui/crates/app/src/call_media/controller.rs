//! Glue between the call state and the media session.
//!
//! [`init`] registers a [`CallMediaState`] global and watches [`CallManager`]:
//! when a call becomes connected a [`Session`] is started, when it ends (or is
//! replaced) the session is dropped, and the mute/video toggles are mirrored
//! into the running session. Pictures and errors flow back through a channel
//! and land in the global for the overlay to draw.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{App, BorrowAppContext, Global, RenderImage};

use super::session::Session;
use super::ui_link::{Source, UiEvent, Which};
use crate::state::auth::AuthState;
use crate::state::call::CallManager;

/// A remote picture older than this is considered stale (the peer turned the
/// camera off or the stream stalled) and the overlay falls back to the avatar.
pub const REMOTE_FRESH: Duration = Duration::from_secs(3);

pub struct CallMediaState {
    session: Option<Session>,
    next_session: u64,
    tx: flume::Sender<UiEvent>,
    remote: Option<(Arc<RenderImage>, Instant)>,
    local: Option<Arc<RenderImage>>,
    errors: BTreeMap<Source, String>,
}

impl Global for CallMediaState {}

impl CallMediaState {
    fn new(tx: flume::Sender<UiEvent>) -> Self {
        Self {
            session: None,
            next_session: 1,
            tx,
            remote: None,
            local: None,
            errors: BTreeMap::new(),
        }
    }

    pub fn global(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    /// True while a media session exists for the current call.
    pub fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// The other party's latest picture, while it is still arriving.
    pub fn remote_image(&self) -> Option<Arc<RenderImage>> {
        self.remote
            .as_ref()
            .filter(|(_, at)| at.elapsed() <= REMOTE_FRESH)
            .map(|(image, _)| Arc::clone(image))
    }

    /// The local camera preview.
    pub fn local_image(&self) -> Option<Arc<RenderImage>> {
        self.local.clone()
    }

    /// All current media problems, one per line, for the overlay.
    pub fn error_text(&self) -> Option<String> {
        if self.errors.is_empty() {
            None
        } else {
            Some(self.errors.values().cloned().collect::<Vec<_>>().join("\n"))
        }
    }

    pub fn error_for(&self, source: Source) -> Option<&str> {
        self.errors.get(&source).map(String::as_str)
    }
}

/// Install the global and start following the call state. Call once, after the
/// [`CallManager`] global exists.
pub fn init(cx: &mut App) {
    let (tx, rx) = flume::unbounded::<UiEvent>();
    cx.set_global(CallMediaState::new(tx));

    cx.observe_global::<CallManager>(sync).detach();

    // Release the camera and devices on quit.
    cx.on_app_quit(|cx| {
        stop_session(cx);
        async {}
    })
    .detach();

    cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv_async().await {
            let _ = cx.update(|cx| apply(cx, event));
        }
    })
    .detach();

    sync(cx);
}

/// Bring the session in line with the call state. Cheap and idempotent.
fn sync(cx: &mut App) {
    if !cx.has_global::<CallMediaState>() || !cx.has_global::<CallManager>() {
        return;
    }
    let (want, mic_muted, speaker_muted) = {
        let mgr = CallManager::global(cx);
        let want = mgr
            .active_call
            .as_ref()
            .filter(|_| mgr.is_connected())
            .map(|call| (call.id.clone(), call.video_enabled));
        (want, mgr.mic_muted, mgr.speaker_muted)
    };
    let current = cx
        .global::<CallMediaState>()
        .session
        .as_ref()
        .map(|s| s.call_id.clone());

    let same = matches!((&want, &current), (Some((id, _)), Some(cur)) if id == cur);
    if !same {
        if current.is_some() {
            stop_session(cx);
        }
        if let Some((call_id, _)) = &want {
            start_session(cx, call_id.clone());
        }
    }

    if let (Some((_, video)), Some(session)) = (&want, &cx.global::<CallMediaState>().session) {
        session.set_mic_muted(mic_muted);
        session.set_speaker_muted(speaker_muted);
        session.set_video(*video);
    }
}

fn start_session(cx: &mut App, call_id: String) {
    let base_url = if cx.has_global::<AuthState>() {
        AuthState::global(cx).base_url.clone()
    } else {
        "http://127.0.0.1:3000/api".to_string()
    };
    cx.update_global::<CallMediaState, _>(|state, _| {
        let id = state.next_session;
        state.next_session += 1;
        state.errors.clear();
        state.session = Some(Session::start(id, call_id, &base_url, state.tx.clone()));
    });
}

/// Drop the session and release every picture it produced.
fn stop_session(cx: &mut App) {
    if !cx.has_global::<CallMediaState>() {
        return;
    }
    let (session, remote, local) = cx.update_global::<CallMediaState, _>(|state, _| {
        state.errors.clear();
        (
            state.session.take(),
            state.remote.take().map(|(image, _)| image),
            state.local.take(),
        )
    });
    // Stops the threads, the socket and ffmpeg.
    drop(session);
    for image in [remote, local].into_iter().flatten() {
        cx.drop_image(image, None);
    }
}

/// Fold one event from the media threads into the global.
fn apply(cx: &mut App, event: UiEvent) {
    let Some(state) = cx.try_global::<CallMediaState>() else { return };
    let current = state.session.as_ref().map(|s| s.id);
    let event_session = match &event {
        UiEvent::Notice { session, .. }
        | UiEvent::Frame { session, .. }
        | UiEvent::LocalCleared { session } => *session,
    };
    if current != Some(event_session) {
        return; // a previous call's leftovers
    }

    match event {
        UiEvent::Notice { source, message, .. } => {
            if state.errors.get(&source) == message.as_ref() {
                return;
            }
            cx.update_global::<CallMediaState, _>(|state, _| match message {
                Some(message) => {
                    state.errors.insert(source, message);
                }
                None => {
                    state.errors.remove(&source);
                }
            });
        }
        UiEvent::Frame { which, .. } => {
            let Some(image) = state.session.as_ref().and_then(|s| s.slots().take(which)) else {
                return;
            };
            let old = cx.update_global::<CallMediaState, _>(|state, _| match which {
                Which::Remote => state
                    .remote
                    .replace((image, Instant::now()))
                    .map(|(old, _)| old),
                Which::Local => state.local.replace(image),
            });
            if let Some(old) = old {
                cx.drop_image(old, None);
            }
        }
        UiEvent::LocalCleared { .. } => {
            let old = cx.update_global::<CallMediaState, _>(|state, _| state.local.take());
            if let Some(old) = old {
                cx.drop_image(old, None);
            }
        }
    }
}
