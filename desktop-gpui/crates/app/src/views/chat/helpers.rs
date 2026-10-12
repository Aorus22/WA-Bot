use crate::views::chat::*;

pub(crate) fn prepare_mention_text(text: &str, group: Option<&GroupCache>) -> (String, Vec<String>) {
    let Some(group) = group else { return (text.to_string(), Vec::new()) };
    let mut content = text.to_string();
    let mut mentions = Vec::new();
    let mut participants = group.participants.clone();
    participants.sort_by_key(|participant| std::cmp::Reverse(participant.name.as_deref().unwrap_or("").len()));
    for participant in participants {
        let label = participant.name.as_deref().filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| participant.jid.split('@').next().unwrap_or(&participant.jid));
        let token = format!("@{label}");
        if content.contains(&token) {
            content = content.replace(&token, &format!("@{}", participant.jid.split('@').next().unwrap_or(&participant.jid)));
            if !mentions.contains(&participant.jid) {
                mentions.push(participant.jid);
            }
        }
    }
    (content, mentions)
}

#[cfg(test)]
pub(crate) fn render_message_mentions(content: &str, message: &Message, group: Option<&GroupCache>) -> String {
    render_message_mentions_with_spans(content, message, group).0
}

/// Rewrites `@<id>` tokens to the contact's name and returns the byte ranges of
/// every mention in the rewritten text, so callers can colour them.
pub(crate) fn render_message_mentions_with_spans(
    content: &str,
    message: &Message,
    group: Option<&GroupCache>,
) -> (String, Vec<Range<usize>>) {
    let Some(extra) = message.extra.as_ref() else { return (content.to_string(), Vec::new()) };
    let mut names = std::collections::HashMap::new();
    let mut mentioned = HashSet::new();
    for jid in &extra.mentions {
        let id = jid.split('@').next().unwrap_or(jid);
        mentioned.insert(id);
        let label = extra.mention_names.get(jid).map(String::as_str)
            .filter(|name| !name.trim().is_empty())
            .or_else(|| group.and_then(|group| {
                group.participants.iter().find(|member| member.jid == *jid)
                    .and_then(|member| member.name.as_deref()).filter(|name| !name.trim().is_empty())
            }));
        if let Some(label) = label {
            names.insert(id, label);
        }
    }
    // Match complete IDs so @123 cannot rewrite the prefix of @12345.
    let mut parts = content.split('@');
    let mut rendered = parts.next().unwrap_or_default().to_owned();
    let mut spans = Vec::new();
    for part in parts {
        let end = part.find(|c: char| !c.is_ascii_digit()).unwrap_or(part.len());
        let number = &part[..end];
        let start = rendered.len();
        rendered.push('@');
        rendered.push_str(names.get(number).copied().unwrap_or(number));
        if mentioned.contains(number) {
            spans.push(start..rendered.len());
        }
        rendered.push_str(&part[end..]);
    }
    (rendered, spans)
}

#[cfg(test)]
mod mention_tests {
    use super::*;
    use core::prelude::v1::test;

    fn message(mentions: serde_json::Value, names: serde_json::Value) -> Message {
        serde_json::from_value(serde_json::json!({
            "id": "m1", "chatId": "123@g.us", "extra": {"mentions": mentions, "mentionNames": names}
        })).unwrap()
    }

    #[test]
    fn lid_mentions_show_names_before_group_info_has_loaded() {
        let m = message(serde_json::json!(["242622887161958@lid"]), serde_json::json!({"242622887161958@lid": "Nama kontak"}));
        assert_eq!(render_message_mentions("halo @242622887161958!", &m, None), "halo @Nama kontak!");
    }

    #[test]
    fn mention_replacement_matches_whole_ids_and_preserves_other_text() {
        let m = message(serde_json::json!(["123@lid"]), serde_json::json!({"123@lid": "Zoë Putri"}));
        assert_eq!(render_message_mentions("@123, @12345\n@123 🙂", &m, None), "@Zoë Putri, @12345\n@Zoë Putri 🙂");
    }

    #[test]
    fn mention_spans_cover_the_rendered_names_only() {
        let m = message(serde_json::json!(["123@lid"]), serde_json::json!({"123@lid": "Zoë"}));
        let (text, spans) = render_message_mentions_with_spans("hi @123 and @456", &m, None);
        assert_eq!(text, "hi @Zoë and @456");
        assert_eq!(spans.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(), ["@Zoë"]);
    }

    #[test]
    fn unresolved_mentions_and_plain_at_text_keep_their_content() {
        let m = message(serde_json::json!(["123@lid"]), serde_json::json!({}));
        assert_eq!(render_message_mentions("halo @123 dan @456", &m, None), "halo @123 dan @456");
        let plain = message(serde_json::json!([]), serde_json::json!({"123@lid": "Nama"}));
        assert_eq!(render_message_mentions("@123", &plain, None), "@123");
    }

    #[test]
    fn older_messages_can_resolve_names_from_the_group_cache() {
        let m = message(serde_json::json!(["123@s.whatsapp.net"]), serde_json::json!({}));
        let group: GroupCache = serde_json::from_value(serde_json::json!({
            "jid": "123@g.us", "name": "Test group", "participants": [{"jid": "123@s.whatsapp.net", "name": "Nama anggota"}]
        })).unwrap();
        assert_eq!(render_message_mentions("@123", &m, Some(&group)), "@Nama anggota");
    }
}


/// Load state of the viewer image.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PreviewLoad {
    /// Still downloading / decoding.
    Loading,
    /// Ready, with its natural pixel size.
    Ready(f32, f32),
    Failed,
}

