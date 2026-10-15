//! How the media threads talk back to the UI thread.
//!
//! Errors and state changes are small events on an unbounded channel. Video
//! frames are different: they are big and arrive 15+ times a second, so they go
//! through a "latest frame" slot and the channel only carries a wake-up when the
//! slot was empty. A stalled UI therefore never queues up decoded pictures.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::RenderImage;
use parking_lot::Mutex;

/// Which part of the media pipeline an error belongs to. Each source keeps at
/// most one message, which is cleared when that part recovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Network,
    Mic,
    Speaker,
    Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Local,
    Remote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEvent {
    /// Set (`Some`) or clear (`None`) the error shown for a source.
    Notice { session: u64, source: Source, message: Option<String> },
    /// A new picture is waiting in the session's [`FrameSlots`].
    Frame { session: u64, which: Which },
    /// The local camera stopped; drop the preview.
    LocalCleared { session: u64 },
}

#[derive(Default)]
pub struct FrameSlots {
    local: Mutex<Option<Arc<RenderImage>>>,
    remote: Mutex<Option<Arc<RenderImage>>>,
    local_pending: AtomicBool,
    remote_pending: AtomicBool,
}

impl FrameSlots {
    fn parts(&self, which: Which) -> (&Mutex<Option<Arc<RenderImage>>>, &AtomicBool) {
        match which {
            Which::Local => (&self.local, &self.local_pending),
            Which::Remote => (&self.remote, &self.remote_pending),
        }
    }

    /// Take the newest picture (UI side). Re-arms the wake-up.
    pub fn take(&self, which: Which) -> Option<Arc<RenderImage>> {
        let (slot, pending) = self.parts(which);
        pending.store(false, Ordering::SeqCst);
        slot.lock().take()
    }
}

/// Producer handle given to every media thread of a session.
#[derive(Clone)]
pub struct UiLink {
    session: u64,
    tx: flume::Sender<UiEvent>,
    slots: Arc<FrameSlots>,
}

impl UiLink {
    pub fn new(session: u64, tx: flume::Sender<UiEvent>) -> Self {
        Self { session, tx, slots: Arc::new(FrameSlots::default()) }
    }

    pub fn session(&self) -> u64 {
        self.session
    }

    pub fn slots(&self) -> Arc<FrameSlots> {
        Arc::clone(&self.slots)
    }

    pub fn notice(&self, source: Source, message: Option<String>) {
        let _ = self.tx.send(UiEvent::Notice { session: self.session, source, message });
    }

    pub fn error(&self, source: Source, message: impl Into<String>) {
        self.notice(source, Some(message.into()));
    }

    pub fn ok(&self, source: Source) {
        self.notice(source, None);
    }

    pub fn frame(&self, which: Which, image: Arc<RenderImage>) {
        let (slot, pending) = self.slots.parts(which);
        *slot.lock() = Some(image);
        if !pending.swap(true, Ordering::SeqCst) {
            let _ = self.tx.send(UiEvent::Frame { session: self.session, which });
        }
    }

    pub fn clear_local(&self) {
        self.slots.local.lock().take();
        self.slots.local_pending.store(false, Ordering::SeqCst);
        let _ = self.tx.send(UiEvent::LocalCleared { session: self.session });
    }
}
