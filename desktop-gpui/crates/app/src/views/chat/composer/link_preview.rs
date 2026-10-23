use crate::views::chat::*;

/// How long typing must pause before a link is looked up.
const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(600);

/// The first http(s) link in `text`, without the punctuation that trails it in prose.
pub(crate) fn first_url(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches(['(', '<', '"', '\'']);
        let lower = token.to_ascii_lowercase();
        let scheme = if lower.starts_with("https://") {
            8
        } else if lower.starts_with("http://") {
            7
        } else {
            return None;
        };
        let url = token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'']);
        let host = url.get(scheme..)?.split(['/', '?', '#']).next().unwrap_or("");
        let name = host.split(':').next().unwrap_or("");
        // "https://exa" mid-typing is not a site yet.
        (name.contains('.') && !name.ends_with('.') || name == "localhost").then(|| url.to_string())
    })
}

/// "example.com" from "https://www.example.com/a?b".
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

impl ChatView {
    /// Forget the card, anything dismissed and any lookup still waiting.
    pub(crate) fn reset_compose_link(&mut self) {
        self.compose_link = None;
        self.dismissed_link_previews.clear();
        self.link_preview_seq = self.link_preview_seq.wrapping_add(1);
    }

    /// The composer text changed: show, hide or look up the card for its link.
    pub(crate) fn on_compose_text_changed(&mut self, cx: &mut Context<Self>) {
        if self.compose_text.trim().is_empty() {
            self.reset_compose_link();
            return;
        }
        // An edit goes out as an edit, which carries no card.
        let url = if self.editing_message.is_some() { None } else { first_url(&self.compose_text) };
        let Some(url) = url else {
            self.compose_link = None;
            self.link_preview_seq = self.link_preview_seq.wrapping_add(1);
            return;
        };
        if self.compose_link.as_ref().is_some_and(|link| link.url == url) {
            return;
        }
        self.compose_link = None;
        self.link_preview_seq = self.link_preview_seq.wrapping_add(1);
        if self.dismissed_link_previews.contains(&url) {
            return;
        }
        if let Some(cached) = self.link_preview_cache.get(&url) {
            self.compose_link = Self::link_card(&url, cached.as_ref());
            cx.notify();
            return;
        }

        let seq = self.link_preview_seq;
        let base_url = Self::compose_base_url(cx);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                cx_handle.background_executor().timer(DEBOUNCE).await;
                let still_current = cx_handle.update(|cx: &mut App| {
                    this.upgrade().is_some_and(|view| view.read(cx).link_preview_seq == seq)
                });
                if !still_current {
                    return;
                }
                let client = HttpClient::new(&base_url);
                let target = url.clone();
                let res = TOKIO_RT.spawn(async move { client.get_link_preview(&target).await }).await;
                // 404 means nothing to scrape: remember that. Any other failure
                // (offline, backend restarting) is not worth remembering.
                let outcome = match res {
                    Ok(Ok(preview)) => Some(Some(preview)),
                    Ok(Err(error)) if error.is_not_found() => Some(None),
                    _ => None,
                };
                let Some(outcome) = outcome else { return };
                let _ = cx_handle.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return };
                    view.update(cx, |this, cx| {
                        this.link_preview_cache.insert(url.clone(), outcome.clone());
                        if this.link_preview_seq == seq {
                            this.compose_link = Self::link_card(&url, outcome.as_ref());
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
    }

    /// The card for a fetched preview, if it has anything to show.
    fn link_card(url: &str, preview: Option<&LinkPreview>) -> Option<ComposeLinkPreview> {
        let preview = preview?;
        let has_content = !preview.title.trim().is_empty()
            || !preview.description.trim().is_empty()
            || !preview.thumbnail_url.trim().is_empty();
        has_content.then(|| ComposeLinkPreview { url: url.to_string(), preview: preview.clone() })
    }

    /// The X on the card: this link goes out as plain text.
    pub(crate) fn dismiss_link_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(link) = self.compose_link.take() {
            self.dismissed_link_previews.insert(link.url);
        }
        cx.notify();
    }

    /// Card above the input for the link being typed: thumbnail, title,
    /// description and site, with an X to leave the preview off.
    pub(crate) fn render_compose_link_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let link = self.compose_link.as_ref()?;
        let theme = *cx.app_theme();
        let base_url = Self::compose_base_url(cx);
        let preview = &link.preview;
        let thumbnail = (!preview.thumbnail_url.trim().is_empty())
            .then(|| resolve_media_url(&preview.thumbnail_url, &base_url));
        let title = if preview.title.trim().is_empty() { host_of(&link.url) } else { preview.title.trim().to_string() };
        let description = preview.description.trim().to_string();

        Some(
            h_flex()
                .id("compose-link-preview")
                .mx_3()
                .mt_2p5()
                .px_3()
                .py_2()
                .gap_3()
                .bg(theme.muted.opacity(0.4))
                .border_l_4()
                .border_color(theme.primary)
                .rounded_r_xl()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap_3()
                        .items_center()
                        .children(thumbnail.map(|url| {
                            img(url)
                                .w(px(56.0))
                                .h(px(56.0))
                                .flex_shrink_0()
                                .rounded_lg()
                                .object_fit(ObjectFit::Cover)
                        }))
                        .child(
                            v_flex()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.foreground)
                                        .overflow_hidden()
                                        .line_clamp(1)
                                        .child(title),
                                )
                                .children((!description.is_empty()).then(|| {
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .overflow_hidden()
                                        .line_clamp(2)
                                        .child(description)
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground.opacity(0.7))
                                        .overflow_hidden()
                                        .line_clamp(1)
                                        .child(host_of(&link.url)),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("btn-dismiss-link-preview")
                        .cursor_pointer()
                        .p_1()
                        .rounded(px(cx.shape().radius_button))
                        .hover(|s| s.bg(theme.muted.opacity(0.6)))
                        .tooltip(move |window, cx| Tooltip::new("Send without preview").build(window, cx))
                        .child(svg().data(X_SVG).size(px(14.0)).text_color(theme.muted_foreground))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.dismiss_link_preview(cx);
                        })),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn finds_the_first_link_without_trailing_punctuation() {
        assert_eq!(first_url("look https://example.com/a?b=1, nice").as_deref(), Some("https://example.com/a?b=1"));
        assert_eq!(first_url("(http://example.org).").as_deref(), Some("http://example.org"));
        assert_eq!(first_url("HTTPS://Example.com").as_deref(), Some("HTTPS://Example.com"));
        assert_eq!(first_url("no link here, example.com"), None);
    }

    #[test]
    fn a_half_typed_host_is_not_a_link_yet() {
        assert_eq!(first_url("https://exa"), None);
        assert_eq!(first_url("https://example."), None);
        assert_eq!(first_url("http://localhost:3000/x").as_deref(), Some("http://localhost:3000/x"));
        assert_eq!(first_url("https://").as_deref(), None);
    }

    #[test]
    fn host_drops_scheme_path_and_www() {
        assert_eq!(host_of("https://www.example.com/a?b"), "example.com");
        assert_eq!(host_of("http://sub.example.org:8080"), "sub.example.org:8080");
    }
}
