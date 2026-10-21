//! Backup section of the Settings page: pick a folder, back up now, see the backups.

use gpui::*;
use wabot_backend_client::client::HttpClient;
use wabot_backend_client::dto::BackupRequest;
use wabot_backend_client::ClientError;
use wabot_settings::DesktopSettings;

use super::settings::SettingsView;
use super::settings_history::{button, header, option_row, progress_bar, Palette};
use crate::components::toast;
use crate::state::auth::AuthState;
use crate::TOKIO_RT;

/// Backups kept per account after each run.
const KEEP_BACKUPS: u32 = 5;

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
    /// Backup folder, status and list as the backend knows them.
    pub fn load_backup(&mut self, cx: &mut Context<Self>) {
        self.backup_folder = DesktopSettings::load().ok().and_then(|s| s.backup_folder);
        let base_url = Self::backend_base_url(cx);
        let target = self.backup_folder.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let status = TOKIO_RT
                    .spawn({
                        let base_url = base_url.clone();
                        async move { HttpClient::new(&base_url).get_backup_status().await }
                    })
                    .await;
                let list = match target {
                    Some(target) => TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).list_backups(&target).await })
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => None,
                };
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            if let Ok(Ok(status)) = status {
                                this.is_backing_up = status.running;
                                this.backup_status = Some(status);
                                this.watch_backup(cx);
                            }
                            if let Some(list) = list {
                                this.backups = list;
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Ask for a folder and remember it.
    pub fn choose_backup_folder(&mut self, cx: &mut Context<Self>) {
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let options = PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Use this folder for backups".into()),
                };
                let receiver = cx_handle.update(|cx: &mut App| cx.prompt_for_paths(options));
                let Ok(Ok(Some(paths))) = receiver.await else { return };
                let Some(folder) = paths.into_iter().next() else { return };
                let folder = folder.to_string_lossy().to_string();
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Ok(mut settings) = DesktopSettings::load() {
                        settings.backup_folder = Some(folder.clone());
                        let _ = settings.save();
                    }
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.backup_folder = Some(folder);
                            this.backups.clear();
                            this.load_backup(cx);
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Start a backup via POST /api/backup/run and follow it.
    pub fn start_backup(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.backup_folder.clone() else { return };
        if self.is_backing_up {
            return;
        }
        self.is_backing_up = true;
        self.backup_status = None;
        let base_url = Self::backend_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let request = BackupRequest { target, keep: KEEP_BACKUPS };
                let res = TOKIO_RT
                    .spawn(async move { HttpClient::new(&base_url).start_backup(&request).await })
                    .await;
                let _ = cx_handle.update(|cx: &mut App| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            match res {
                                Ok(Ok(_)) => this.watch_backup(cx),
                                other => {
                                    this.is_backing_up = false;
                                    let reason = match other {
                                        Ok(Err(ClientError::Api { message, .. })) => message,
                                        Ok(Err(e)) => e.to_string(),
                                        Err(e) => e.to_string(),
                                        Ok(Ok(_)) => String::new(),
                                    };
                                    toast::error(format!("Could not start the backup: {reason}"), cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Refetch the backup status every second until the run ends.
    fn watch_backup(&mut self, cx: &mut Context<Self>) {
        if self.backup_polling || !self.is_backing_up {
            return;
        }
        self.backup_polling = true;
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                loop {
                    cx_handle.background_executor().timer(std::time::Duration::from_millis(1000)).await;
                    let Some(base_url) = cx_handle.update(|cx: &mut App| {
                        cx.has_global::<AuthState>().then(|| AuthState::global(cx).base_url.clone())
                    }) else {
                        break;
                    };
                    let res = TOKIO_RT
                        .spawn(async move { HttpClient::new(&base_url).get_backup_status().await })
                        .await;
                    let Ok(Ok(status)) = res else { continue };
                    let running = status.running;
                    let alive = cx_handle.update(|cx: &mut App| {
                        let Some(view) = this.upgrade() else { return false };
                        view.update(cx, |this, cx| {
                            this.is_backing_up = running;
                            if !running {
                                this.backup_polling = false;
                                if status.error.is_empty() {
                                    toast::success("Backup finished", cx);
                                } else {
                                    toast::error(format!("Backup failed: {}", status.error), cx);
                                }
                            }
                            this.backup_status = Some(status);
                            if !running {
                                this.load_backup(cx);
                            }
                            cx.notify();
                        });
                        true
                    });
                    if !alive || !running {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    pub(super) fn render_backup_section(&mut self, cx: &mut Context<Self>, p: Palette) -> Div {
        let running = self.is_backing_up;
        let has_folder = self.backup_folder.is_some();

        let action = button(p, if running { "Backing up…" } else { "Back up now" }, true, has_folder && !running)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.start_backup(cx)));

        let folder = self.backup_folder.clone();
        let change = button(p, if has_folder { "Change folder" } else { "Choose folder" }, false, !running)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if !this.is_backing_up {
                    this.choose_backup_folder(cx);
                }
            }));

        let mut section = div()
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
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text)
                            .child(format!("Backing up · {}%", st.percent)),
                    )
                    .child(progress_bar(p, &[((st.percent.max(1) as f32) / 100.0, p.primary)]))
                    .child(div().text_xs().text_color(p.muted).child(detail));
                if st.account_count > 1 {
                    status = status.child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(format!("Account {} of {}", st.account_index, st.account_count)),
                    );
                }
                section = section.child(status);
            } else if !st.error.is_empty() {
                section = section.child(
                    div().pt_3().border_t_1().border_color(p.border).text_xs().text_color(rgb(0xef4444)).child(st.error.clone()),
                );
            } else if !st.summary.is_empty() {
                section = section.child(
                    div().pt_3().border_t_1().border_color(p.border).text_xs().text_color(p.muted).child(st.summary.clone()),
                );
            }
        }

        if !self.backups.is_empty() {
            let mut newest: Vec<_> = self.backups.iter().collect();
            newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text)
                            .child(format!("{} backups in this folder", newest.len())),
                    )
                    .children(newest.into_iter().take(5).map(|b| {
                        div().text_xs().text_color(p.muted).child(format!("{}  ·  {}", format_time(b.created_at), b.account))
                    })),
            );
        }
        section
    }
}
