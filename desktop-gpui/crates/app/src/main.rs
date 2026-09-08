//! WA Bot desktop shell (GPUI) — Phase 1 stub; Phase 3 builds the full shell.
//!
//! This stub intentionally touches both `gpui` and `gpui-component` at the call
//! site so a version mismatch fails here, not in a later phase (E0277 guard).

use gpui::*;

fn main() {
    Application::with_platform(gpui_platform::current_platform(false)).run(move |cx: &mut App| {
        // Resolve gpui-component against the same gpui-pre version.
        gpui_component::init(cx);
    });
}
