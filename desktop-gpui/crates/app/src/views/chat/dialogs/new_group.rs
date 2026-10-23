use crate::components::ui::boxed_list::boxed_list;
use crate::theme::surface::SurfaceExt;
use crate::views::chat::*;

impl ChatView {
    /// New group modal.
    pub(crate) fn render_new_group_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = *cx.app_theme();
        let border_color = theme.border;
        let card_bg = theme.card;
        let primary_color = theme.primary;
        let text_color = theme.foreground;
        let muted_text = theme.muted_foreground;
        let shape = cx.shape();
        let surface = cx.surface();
        let scrim = if shape.adwaita { surface.scrim } else { surface.scrim_default };
        let dialog_bg = if shape.adwaita { surface.dialog } else { card_bg };
        let dialog_border = if shape.adwaita { surface.border } else { border_color };
        if self.is_new_group_open {
            let adding = self.add_members_for.is_some();
            let already_in: HashSet<String> = if adding {
                self.group_info
                    .as_ref()
                    .map(|g| g.participants.iter().map(|p| p.jid.clone()).collect())
                    .unwrap_or_default()
            } else {
                HashSet::new()
            };
            let contacts: Vec<Chat> = if cx.has_global::<ChatStore>() {
                ChatStore::global(cx)
                    .chats
                    .iter()
                    .filter(|c| !c.is_group && !c.archived && !already_in.contains(&c.id))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            Some(crate::components::motion::modal_in(
                div()
                    .absolute()
                    .occlude()
                    .inset_0()
                    .bg(scrim)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .p_5()
                            .rounded_2xl()
                            .when(shape.adwaita, |dialog| dialog.rounded(px(shape.radius_popover)))
                            .bg(dialog_bg)
                            .border_1()
                            .border_color(dialog_border)
                            .when(!shape.adwaita, |dialog| dialog.shadow_2xl())
                            .when(shape.adwaita, |dialog| dialog.shadow_md())
                            .gap_4()
                            // Header
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .when(shape.adwaita, |header| header.h(px(shape.header_height)).bg(surface.headerbar))
                                    .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(text_color).child(if adding { "Add members" } else { "New group" }))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .p_1()
                                            .rounded(px(cx.shape().radius_button))
                                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                            .child(svg().data(X_SVG).size(px(16.0)).text_color(muted_text))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_new_group_open = false;
                                                this.add_members_for = None;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Group Name Input
                            .children((!adding).then(|| self.new_group_field.as_ref().map(|field| field.element(&theme, cx.shape(), None))).flatten())
                            // Contact Picker Scroll
                            .child(
                                v_flex()
                                    .id("new-group-contacts-scroll")
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .child(boxed_list(cx, contacts.into_iter().map(|c| {
                                        let is_checked = self.new_group_selected.contains(&c.id);
                                        let cid = c.id.clone();
                                        h_flex()
                                            .cursor_pointer()
                                            .when(!shape.adwaita, |row| row.p_2().rounded_lg())
                                            .items_center()
                                            .justify_between()
                                            .bg(if is_checked {
                                                if shape.adwaita { surface.card_shade } else { theme.primary.opacity(0.12) }
                                            } else {
                                                rgba(0x00000000).into()
                                            })
                                            .hover(|s| s.bg(if shape.adwaita { theme.foreground.opacity(0.03) } else { theme.muted.opacity(0.4) }))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                if this.new_group_selected.contains(&cid) {
                                                    this.new_group_selected.remove(&cid);
                                                } else {
                                                    this.new_group_selected.insert(cid.clone());
                                                }
                                                cx.notify();
                                            }))
                                            .child(div().text_sm().text_color(text_color).child(c.name.clone()))
                                            .child(
                                                div()
                                                    .w(px(20.0))
                                                    .h(px(20.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(primary_color)
                                                    .bg(if is_checked { primary_color } else { rgba(0x00000000).into() })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .children(if is_checked {
                                                        Some(svg().data(CHECK_SVG).size(px(12.0)).text_color(rgb(0xffffff)))
                                                    } else {
                                                        None
                                                    }),
                                            )
                                    })))
                                    .when(!shape.adwaita, |list| list.gap_1())
                            )
                            // Actions
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .when(shape.adwaita, |button| button.rounded(px(shape.radius_button)))
                                            .border_1()
                                            .border_color(dialog_border)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text_color)
                                            .child("Cancel")
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_new_group_open = false;
                                                this.add_members_for = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_xl()
                                            .when(shape.adwaita, |button| button.rounded(px(shape.radius_button)))
                                            .bg(primary_color)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(match (adding, self.is_creating_group) {
                                                (true, true) => "Adding...",
                                                (true, false) => "Add",
                                                (false, true) => "Creating...",
                                                (false, false) => "Create Group",
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                if this.add_members_for.is_some() {
                                                    this.submit_add_members(cx);
                                                } else {
                                                    this.submit_create_group(cx);
                                                }
                                            })),
                                    ),
                            ),
                    ),
                    "new-group-in",
                ))
        } else {
            None
        }
    }
}
