use crate::components::rich_bubbles::{ContactHelper, LocationHelper, PollHelper};
use crate::views::chat::*;
use wabot_backend_client::dto::{ContactMeta, LocationMeta, PollMeta};

const CARD_WIDTH: f32 = 260.0;

/// A small outlined action button used at the bottom of the cards.
fn card_button(label: &str, icon: &'static [u8], color: Hsla) -> Div {
    h_flex()
        .flex_1()
        .justify_center()
        .items_center()
        .gap_1p5()
        .py_2()
        .rounded_lg()
        .cursor_pointer()
        .bg(color.opacity(0.12))
        .hover(move |s| s.bg(color.opacity(0.22)))
        .child(svg().data(icon).size(px(14.0)).text_color(color))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(label.to_string()),
        )
}

impl ChatView {
    /// Poll bubble, like the web's: the question, one tappable row per option
    /// with its vote count behind a tally bar, and the voter total.
    pub(crate) fn render_poll_card(
        chat_id: &str,
        msg_id: &str,
        poll: &PollMeta,
        primary_color: Hsla,
        bubble_text: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (counts, total) = PollHelper::compute_tallies(poll);
        let mine: Vec<String> = poll
            .votes
            .as_ref()
            .and_then(|votes| votes.get("me").cloned())
            .unwrap_or_default();

        let options = poll.options.iter().map(|option| {
            let count = counts.get(&option.name).copied().unwrap_or(0);
            let percent = PollHelper::option_percentage(count, total);
            let voted = mine.contains(&option.name);
            let next = PollHelper::toggle_vote(&mine, &option.name, poll.multi_select);
            let (chat_id, msg_id) = (chat_id.to_string(), msg_id.to_string());
            div()
                .id(format!("poll-{msg_id}-{}", option.name))
                .relative()
                .overflow_hidden()
                .rounded_lg()
                .border_1()
                .border_color(if voted { primary_color } else { Hsla::from(bubble_text).opacity(0.15) })
                .cursor_pointer()
                .hover(move |s| s.border_color(primary_color.opacity(0.6)))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(relative(percent as f32 / 100.0))
                        .bg(primary_color.opacity(0.18)),
                )
                .child(
                    h_flex()
                        .relative()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .px_2p5()
                        .py_1p5()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .when(voted, |d| d.font_weight(FontWeight::BOLD).text_color(primary_color))
                                .child(format!("{} {}", if voted { "●" } else { "○" }, option.name)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .opacity(0.6)
                                .child(count.to_string()),
                        ),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    this.vote_poll(chat_id.clone(), msg_id.clone(), next.clone(), cx);
                }))
        });

        v_flex()
            .w(px(CARD_WIDTH))
            .max_w_full()
            .gap_2()
            .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).child(poll.question.clone()))
            .child(v_flex().gap_1p5().children(options))
            .child(
                div().text_size(px(11.0)).opacity(0.6).child(format!(
                    "{total} voter{}{}",
                    if total == 1 { "" } else { "s" },
                    if poll.multi_select { " · select multiple" } else { "" },
                )),
            )
            .into_any_element()
    }

    /// Location bubble: optional map thumbnail, the place and its coordinates,
    /// and an Open in Maps action.
    pub(crate) fn render_location_card(
        msg_id: &str,
        location: &LocationMeta,
        base_url: &str,
        primary_color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let live = location.live.unwrap_or(false);
        let title = location
            .name
            .as_deref()
            .filter(|n| !n.trim().is_empty())
            .or(location.address.as_deref().filter(|a| !a.trim().is_empty()))
            .map(str::to_string)
            .unwrap_or_else(|| if live { "Live location".into() } else { "Location".into() });
        let maps_url = LocationHelper::google_maps_url(location.latitude, location.longitude);

        v_flex()
            .w(px(CARD_WIDTH))
            .max_w_full()
            .gap_2()
            .children(location.thumbnail_url.as_ref().map(|thumb| {
                img(resolve_media_url(thumb, base_url))
                    .id(format!("location-thumb-{msg_id}"))
                    .w_full()
                    .h(px(120.0))
                    .rounded_lg()
                    .object_fit(ObjectFit::Cover)
            }))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(36.0))
                            .h(px(36.0))
                            .flex_none()
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(primary_color.opacity(0.12))
                            .child(svg().data(MAP_PIN_SVG).size(px(18.0)).text_color(primary_color)),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(title),
                            )
                            .child(div().text_size(px(11.0)).opacity(0.6).child(format!(
                                "{:.5}, {:.5}{}",
                                location.latitude,
                                location.longitude,
                                if live { " · live" } else { "" },
                            ))),
                    ),
            )
            .child(
                card_button("Open in Maps", EXTERNAL_LINK_SVG, primary_color)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, _, cx| {
                        cx.open_url(&maps_url);
                    })),
            )
            .into_any_element()
    }

    /// Shared contact bubble: one row per card with its name and number, and
    /// a Message action that opens a chat with them.
    pub(crate) fn render_contact_card(
        msg_id: &str,
        contact: &ContactMeta,
        primary_color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entries: Vec<(String, Option<String>)> = if contact.contacts.is_empty() {
            vec![(contact.display_name.clone().unwrap_or_else(|| "Contact".into()), None)]
        } else {
            contact
                .contacts
                .iter()
                .map(|c| (c.display_name.clone(), c.vcard.as_deref().and_then(ContactHelper::phone_of)))
                .collect()
        };

        v_flex()
            .w(px(CARD_WIDTH))
            .max_w_full()
            .gap_2()
            .children(entries.into_iter().enumerate().map(|(index, (name, phone))| {
                let jid = phone
                    .as_deref()
                    .map(|p| p.chars().filter(char::is_ascii_digit).collect::<String>())
                    .filter(|digits| digits.len() >= 6)
                    .map(|digits| format!("{digits}@s.whatsapp.net"));
                let chat_name = name.clone();
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(40.0))
                                    .h(px(40.0))
                                    .flex_none()
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(primary_color.opacity(0.12))
                                    .child(svg().data(USER_SVG).size(px(20.0)).text_color(primary_color)),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::BOLD)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(name),
                                    )
                                    .children(phone.map(|phone| {
                                        div().text_size(px(11.0)).opacity(0.6).child(phone)
                                    })),
                            ),
                    )
                    .children(jid.map(|jid| {
                        card_button("Message", MESSAGE_SQUARE_SVG, primary_color)
                            .id(format!("contact-{msg_id}-{index}"))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                this.open_direct_chat(&jid, &chat_name, window, cx);
                            }))
                    }))
            }))
            .into_any_element()
    }
}
