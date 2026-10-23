//! Backup section of the Settings page: pick a folder, back up now, see the backups.

use gpui::*;
use gpui::prelude::FluentBuilder as _;

use crate::components::ui::{boxed_list::boxed_list, card_group, h_flex};
use crate::theme::shape::ShapeExt;
use super::SettingsView;
use super::controls::{header, option_row, progress_bar, Palette};
use crate::components::ui::{ButtonVariant, action_button};

/// Backups kept per account after each run.
pub(super) const KEEP_BACKUPS: u32 = 5;

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "snapshot" => "Checking the databases",
        "chat" => "Saving chats",
        "media" => "Saving media",
        "manifest" => "Finishing",
        _ => "Working",
    }
}

fn format_time(ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

impl SettingsView {
    pub(super) fn render_backup_section(&mut self, cx: &mut Context<Self>, p: Palette) -> Div {
        let shape = cx.shape();
        let running = self.is_backing_up;
        let has_folder = self.backup_folder.is_some();

        let action = action_button(
            "backup-start",
            if running { "Backing up…" } else { "Back up now" },
            ButtonVariant::Primary,
            has_folder && !running,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| this.start_backup(cx)));

        let folder = self.backup_folder.clone();
        let change = action_button(
            "backup-change-folder",
            if has_folder { "Change folder" } else { "Choose folder" },
            ButtonVariant::Secondary,
            !running,
            cx,
        )
            .on_click(cx.listener(|this, _, _, cx| {
                if !this.is_backing_up {
                    this.choose_backup_folder(cx);
                }
            }));

        let mut section = card_group(cx)
            .px_4()
            .py_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(header(
                p,
                "Backup",
                "Copies your chats and media into a folder. Your WhatsApp login is not included.",
                None,
                action,
            ))
            .child(option_row(
                p,
                "Backup folder",
                folder.unwrap_or_else(|| format!("No folder chosen. The newest {KEEP_BACKUPS} backups are kept.")),
                change,
            ));

        if let Some(st) = self.backup_status.as_ref() {
            if running {
                let mut detail = stage_label(&st.stage).to_string();
                if st.total > 0 {
                    detail.push_str(&format!(" · {} of {}", st.done, st.total));
                } else if !st.item.is_empty() {
                    detail.push_str(&format!(" · {}", st.item));
                }
                let mut status = div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .pt_3()
                    .border_t_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .text_size(rems(p.typography.caption_size))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text)
                            .child(format!("Backing up · {}%", st.percent)),
                    )
                    .child(progress_bar(p, &[((st.percent.max(1) as f32) / 100.0, p.primary)]))
                    .child(div().text_size(rems(p.typography.caption_size)).text_color(p.muted).child(detail));
                if st.account_count > 1 {
                    status = status.child(
                        div()
                            .text_size(rems(p.typography.caption_size))
                            .text_color(p.muted)
                            .child(format!("Account {} of {}", st.account_index, st.account_count)),
                    );
                }
                section = section.child(status);
            } else if !st.error.is_empty() {
                section = section.child(
                    div().pt_3().border_t_1().border_color(p.border).text_size(rems(p.typography.caption_size)).text_color(rgb(0xef4444)).child(st.error.clone()),
                );
            } else if !st.summary.is_empty() {
                section = section.child(
                    div().pt_3().border_t_1().border_color(p.border).text_size(rems(p.typography.caption_size)).text_color(p.muted).child(st.summary.clone()),
                );
            }
        }

        if !self.backups.is_empty() {
            let mut newest: Vec<_> = self.backups.iter().collect();
            newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let rows = newest.into_iter().take(5).map(|b| {
                h_flex()
                    .items_center()
                    .text_size(rems(p.typography.caption_size))
                    .text_color(p.muted)
                    .child(format!("{}  ·  {}", format_time(b.created_at), b.account))
            });
            let backup_rows = boxed_list(cx, rows).when(!shape.adwaita, |list| list.gap_0p5());
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(rems(p.typography.caption_size))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text)
                            .child(format!("{} backups in this folder", self.backups.len())),
                    )
                    .child(backup_rows),
            );
        }
        section
    }
}
