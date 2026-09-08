

#[derive(Clone, Debug)]
pub struct AppSettingsState {
    pub has_gemini_key: bool,
    pub has_fish_key: bool,
    pub ai_server_url: String,
    pub gemini_key_input: String,
    pub tts_provider: String,
    pub tts_voice: String,
    pub fish_key_input: String,
    pub read_receipts_enabled: bool,
}

impl Default for AppSettingsState {
    fn default() -> Self {
        Self {
            has_gemini_key: false,
            has_fish_key: false,
            ai_server_url: "https://generativelanguage.googleapis.com".to_string(),
            gemini_key_input: String::new(),
            tts_provider: "browser".to_string(),
            tts_voice: "default".to_string(),
            fish_key_input: String::new(),
            read_receipts_enabled: true,
        }
    }
}

impl AppSettingsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mask_secret(secret: &str) -> String {
        if secret.is_empty() {
            String::new()
        } else if secret.len() <= 4 {
            "••••".to_string()
        } else {
            let visible_tail = &secret[secret.len() - 4..];
            format!("••••••••{}", visible_tail)
        }
    }

    pub fn toggle_read_receipts(&mut self) {
        self.read_receipts_enabled = !self.read_receipts_enabled;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocPage {
    pub slug: String,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct DocsStore {
    pub pages: Vec<DocPage>,
    pub active_slug: Option<String>,
}

impl DocsStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_pages(&mut self, pages: Vec<DocPage>) {
        self.pages = pages;
        if self.active_slug.is_none() && !self.pages.is_empty() {
            self.active_slug = Some(self.pages[0].slug.clone());
        }
    }

    pub fn active_page(&self) -> Option<&DocPage> {
        let slug = self.active_slug.as_ref()?;
        self.pages.iter().find(|p| p.slug == *slug)
    }

    pub fn select_page(&mut self, slug: &str) {
        if self.pages.iter().any(|p| p.slug == slug) {
            self.active_slug = Some(slug.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_masking() {
        assert_eq!(AppSettingsState::mask_secret(""), "");
        assert_eq!(AppSettingsState::mask_secret("123"), "••••");
        assert_eq!(AppSettingsState::mask_secret("my-gemini-secret-key-1234"), "••••••••1234");
    }

    #[test]
    fn test_docs_store_selection() {
        let mut docs = DocsStore::new();
        docs.set_pages(vec![
            DocPage {
                slug: "intro".to_string(),
                title: "Introduction".to_string(),
                content: "# WA Bot Desktop Docs\nWelcome!".to_string(),
            },
            DocPage {
                slug: "triggers".to_string(),
                title: "Triggers Guide".to_string(),
                content: "# Triggers\nHow to write scripts".to_string(),
            },
        ]);

        assert_eq!(docs.active_slug.as_deref(), Some("intro"));
        assert_eq!(docs.active_page().unwrap().title, "Introduction");

        docs.select_page("triggers");
        assert_eq!(docs.active_page().unwrap().title, "Triggers Guide");
    }
}