/// Load state (and natural size) of the image at `url`. Uses the same asset
/// cache as the `img` element, so this adds no second download; gpui re-renders
/// the view when the load finishes.
pub(crate) fn preview_load_state(url: &str, window: &mut Window, cx: &mut App) -> PreviewLoad {
    let resource = if url.starts_with("http://") || url.starts_with("https://") {
        Resource::Uri(SharedUri::from(url.to_string()))
    } else {
        Resource::Path(std::path::Path::new(url).into())
    };
    match window.use_asset::<ImageAssetLoader>(&resource, cx) {
        None => PreviewLoad::Loading,
        Some(Err(_)) => PreviewLoad::Failed,
        Some(Ok(data)) => {
            let size = data.size(0);
            PreviewLoad::Ready(size.width.0 as f32, size.height.0 as f32)
        }
    }
}


/// the click handler.
/// One row of a context menu: icon + label, hover highlight. The caller adds
/// the click handler.
pub(crate) fn menu_row(
    id: &'static str,
    icon: &'static [u8],
    label: &'static str,
    icon_color: Hsla,
    text_color: Hsla,
    hover_bg: Hsla,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .cursor_pointer()
        .px_3()
        .py_2()
        .rounded_xl()
        .gap_2p5()
        .items_center()
        .hover(move |s| s.bg(hover_bg))
        .child(svg().data(icon).size(px(15.0)).text_color(icon_color))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_color)
                .child(label),
        )
}

/// Conversation sidebar sizing: the desktop opens at 360 like before, and


pub(crate) fn avatar_color_for(id: &str) -> Hsla {
    let mut sum: usize = 0;
    for b in id.bytes() {
        sum = sum.wrapping_add(b as usize);
    }
    let hex = AVATAR_COLORS[sum % AVATAR_COLORS.len()];
    rgb(hex).into()
}

pub(crate) fn urlencode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
}

pub(crate) fn resolve_avatar_url(avatar: &str, chat_id: &str, base_url: &str) -> String {
    let avatar = avatar.trim();
    if !avatar.is_empty() && !avatar.starts_with("data:") {
        if avatar.starts_with("http://") || avatar.starts_with("https://") {
            return avatar.to_string();
        }
        let clean_base = base_url.trim_end_matches('/');
        let clean_avatar = avatar.trim_start_matches('/');
        if clean_avatar.starts_with("api/") {
            let root = clean_base.strip_suffix("/api").unwrap_or(clean_base);
            return format!("{root}/{clean_avatar}");
        }
        return format!("{clean_base}/{clean_avatar}");
    }
    let clean_base = base_url.trim_end_matches('/');
    format!("{clean_base}/avatar/{}", urlencode(chat_id))
}

pub(crate) fn resolve_media_url(media_url: &str, base_url: &str) -> String {
    let media_url = media_url.trim();
    if media_url.starts_with("http://") || media_url.starts_with("https://") {
        return media_url.to_string();
    }
    let clean_base = base_url.trim_end_matches('/');
    let clean_url = media_url.trim_start_matches('/');
    if clean_url.starts_with("api/") {
        let root = clean_base.strip_suffix("/api").unwrap_or(clean_base);
        format!("{root}/{clean_url}")
    } else {
        format!("{clean_base}/{clean_url}")
    }
}

pub(crate) fn render_avatar(
    chat_id: &str,
    chat_name: &str,
    avatar_field: &str,
    is_group: bool,
    size_px: f32,
    base_url: &str,
    theme: &ActiveTokens,
) -> AnyElement {
    let initial = chat_name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let avatar_url = resolve_avatar_url(avatar_field, chat_id, base_url);
    let color = avatar_color_for(chat_id);

    let fallback_init = initial.clone();
    let fallback_theme = *theme;
    let fallback_color = color;
    let fallback_group = is_group;
    let fallback_size = size_px;

    let make_fallback = move || -> AnyElement {
        if fallback_group {
            div()
                .w(px(fallback_size))
                .h(px(fallback_size))
                .rounded_full()
                .bg(fallback_theme.primary.opacity(0.15))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(svg().data(USERS_SVG).size(px(fallback_size * 0.46)).text_color(fallback_theme.primary))
                .into_any_element()
        } else {
            div()
                .w(px(fallback_size))
                .h(px(fallback_size))
                .rounded_full()
                .bg(fallback_color.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_size(px(fallback_size * 0.38))
                .font_weight(FontWeight::BOLD)
                .text_color(fallback_color)
                .child(fallback_init.clone())
                .into_any_element()
        }
    };

    let loading_init = initial.clone();
    let loading_theme = *theme;
    let loading_color = color;
    let loading_group = is_group;
    let loading_size = size_px;

    let make_loading = move || -> AnyElement {
        if loading_group {
            div()
                .w(px(loading_size))
                .h(px(loading_size))
                .rounded_full()
                .bg(loading_theme.primary.opacity(0.15))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(svg().data(USERS_SVG).size(px(loading_size * 0.46)).text_color(loading_theme.primary))
                .into_any_element()
        } else {
            div()
                .w(px(loading_size))
                .h(px(loading_size))
                .rounded_full()
                .bg(loading_color.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .text_size(px(loading_size * 0.38))
                .font_weight(FontWeight::BOLD)
                .text_color(loading_color)
                .child(loading_init.clone())
                .into_any_element()
        }
    };

    img(avatar_url)
        .w(px(size_px))
        .h(px(size_px))
        .rounded_full()
        .flex_shrink_0()
        .object_fit(ObjectFit::Cover)
        .with_fallback(make_fallback)
        .with_loading(make_loading)
        .into_any_element()
}
