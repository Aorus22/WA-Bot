pub(crate) const MESSAGE_IMAGE_MIN_WIDTH: f32 = 240.0;
pub(crate) const MESSAGE_IMAGE_MAX_WIDTH: f32 = 340.0;
pub(crate) const MESSAGE_IMAGE_MIN_HEIGHT: f32 = 180.0;
pub(crate) const MESSAGE_IMAGE_MAX_HEIGHT: f32 = 360.0;

/// drags within the web client's bounds (`ChatPage.tsx` clamps 280–600).
pub(crate) const SIDEBAR_DEFAULT_WIDTH: f32 = 360.0;
pub(crate) const SIDEBAR_MIN_WIDTH: f32 = 280.0;
pub(crate) const SIDEBAR_MAX_WIDTH: f32 = 600.0;
/// Width of the divider's grab area, straddling the sidebar's right edge.
pub(crate) const SIDEBAR_HANDLE_WIDTH: f32 = 6.0;
/// Unread count badge: square minimum + fixed height keeps a single digit a
/// circle while two digits grow into a pill, like WhatsApp.
pub(crate) const UNREAD_BADGE_SIZE: f32 = 20.0;
/// Width of the right-hand drawers (search + chat info); the entrance
/// animation slides across exactly this distance.
pub(crate) const SHEET_WIDTH: f32 = 320.0;
/// Entrance duration for the right-hand drawers, in ms.
pub(crate) const SHEET_ANIM_MS: u64 = 240;
/// A live message plays its entrance for this long, then counts as settled.
pub(crate) const FRESH_MSG_TTL: std::time::Duration = std::time::Duration::from_millis(600);
/// Entrance duration for a newly arrived message bubble, in ms.
pub(crate) const MSG_ENTER_MS: u64 = 220;


/// Deterministic color palette for contact avatars
pub(crate) const AVATAR_COLORS: &[u32] = &[
    0x00a884, // WhatsApp Emerald
    0x0284c7, // Sky
    0x6366f1, // Indigo
    0x8b5cf6, // Violet
    0xec4899, // Pink
    0xf59e0b, // Amber
    0x10b981, // Green
    0x14b8a6, // Teal
];
