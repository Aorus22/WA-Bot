//! Root Settings view rendering. Section content stays in its local modules.

use super::*;

pub(super) fn settings_section_heading(
    shape: ShapeTokens,
    default_label: &'static str,
    gtk_label: &'static str,
    text_color: Hsla,
    muted_text: Hsla,
) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(muted_text)
        .when(shape.adwaita, |style| {
            style
                .text_size(rems(shape.typography.title4_size))
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
        })
        // Fluent section headers are BodyStrong: 14px semibold, sentence case.
        .when(shape.winui, |style| {
            style
                .text_size(rems(shape.typography.body_size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text_color)
        })
        .child(if shape.adwaita || shape.winui { gtk_label } else { default_label })
}


impl SettingsView {
    fn render_update_card(
        &self,
        text_color: Hsla,
        muted_text: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = self.update_status.clone();
        let appimage = matches!(updater::install_kind(), InstallKind::AppImage(_));
        let (line, is_error): (String, bool) = match &status {
            UpdateStatus::Idle => (String::new(), false),
            UpdateStatus::Checking => ("Checking for updates...".into(), false),
            UpdateStatus::UpToDate => ("You're up to date.".into(), false),
            UpdateStatus::Available(i) => (format!("Version {} is available.", i.version), false),
            UpdateStatus::Downloading(_) => {
                let done = self.update_progress.0.load(Ordering::Relaxed);
                let total = self.update_progress.1.load(Ordering::Relaxed);
                (
                    if total > 0 {
                        format!("Downloading... {}%", done * 100 / total)
                    } else {
                        format!("Downloading... {:.1} MB", done as f64 / 1_048_576.0)
                    },
                    false,
                )
            }
            UpdateStatus::ReadyToRestart => ("Update installed. Restart to apply.".into(), false),
            UpdateStatus::Error(e) => (e.clone(), true),
        };
        // A beta follows the rolling beta release; the stable updater would only
        // offer to swap it for the stable build.
        let is_beta = updater::CURRENT_VERSION.contains("-beta");
        let busy = matches!(status, UpdateStatus::Checking | UpdateStatus::Downloading(_));
        let mut actions = h_flex().gap_2();
        if !is_beta && !busy && !matches!(status, UpdateStatus::ReadyToRestart) {
            actions = actions.child(
                action_button("update-check", "Check for updates", ButtonVariant::Secondary, true, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx))),
            );
        }
        if let (false, UpdateStatus::Available(info)) = (is_beta, &status) {
            if appimage && info.appimage_url.is_some() && info.sums_url.is_some() {
                let i = info.clone();
                actions = actions.child(action_button("update-install", "Update now", ButtonVariant::Secondary, true, cx).on_click(
                    cx.listener(move |this, _, _, cx| this.install_update(i.clone(), cx)),
                ));
            }
            let url = info.page_url.clone();
            actions = actions.child(action_button("update-open", "Release page", ButtonVariant::Secondary, true, cx).on_click(
                cx.listener(move |_, _, _, _| {
                    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                }),
            ));
        }
        if matches!(status, UpdateStatus::ReadyToRestart) {
            actions = actions.child(
                action_button("update-restart", "Restart", ButtonVariant::Secondary, true, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.restart_app(cx))),
            );
        }

        let mut text = div().flex().flex_col().gap_1().child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_color)
                .child(format!("Version {}", updater::CURRENT_VERSION)),
        );
        if is_beta {
            text = text.child(div().text_xs().text_color(muted_text).child("Beta build. Install a newer beta to update."));
        } else if !line.is_empty() {
            let color = if is_error { gpui::red() } else { muted_text };
            text = text.child(div().text_xs().text_color(color).child(line));
        }
        if matches!(&status, UpdateStatus::Available(_)) && !appimage {
            text = text.child(
                div()
                    .text_xs()
                    .text_color(muted_text)
                    .child("Not running from an AppImage. Download the release to update."),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                settings_section_heading(cx.shape(), "VERSION & UPDATE", "Version & update", text_color, muted_text),
            )
            .child(
                card_group(cx)
                    .px_4()
                    .py_3()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(text)
                    .child(actions),
            )
            .into_any_element()
    }

}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.app_theme();
        let framed = !crate::popover::is_popover(cx)
            && window_material::is_framed_window(window);
        let card_bg = theme.card;
        let border_color = theme.border;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let primary_color = theme.primary;
        let secondary_bg = theme.secondary;
        let tag_bg = theme.muted;
        let dropdown_style = DropdownMenuStyle::from_theme(cx);
        let update_card = self.render_update_card(text_color, muted_text, cx);

        let palette = Palette {
            shape: cx.shape(),
            typography: cx.shape().typography,
            text: text_color,
            muted: muted_text,
            primary: primary_color,
            secondary: secondary_bg,
            tag: tag_bg,
            border: border_color,
        };
        // Chat history and chat media rows.
        let history_card = self.render_history_card(cx, palette);
        let media_section = self.render_media_section(cx, palette);

        let backup_section = self.render_backup_section(cx, palette);

        let ai_section = self.render_ai_section(cx, text_color, muted_text, primary_color, secondary_bg, tag_bg, border_color);

        let layout_pref_now = crate::views::chat::layout_pref::preference(cx);
        let notifications_now = crate::notifications::enabled(cx);
        let format_now = crate::text_format::enabled(cx);
        let theme_mgr = ThemeManager::global(cx);
        let active_preset_name = theme_mgr.active_preset.name.clone();
        let window_transparency = theme_mgr.window_transparency;
        let ui_font_scale = theme_mgr.ui_font_scale;
        let chat_font_scale = theme_mgr.chat_font_scale;
        let window_blur_enabled = theme_mgr.effective_window_blur_enabled();
        let gtk_style = theme_mgr.gtk_style;
        let winui_style = theme_mgr.winui_style;

        // Filter theme presets
        let mode_filter = self.theme_mode_filter.clone();
        // "Desktop (GTK)" comes first and ignores the dark/light filter: it
        // follows whatever the desktop is currently using.
        let mut filtered_presets: Vec<Cow<'static, ThemePreset>> = theme_mgr
            .gtk_preset
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
        filtered_presets.extend(
            all_presets()
                .iter()
                .filter(|p| match mode_filter.as_str() {
                    "dark" => p.is_dark(),
                    "light" => !p.is_dark(),
                    _ => true,
                })
                .map(Cow::Borrowed),
        );

        let theme_count = filtered_presets.len();
        let (active_theme_label, active_swatch) = {
            let active = &theme_mgr.active_preset;
            (
                active.label.clone(),
                (
                    parse_hex_hsla(&active.colors.background),
                    parse_hex_hsla(&active.colors.primary),
                    parse_hex_hsla(&active.colors.accent),
                    parse_hex_hsla(&active.colors.destructive),
                ),
            )
        };

        let is_connected = cx.has_global::<ConnectionState>()
            && ConnectionState::global(cx).status == ConnectionStatus::Connected;
        let is_logged_in = cx.has_global::<AuthState>()
            && AuthState::global(cx).status == AuthStatus::Authenticated;
        let linked_phone = if cx.has_global::<AuthState>() {
            AuthState::global(cx).phone.clone()
        } else {
            None
        };

        let show_tts_picker = self.show_tts_provider_picker;

        let tts_provider_label = match self.tts_provider.as_str() {
            "fish" => "Fish Audio",
            "edge" => "Edge TTS",
            _ => "Disabled",
        };

        let theme_modal = if self.show_theme_modal {
            let grid = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(12.0))
                                                    .children(filtered_presets.into_iter().map(|preset| {
                                                        let is_active = preset.name == active_preset_name;
                                                        let preset_name = preset.name.clone();
                                                        let p_bg = parse_hex_hsla(&preset.colors.background);
                                                        let p_primary = parse_hex_hsla(&preset.colors.primary);
                                                        let p_accent = parse_hex_hsla(&preset.colors.accent);
                                                        let p_destructive = parse_hex_hsla(&preset.colors.destructive);
                                                        let p_fg = parse_hex_hsla(&preset.colors.foreground);
                                                        let p_muted_fg = parse_hex_hsla(&preset.colors.muted_foreground);
                                                        let clean_label = preset.label.clone();

                                                        div()
                                                            .relative()
                                                            .w(px(204.0))
                                                            .p_2()
                                                            .rounded(px(cx.shape().radius_group))
                                                            .border_2()
                                                            .border_color(if is_active {
                                                                primary_color
                                                            } else {
                                                                border_color
                                                            })
                                                            .bg(if is_active {
                                                                tag_bg
                                                            } else {
                                                                secondary_bg
                                                            })
                                                            .cursor_pointer()
                                                            .hover(|s| s.border_color(theme.muted_foreground))
                                                            // Mini preview box
                                                            .child(
                                                                div()
                                                                    .h(px(56.0))
                                                                    .w_full()
                                                                    .rounded(px(cx.shape().radius_control))
                                                                    .p_2()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .justify_between()
                                                                    .bg(p_bg)
                                                                    // 3 colored dots
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_row()
                                                                            .gap_1()
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_primary))
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_accent))
                                                                            .child(div().size(px(8.0)).rounded_full().bg(p_destructive)),
                                                                    )
                                                                    // 2 preview bars
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_row()
                                                                            .items_end()
                                                                            .gap_1()
                                                                            .child(div().w(px(110.0)).h(px(4.0)).rounded_sm().bg(p_fg))
                                                                            .child(div().w(px(40.0)).h(px(4.0)).rounded_sm().bg(p_muted_fg)),
                                                                    ),
                                                            )
                                                            // Label
                                                            .child(
                                                                div()
                                                                    .mt_1p5()
                                                                    .px_1()
                                                                    .flex()
                                                                    .flex_row()
                                                                    .items_center()
                                                                    .justify_between()
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(if is_active { text_color } else { muted_text })
                                                                            .child(clean_label),
                                                                    ),
                                                            )
                                                            // Active Checkmark icon
                                                            .children(if is_active {
                                                                Some(
                                                                    div()
                                                                        .absolute()
                                                                        .top(px(8.0))
                                                                        .right(px(8.0))
                                                                        .child(Icon::new(IconName::Check).text_color(primary_color)),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, window, cx| {
                                                                ThemeManager::apply_preset(&preset_name, Some(window), cx);
                                                            }))
                                                    }))
                ;
            let chips = ["all", "dark", "light"].into_iter().map(|m| {
                let selected = mode_filter == m;
                div()
                    .px_2p5()
                    .py_1()
                    .rounded(px(cx.shape().radius_control))
                    .border_1()
                    .border_color(if selected { primary_color } else { border_color })
                    .bg(if selected { tag_bg } else { gpui::transparent_black() })
                    .text_xs()
                    .text_color(if selected { primary_color } else { text_color })
                    .cursor_pointer()
                    .hover(|s| s.bg(secondary_bg))
                    .child(match m {
                        "dark" => "Dark",
                        "light" => "Light",
                        _ => "All",
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        this.set_theme_mode_filter(m, window, cx);
                    }))
            });
            Some(crate::components::motion::modal_in(
                div()
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(if cx.shape().adwaita { cx.surface().scrim } else { cx.surface().scrim_default })
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.show_theme_modal = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(720.0))
                            .max_h(px(600.0))
                            .rounded(px(cx.shape().radius_card))
                            .border_1()
                            .border_color(border_color)
                            .bg(card_bg)
                            .p_6()
                            .shadow_2xl()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                if ev.keystroke.key.as_str() == "escape" {
                                    this.show_theme_modal = false;
                                    cx.notify();
                                }
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child("Choose a theme"))
                                            .child(div().text_xs().text_color(muted_text).child(format!("{theme_count} themes"))),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .child(div().flex().flex_row().gap_1p5().children(chips))
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .text_sm()
                                                    .text_color(muted_text)
                                                    .child("✕")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.show_theme_modal = false;
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("theme-modal-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(grid),
                            ),
                    ),
                "theme-modal",
            ))
        } else {
            None
        };

        div()
            .id("settings-root")
            .relative()
            .size_full()
            .bg(if cx.shape().adwaita {
                cx.surface().view
            } else {
                window_material::glass_fill(theme.background, window_transparency)
            })
            .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
            .child(
                div()
            .id("settings-scroll-area")
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.settings_scroll_handle)
            .bg(if cx.shape().adwaita {
                cx.surface().view
            } else {
                window_material::glass_fill(theme.background, window_transparency)
            })
            .when(framed, |s| s.rounded_br(px(window_material::corner_radius(cx))))
            .pt(px(40.0))
            .pb(px(60.0))
            .px_4()
            .child(
                div()
                    .w_full()
                    .max_w(px(672.0))
                    .flex()
                    .flex_col()
                    .gap(px(32.0))
                    .pb_12()
                    // Page Title
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_text)
                                    .when(cx.shape().adwaita, |style| {
                                        style
                                            .text_size(rems(cx.shape().typography.caption_size))
                                            .text_color(text_color.opacity(cx.shape().typography.caption_opacity))
                                    })
                                    .child("Customize your application appearance, credentials, and bot configurations"),
                            ),
                    )
                    // ====================================================
                    // 1. APPEARANCE SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "APPEARANCE", "Appearance", text_color, muted_text),
                            )
                            .child(
                                settings_group()
                                    // Color theme: the active one, a click opens the picker.
                                    .child(
                                        div()
                                            .id("color-theme-row")
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(tag_bg.opacity(0.5)))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Palette).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Color theme"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child(active_theme_label.clone()),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .w(px(72.0))
                                                            .h(px(32.0))
                                                            .rounded(px(cx.shape().radius_control))
                                                            .border_1()
                                                            .border_color(border_color)
                                                            .bg(active_swatch.0)
                                                            .px_2()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(div().size(px(8.0)).rounded_full().bg(active_swatch.1))
                                                            .child(div().size(px(8.0)).rounded_full().bg(active_swatch.2))
                                                            .child(div().size(px(8.0)).rounded_full().bg(active_swatch.3)),
                                                    )
                                                    .child(Icon::new(IconName::ChevronRight).text_color(muted_text)),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.show_theme_modal = true;
                                                cx.notify();
                                            })),
                                    )
                                    .divider(border_color)
                                    // Window transparency slider.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_3()
                                                            .child(Icon::new(IconName::Eye).text_color(muted_text))
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(text_color)
                                                                            .child("Window transparency"),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(muted_text)
                                                                            .child("Adjust how much of the desktop shows through"),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .w(px(280.0))
                                                            .items_center()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .w(px(42.0))
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .text_align(TextAlign::Right)
                                                                    .child(format!("{window_transparency}%")),
                                                            )
                                                            .child(
                                                                Slider::new(&self.window_transparency_slider)
                                                                    .flex_1(),
                                                            ),
                                                    ),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Interface text scale.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_3()
                                                            .child(Icon::new(IconName::ALargeSmall).text_color(muted_text))
                                                            .child(div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Interface text size"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Scales menus, lists, headers and settings"),
                                                            ),)
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .w(px(280.0))
                                                            .items_center()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .w(px(42.0))
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .text_align(TextAlign::Right)
                                                                    .child(format!("{ui_font_scale}%")),
                                                            )
                                                            .child(Slider::new(&self.ui_font_scale_slider).flex_1()),
                                                    ),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Message bubble text scale.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_3()
                                                            .child(Icon::new(IconName::CaseSensitive).text_color(muted_text))
                                                            .child(div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Chat text size"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Scales the text inside message bubbles only"),
                                                            ),)
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .w(px(280.0))
                                                            .items_center()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .w(px(42.0))
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .text_align(TextAlign::Right)
                                                                    .child(format!("{chat_font_scale}%")),
                                                            )
                                                            .child(Slider::new(&self.chat_font_scale_slider).flex_1()),
                                                    ),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Blur uses the compositor's default strength.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Eye).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Backdrop blur"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child(if winui_style { "Acrylic blur is enabled by WinUI style" } else { "Blur strength is controlled by COSMIC" }),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("window-backdrop-blur")
                                                    .checked(window_blur_enabled)
                                                    .disabled(winui_style)
                                                    .on_change(|enabled, _, cx| {
                                                        ThemeManager::apply_window_blur_enabled(*enabled, cx);
                                                    }),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row: GTK style (shapes, on top of any colour theme)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Palette).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("GTK style"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Flat, rounded shapes like a GNOME app. Works with any colour theme"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("gtk-style")
                                                    .checked(gtk_style)
                                                    .on_change(|enabled, _, cx| {
                                                        ThemeManager::apply_gtk_style(*enabled, cx);
                                                    }),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row: WinUI style (Fluent surfaces and Acrylic material)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Palette).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("WinUI style"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Windows 11 Fluent shapes and Acrylic. Works with any colour theme"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("winui-style")
                                                    .checked(winui_style)
                                                    .on_change(|enabled, _, cx| {
                                                        ThemeManager::apply_winui_style(*enabled, cx);
                                                    }),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row: Chat layout (pages vs sidebar)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::PanelLeft).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Chat Layout"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Auto: pages in the popover, sidebar in a window"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .gap_1()
                                                    .p_0p5()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .children([
                                                        (ChatLayout::Auto, "Auto"),
                                                        (ChatLayout::Pages, "Detail"),
                                                        (ChatLayout::Sidebar, "Sidebar"),
                                                    ].into_iter().map(|(value, label)| {
                                                        let active = layout_pref_now == value
                                                            || (value == ChatLayout::Auto && layout_pref_now == ChatLayout::Unknown);
                                                        div()
                                                            .id(label)
                                                            .cursor_pointer()
                                                            .px_3()
                                                            .py_1()
                                                            .rounded(px(cx.shape().radius_control))
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .bg(if active { primary_color } else { rgba(0x00000000).into() })
                                                            .text_color(if active { theme.primary_foreground } else { text_color })
                                                            .child(label)
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                                                                crate::views::chat::layout_pref::set(cx, value);
                                                            }))
                                                    })),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row: OS notifications for incoming messages
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::Bell).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Notifications"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Show a desktop notification for new messages while the app is not focused"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("os-notifications")
                                                    .checked(notifications_now)
                                                    .on_change(|enabled, _, cx| {
                                                        crate::notifications::set_enabled(cx, *enabled);
                                                    }),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row: WhatsApp text formatting in message bubbles
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::FileText).text_color(muted_text))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Format message text"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Show *bold*, _italic_, ~strikethrough~ and `monospace` like WhatsApp"),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("format-messages")
                                                    .checked(format_now)
                                                    .on_change(|enabled, _, cx| {
                                                        crate::text_format::set_enabled(cx, *enabled);
                                                    }),
                                            ),
                                    )
                            ),
                    )
                    // ====================================================
                    // 2. ACCOUNT SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "ACCOUNT", "Account", text_color, muted_text),
                            )
                            .child(
                                settings_group()
                                    // Row 1: Connection & Logout
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .size(px(10.0))
                                                            .rounded_full()
                                                            .bg(if is_connected { rgb(0x10b981) } else { rgb(0xef4444) }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child(if is_logged_in {
                                                                        if is_connected { "Connected" } else { "Reconnecting..." }
                                                                    } else {
                                                                        "Not connected"
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child(if is_logged_in {
                                                                        match linked_phone.as_deref() {
                                                                            Some(phone) => format!("Linked account: +{phone}"),
                                                                            None => "WhatsApp bot session is active".to_string(),
                                                                        }
                                                                    } else {
                                                                        "No WhatsApp account linked".to_string()
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                if is_logged_in {
                                                    div()
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded(px(cx.shape().radius_control))
                                                        .bg(rgba(0xef444420))
                                                        .border_1()
                                                        .border_color(rgba(0xef444460))
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(rgba(0xef444440)))
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(rgb(0xef4444))
                                                                .child("Logout"),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, window, cx| {
                                                            open_logout_dialog(window, cx, move |_window, cx| {
                                                                let base_url = if cx.has_global::<AuthState>() {
                                                                    AuthState::global(cx).base_url.clone()
                                                                } else {
                                                                    "http://127.0.0.1:3000/api".to_string()
                                                                };
                                                                cx.spawn(async move |cx| {
                                                                    let client = HttpClient::new(&base_url);
                                                                    let _ = client.logout().await;
                                                                    let _ = cx.update(|cx| {
                                                                        if cx.has_global::<AuthState>() {
                                                                            AuthState::global_mut(cx).set_status(AuthStatus::Unauthenticated);
                                                                        }
                                                                        if cx.has_global::<Router>() {
                                                                            Router::global_mut(cx).navigate(AppRoute::Chat);
                                                                        }
                                                                        cx.refresh_windows();
                                                                    });
                                                                })
                                                                .detach();
                                                            });
                                                        }))
                                                } else {
                                                    div()
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded(px(cx.shape().radius_control))
                                                        .bg(primary_color)
                                                        .cursor_pointer()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme.primary_foreground)
                                                                .child("Login"),
                                                        )
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _, cx| {
                                                            if cx.has_global::<AuthState>() {
                                                                AuthState::global_mut(cx).set_status(AuthStatus::Unauthenticated);
                                                                cx.refresh_windows();
                                                            }
                                                        }))
                                                },
                                            ),
                                    )
                                    .divider(border_color)
                                    // Row 2: Chat History Sync
                                    .child(history_card)
                                    .divider(border_color)
                                    .child(media_section)
                                    .divider(border_color)
                                    .child(backup_section)
                                    .divider(border_color)
                                    // Row 3: restart the local backend
                                    .child(
                                        div()
                                            .px_4()
                                            .py_4()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Backend"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Restart the local WhatsApp backend if messages stop arriving or it stops responding. Your session is kept."),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id("restart-backend")
                                                    .flex_none()
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .when(!self.is_restarting_backend, |d| {
                                                        d.cursor_pointer().hover(|s| s.bg(secondary_bg))
                                                    })
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child(if self.is_restarting_backend { "Restarting..." } else { "Restart backend" }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.restart_backend(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    .child(ai_section)
                    // ====================================================
                    // CONTACT NAMES SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "CONTACT NAMES", "Contact names", text_color, muted_text),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .children([
                                        ("saved", "Saved contact name", "The name from your phone's contacts. Contacts you haven't saved show their phone number, like WhatsApp."),
                                        ("profile", "Profile name", "The name each person chose for themselves."),
                                    ].into_iter().map(|(mode, title, description)| {
                                        let selected = self.contact_name_mode == mode;
                                        card_surface(cx)
                                            .id(SharedString::from(format!("contact-names-{mode}")))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .border_color(if selected { primary_color } else { border_color })
                                            .rounded(px(cx.shape().radius_group))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(tag_bg))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(18.0))
                                                    // A long description must not squeeze the circle into an oval.
                                                    .flex_none()
                                                    .rounded_full()
                                                    .border_2()
                                                    .border_color(if selected { primary_color } else { muted_text })
                                                    .children(selected.then(|| div().size(px(8.0)).rounded_full().bg(primary_color))),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(text_color).child(title))
                                                    .child(div().text_xs().text_color(muted_text).child(description)),
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.set_contact_name_mode(mode, cx);
                                            }))
                                    })),
                            ),
                    )
                    // ====================================================
                    // 4. PRIVACY SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "PRIVACY", "Privacy", text_color, muted_text),
                            )
                            .child(
                                settings_group()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Icon::new(IconName::CircleCheck).text_color(primary_color))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(text_color)
                                                                    .child("Read receipts"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(muted_text)
                                                                    .child("Send blue ticks to WhatsApp when you open a chat. Off keeps reads local only."),
                                                            ),
                                                    ),
                                            )
                                            // Switch Toggle Button matching web-term
                                            .child(
                                                div()
                                                    .w(px(38.0))
                                                    .h(px(22.0))
                                                    .rounded_full()
                                                    .p_0p5()
                                                    .cursor_pointer()
                                                    .bg(if self.read_receipts {
                                                        primary_color
                                                    } else {
                                                        border_color
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .size(px(18.0))
                                                            .rounded_full()
                                                            .bg(rgb(0xffffff))
                                                            .ml(if self.read_receipts { px(16.0) } else { px(1.0) }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.toggle_read_receipts(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 5. CALL TTS SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "CALL TTS", "Call TTS", text_color, muted_text),
                            )
                            .child(
                                settings_group()
                                    // Header
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .child(Icon::new(IconName::Bell).text_color(muted_text))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(text_color)
                                                            .child("Text-to-Speech Provider"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted_text)
                                                            .child("Choose voice synthesis provider for incoming calls"),
                                                    ),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Provider Dropdown Row
                                    .child(
                                        div()
                                            .relative()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_3()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Provider"),
                                            )
                                            .child(
                                                div()
                                                    .relative()
                                                    .w(px(120.0))
                                                    .child(div()
                                                    .w_full()
                                                    .h(px(30.0))
                                                    .px_3()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .justify_between()
                                                    .cursor_pointer()
                                                    .child(
                                                        div().text_xs().text_color(text_color).child(tts_provider_label),
                                                    )
                                                    .child(Icon::new(IconName::ChevronDown).text_color(muted_text))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.show_tts_provider_picker = !this.show_tts_provider_picker;
                                                        this.show_theme_mode_picker = false;
                                                        cx.notify();
                                                    })),)
                                                    .children(show_tts_picker.then(|| {
                                                        dropdown_list(
                                                            dropdown_style,
                                                            120.0,
                                                            [("", "Disabled"), ("edge", "Edge TTS"), ("fish", "Fish Audio")].into_iter().map(|(val, lbl)| {
                                                                let is_selected = self.tts_provider == val;
                                                                div()
                                                                    .px_3()
                                                                    .py_1p5()
                                                                    .text_xs()
                                                                    .text_color(if is_selected { primary_color } else { text_color })
                                                                    .cursor_pointer()
                                                                    .hover(|s| s.bg(tag_bg))
                                                                    .child(lbl)
                                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                        this.tts_provider = val.to_string();
                                                                        this.show_tts_provider_picker = false;
                                                                        cx.notify();
                                                                    }))
                                                            }),
                                                        )
                                                    })),
                                            ),
                                    )
                                    .divider(border_color)
                                    // Default Voice
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Default Voice"),
                                                    )
                                                    .child(
                                                        div().text_xs().text_color(muted_text).child(self.tts_default_voice.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .bg(tag_bg)
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .child("Edit Voice")
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                        this.open_edit_modal(EditField::TtsDefaultVoice, window, cx);
                                                    })),
                                            ),
                                    )
                                    // Fish Audio Fields (shown when fish is selected)
                                    .children(if self.tts_provider == "fish" {
                                        Some(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(div().h(px(1.0)).bg(border_color).w_full())
                                                .child(
                                                    div()
                                                        .px_4()
                                                        .py_3()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(
                                                                    div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Fish Audio Key"),
                                                                )
                                                                .child(
                                                                    div().text_xs().text_color(muted_text).child(if self.has_fish_key {
                                                                        "•••••••••••••••• (Configured)"
                                                                    } else {
                                                                        "Not configured"
                                                                    }),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .px_3()
                                                                .py_1()
                                                                .rounded(px(cx.shape().radius_control))
                                                                .bg(tag_bg)
                                                                .border_1()
                                                                .border_color(border_color)
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(secondary_bg))
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child("Edit Key")
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                                    this.open_edit_modal(EditField::FishAudioKey, window, cx);
                                                                })),
                                                        ),
                                                )
                                                .child(div().h(px(1.0)).bg(border_color).w_full())
                                                .child(
                                                    div()
                                                        .px_4()
                                                        .py_3()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(
                                                                    div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Fish Audio Model"),
                                                                )
                                                                .child(
                                                                    div().text_xs().text_color(muted_text).child(self.fish_audio_model.clone()),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .px_3()
                                                                .py_1()
                                                                .rounded(px(cx.shape().radius_control))
                                                                .bg(tag_bg)
                                                                .border_1()
                                                                .border_color(border_color)
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(secondary_bg))
                                                                .text_xs()
                                                                .text_color(text_color)
                                                                .child("Edit Model")
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                                    this.open_edit_modal(EditField::FishAudioModel, window, cx);
                                                                })),
                                                        ),
                                                ),
                                        )
                                    } else {
                                        None
                                    })
                                    .divider(border_color)
                                    // Save Button
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .justify_end()
                                            .child(
                                                div()
                                                    .px_4()
                                                    .py_1p5()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .bg(primary_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.primary_foreground)
                                                            .child(if self.is_saving_tts { "Saving..." } else { "Save TTS Settings" }),
                                                    )
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.save_tts_settings(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    // ====================================================
                    // 6. CONNECTION & INFO SECTION
                    // ====================================================
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(
                                settings_section_heading(cx.shape(), "CONNECTION INFO", "Connection info", text_color, muted_text),
                            )
                            .child(
                                settings_group()
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Backend Endpoint"),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted_text).child(
                                                    if cx.has_global::<AuthState>() {
                                                        AuthState::global(cx).base_url.clone()
                                                    } else {
                                                        "http://127.0.0.1:3000/api".to_string()
                                                    },
                                                ),
                                            ),
                                    )
                                    .divider(border_color)
                                    .child(
                                        div()
                                            .px_4()
                                            .py_3()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(text_color).child("Platform Framework"),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted_text).child("GPUI (DirectX / DirectWrite Native)"),
                                            ),
                                    ),
                            ),
                    )
                    .child(update_card)
            )
            )
            .child(
                Scrollbar::vertical(&self.settings_scroll_handle)
                    .mode(ScrollbarMode::Always)
                    .styles(|s| {
                        s.track(|t| t.bg(rgba(0x00000000).into()))
                            .thumb(|th| th.bg(theme.muted_foreground.opacity(0.35)).radius(px(3.0)).width(px(6.0)))
                            .thumb_hover(|th| th.bg(theme.muted_foreground.opacity(0.65)).radius(px(4.0)).width(px(8.0)))
                            .thumb_active(|th| th.bg(theme.primary.opacity(0.8)).radius(px(4.0)).width(px(8.0)))
                    }),
            )
            .children(theme_modal)
            // ====================================================
            // 7. EDIT MODAL DIALOG
            // ====================================================
            .children(if let Some(modal) = &self.edit_modal {
                let title = modal.title.clone();
                let desc = modal.description.clone();
                let presets = modal.presets.clone();

                Some(crate::components::motion::modal_in(
                    div()
                        .absolute()
                        .occlude()
                        .inset_0()
                        .bg(if cx.shape().adwaita { cx.surface().scrim } else { cx.surface().scrim_default })
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.edit_modal = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(px(500.0))
                                .rounded(px(cx.shape().radius_card))
                                .border_1()
                                .border_color(border_color)
                                .bg(card_bg)
                                .p_6()
                                .shadow_2xl()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                    if ev.keystroke.key.as_str() == "escape" {
                                        this.edit_modal = None;
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()) // keep clicks inside the card from closing the modal
                                // Modal Header
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(text_color)
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .text_sm()
                                                .text_color(muted_text)
                                                .child("✕")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.edit_modal = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_text)
                                        .child(desc),
                                )
                                // Value input
                                .children(self.modal_field.as_ref().map(|field| field.element(&theme, cx.shape(), None)))
                                // Preset Chips (if any)
                                .children(if !presets.is_empty() {
                                    Some(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .flex_wrap()
                                            .gap_1p5()
                                            .children(presets.into_iter().map(|(lbl, val)| {
                                                div()
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded(px(cx.shape().radius_control))
                                                    .border_1()
                                                    .border_color(border_color)
                                                    .bg(tag_bg)
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(secondary_bg))
                                                    .child(lbl)
                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                        if let Some(field) = this.modal_field.clone() {
                                                            field.set_value(val, window, cx);
                                                        }
                                                    }))
                                            })),
                                    )
                                } else {
                                    None
                                })
                                // Actions
                                .child(
                                    div()
                                        .mt_2()
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_4()
                                                .py_1p5()
                                                .rounded(px(cx.shape().radius_control))
                                                .bg(tag_bg)
                                                .border_1()
                                                .border_color(border_color)
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(text_color)
                                                .child("Cancel")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.edit_modal = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .px_4()
                                                .py_1p5()
                                                .rounded(px(cx.shape().radius_control))
                                                .bg(primary_color)
                                                .cursor_pointer()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.primary_foreground)
                                                .child("Done")
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.save_modal_value(cx);
                                                })),
                                        ),
                                ),
                        ),
                        "settings-edit-in",
                    ))
            } else {
                None
            })
    }
}
