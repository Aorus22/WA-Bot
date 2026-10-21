//! "Chat history" and "Chat media" rows of the Settings page.
//!
//! Both follow one layout: title and the main action on top, the options below
//! as label-and-control rows, and a status block that only shows a progress bar
//! while something runs. After a run it shrinks to a one-line summary.

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::switch::Switch;
use gpui_component::Disableable as _;
use gpui_component::{Icon, IconName};

use super::history_media;
use super::history_sync::{self, SyncDepth};
use super::settings::SettingsView;
use super::settings_dropdown::dropdown_list;

/// Colors the two rows share.
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub text: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub secondary: Hsla,
    pub tag: Hsla,
    pub border: Hsla,
    pub card: Hsla,
}

pub(super) fn button(p: Palette, label: &'static str, primary: bool, enabled: bool) -> Div {
    div()
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(if primary { p.primary } else { p.tag })
        .border_1()
        .border_color(if primary { p.primary } else { p.border })
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.opacity(0.85)))
        .when(!enabled, |d| d.opacity(0.5))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(if primary { rgb(0xffffff).into() } else { p.text })
                .child(label),
        )
}

pub(super) fn stop_button(p: Palette, label: &'static str, enabled: bool) -> Div {
    let _ = p;
    div()
        .px_3()
        .py_1p5()
        .rounded_md()
        .border_1()
        .border_color(rgba(0xef444460))
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(rgba(0xef444440))))
        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0xef4444)).child(label))
}

/// Title and description on the left, the actions on the right.
pub(super) fn header(p: Palette, title: &'static str, description: &'static str, extra: Option<String>, action: impl IntoElement) -> Div {
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
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.text).child(title))
                .child(div().text_xs().text_color(p.muted).child(description))
                .children(extra.map(|e| div().text_xs().text_color(p.muted).child(e))),
        )
        .child(div().flex_none().child(action))
}

/// A label with a hint, and a control on the right.
pub(super) fn option_row(p: Palette, label: &'static str, hint: String, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(p.text).child(label))
                .child(div().text_xs().text_color(p.muted).child(hint)),
        )
        .child(div().flex_none().child(control))
}

pub(super) fn progress_bar(p: Palette, segments: &[(f32, Hsla)]) -> Div {
    let mut bar = div().w_full().h(px(6.0)).flex().flex_row().rounded_full().bg(p.tag).overflow_hidden();
    for (share, color) in segments {
        bar = bar.child(div().h_full().w(relative(*share)).bg(*color));
    }
    bar
}

impl SettingsView {
    pub(super) fn render_history_card(&mut self, cx: &mut Context<Self>, p: Palette) -> Div {
        let state = self.history_status.as_ref().map(|st| st.state.clone()).unwrap_or_default();
        let active = history_sync::is_active(&state) || self.is_syncing_history;
        let stopping = state == "cancelling";
        let resumable = !active && self.history_status.as_ref().is_some_and(|st| st.resumable);
        let depth = self.history_depth;
        let retry_finished = self.history_retry_finished;
        let entity = cx.entity();

        let action = if active {
            stop_button(p, if stopping { "Stopping…" } else { "Stop sync" }, !stopping)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.cancel_history_sync(cx)))
                .into_any_element()
        } else if resumable {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(button(p, "Start over", false, true).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.start_history_sync(false, cx)),
                ))
                .child(button(p, "Resume", true, true).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.start_history_sync(true, cx)),
                ))
                .into_any_element()
        } else {
            button(p, "Sync history", true, true)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.start_history_sync(false, cx)))
                .into_any_element()
        };

        // How far back to go: a dropdown instead of a row of chips.
        let depth_picker = div()
            .relative()
            .w(px(168.0))
            .child(
                div()
                    .id("history-depth-button")
                    .w(px(168.0))
                    .h(px(32.0))
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(p.border)
                    .bg(p.tag)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .when(!active, |d| d.cursor_pointer())
                    .when(active, |d| d.opacity(0.6))
                    .child(div().text_xs().text_color(p.text).child(depth.label()))
                    .child(Icon::new(IconName::ChevronDown).text_color(p.muted))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        if !this.is_syncing_history {
                            this.show_history_depth_picker = !this.show_history_depth_picker;
                            cx.notify();
                        }
                    })),
            )
            .children((self.show_history_depth_picker && !active).then(|| {
                dropdown_list(
                    168.0,
                    p.card,
                    p.border,
                    SyncDepth::ALL.into_iter().map(|choice| {
                        let selected = choice == depth;
                        div()
                            .px_3()
                            .py_1p5()
                            .text_xs()
                            .text_color(if selected { p.primary } else { p.text })
                            .cursor_pointer()
                            .hover(|s| s.bg(p.tag))
                            .child(choice.label())
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.history_depth = choice;
                                this.show_history_depth_picker = false;
                                cx.notify();
                            }))
                    }),
                )
            }));

        let retry_switch = {
            let entity = entity.clone();
            Switch::new("history-retry-finished").checked(retry_finished).disabled(active).on_change(move |on, _, cx| {
                let on = *on;
                entity.update(cx, |this, cx| {
                    this.history_retry_finished = on;
                    cx.notify();
                });
            })
        };

        let mut card = div()
            .px_4()
            .py_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(header(
                p,
                "Chat history",
                "Load older messages from your phone. Existing messages are never removed.",
                None,
                action,
            ))
            .child(option_row(p, "How much to load", depth.hint().to_string(), depth_picker))
            .child(option_row(
                p,
                "Retry synced chats",
                history_sync::finished_chats_text(self.history_status.as_ref()),
                retry_switch,
            ));

        let Some(st) = self.history_status.clone() else { return card };
        if !active && st.started_at.is_none() {
            return card;
        }

        let mut headline = vec![history_sync::state_label(&st.state).to_string()];
        headline.extend(history_sync::chats_text(&st));
        let timing: Vec<String> = history_sync::elapsed_text(&st).into_iter().chain(history_sync::eta_text(&st)).collect();

        let mut status = div().flex().flex_col().gap_1p5().pt_3().border_t_1().border_color(p.border);
        if active {
            status = status.child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(p.text).child(headline.join(" · ")));
            if st.chats_total > 0 || active {
                let fraction = history_sync::progress_fraction(&st);
                let working = history_sync::in_progress_fraction(&st);
                status = status.child(progress_bar(
                    p,
                    &[(if fraction < 0.01 { 0.01 } else { fraction }, p.primary), (working, p.primary.opacity(0.4))],
                ));
            }
            if !timing.is_empty() {
                status = status.child(div().text_xs().text_color(p.muted).child(timing.join(" · ")));
            }
            for current in st.current.iter().take(3) {
                status = status.child(
                    div().text_xs().text_color(p.muted).child(format!("Now: {}", history_sync::current_text(current))),
                );
            }
            if st.current.is_empty() && st.chats_total > 0 {
                status = status.child(div().text_xs().text_color(p.muted).child("Now: picking the next chat…"));
            }
        }
        if let Some(note) = history_sync::paused_text(&st) {
            status = status.child(div().text_xs().text_color(rgb(0xd97706)).child(note));
        }
        status = status.child(div().text_xs().text_color(p.muted).child(history_sync::result_text(&st)));
        if let Some(note) = history_sync::staged_text(&st) {
            status = status.child(div().text_xs().text_color(p.muted).child(note));
        }
        if let Some(error) = history_sync::error_text(&st) {
            status = status.child(div().text_xs().text_color(rgb(0xef4444)).child(error));
        }
        card = card.child(status);
        card
    }

    pub(super) fn render_media_section(&mut self, cx: &mut Context<Self>, p: Palette) -> Div {
        let state = self.media_status.as_ref().map(|st| st.state.clone()).unwrap_or_default();
        let active = history_media::is_active(&state) || self.is_downloading_media;
        let stopping = state == "cancelling";
        let resumable = !active && self.media_status.as_ref().is_some_and(|st| st.resumable);
        let nothing_chosen = self.media_kinds.is_empty();
        let ask_phone = self.media_ask_phone;
        let retry = self.media_retry_unavailable;
        let entity = cx.entity();

        let chips = history_media::MEDIA_KINDS.into_iter().map(|kind| {
            let selected = self.media_kinds.iter().any(|k| k == kind.id);
            let label = match self.media_status.as_ref() {
                Some(st) => format!("{} · {}", kind.label, history_media::files_of_kind(Some(st), kind.id, ask_phone)),
                None => kind.label.to_string(),
            };
            div()
                .px_2p5()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(if selected { p.primary } else { p.border })
                .bg(if selected { p.tag } else { gpui::transparent_black() })
                .text_xs()
                .text_color(if selected { p.primary } else { p.text })
                .when(!active, |d| d.cursor_pointer().hover(|s| s.bg(p.secondary)))
                .child(label)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    if this.is_downloading_media {
                        return;
                    }
                    if let Some(at) = this.media_kinds.iter().position(|k| k == kind.id) {
                        this.media_kinds.remove(at);
                    } else {
                        this.media_kinds.push(kind.id.to_string());
                    }
                    cx.notify();
                }))
        });

        let action = if active {
            stop_button(p, if stopping { "Stopping…" } else { "Stop download" }, !stopping)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.cancel_history_media(cx)))
                .into_any_element()
        } else if resumable {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(button(p, "Start over", false, !nothing_chosen).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.start_history_media(false, cx)),
                ))
                .child(button(p, "Resume", true, true).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.start_history_media(true, cx)),
                ))
                .into_any_element()
        } else {
            button(p, "Download media", true, !nothing_chosen)
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.start_history_media(false, cx)))
                .into_any_element()
        };

        let ask_switch = {
            let entity = entity.clone();
            Switch::new("media-ask-phone").checked(ask_phone).disabled(active).on_change(move |on, _, cx| {
                let on = *on;
                entity.update(cx, |this, cx| {
                    this.media_ask_phone = on;
                    cx.notify();
                });
            })
        };
        let retry_switch = Switch::new("media-retry-unavailable").checked(retry).disabled(active).on_change(move |on, _, cx| {
            let on = *on;
            entity.update(cx, |this, cx| {
                this.media_retry_unavailable = on;
                cx.notify();
            });
        });

        let mut section = div()
            .px_4()
            .py_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(header(
                p,
                "Chat media",
                "History messages only keep their text. Download their photos, stickers and files so they open instantly.",
                history_media::usage_text(self.storage_usage.as_ref()).map(|usage| format!("Storage used: {usage}")),
                action,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().flex_row().flex_wrap().gap_1p5().children(chips))
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(history_media::chosen_text(self.media_status.as_ref(), &self.media_kinds, ask_phone)),
                    ),
            )
            .child(option_row(
                p,
                "Ask my phone for media that expired",
                "Your phone must be online with WhatsApp open. Only files still on the phone can be recovered.".to_string(),
                ask_switch,
            ))
            .child(option_row(
                p,
                "Retry media that was no longer available",
                "WhatsApp deletes old media from its servers; files found gone are skipped unless this is on.".to_string(),
                retry_switch,
            ));

        let Some(st) = self.media_status.clone() else { return section };
        if !active && st.started_at.is_none() {
            return section;
        }

        let mut status = div().flex().flex_col().gap_1p5().pt_3().border_t_1().border_color(p.border);
        if active {
            status = status.child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(p.text).child(history_media::headline(&st)));
            if st.total > 0 || active {
                let (done, gone, failed) = history_media::progress_shares(&st);
                status = status.child(progress_bar(
                    p,
                    &[(done, p.primary), (gone, p.muted.opacity(0.5)), (failed, rgb(0xef4444).into())],
                ));
            }
            if let Some(timing) = history_media::timing_text(&st) {
                status = status.child(div().text_xs().text_color(p.muted).child(timing));
            }
            if st.current > 0 {
                status = status.child(
                    div().text_xs().text_color(p.muted).child(format!("Now: downloading {} files at once", st.current)),
                );
            }
        }
        if let Some(note) = history_media::paused_text(&st) {
            status = status.child(div().text_xs().text_color(rgb(0xd97706)).child(note));
        }
        status = status.child(div().text_xs().text_color(p.muted).child(history_media::result_text(&st)));
        if let Some(error) = history_media::error_text(&st) {
            status = status.child(div().text_xs().text_color(rgb(0xef4444)).child(error));
        }
        section = section.child(status);
        section
    }
}
