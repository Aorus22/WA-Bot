#[derive(Clone, Debug)]
pub struct EmojiCategory {
    pub name: &'static str,
    pub emojis: &'static [&'static str],
}

pub static EMOJI_CATEGORIES: &[EmojiCategory] = &[
    EmojiCategory {
        name: "Smileys",
        emojis: &[
            "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🤣", "😊", "😇", "🙂", "🙃", "😉", "😌", "😍", "🥰",
            "😘", "😗", "😙", "😚", "😋", "😛", "😜", "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🤐", "🤨",
            "😐", "😑", "😶", "😏", "😒", "🙄", "😬", "🤥", "😔", "😪", "🤤", "😴", "😷", "🤒", "🤕", "🤢",
        ],
    },
    EmojiCategory {
        name: "Gestures",
        emojis: &[
            "👍", "👎", "👏", "🙌", "👐", "🤲", "🤝", "🙏", "✌️", "🤞", "🤟", "🤘", "👌", "🤌", "🤏", "👈",
            "👉", "👆", "👇", "✋", "🤚", "🖐️", "🖖", "👋", "🤙", "💪", "🦾", "🖕", "✍️", "🤳",
        ],
    },
    EmojiCategory {
        name: "Hearts & Sparkles",
        emojis: &[
            "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞", "💓", "💗", "💖",
            "💘", "💝", "💟", "🔥", "✨", "⭐", "🌟", "💫", "💥", "🎉", "🎊", "💯",
        ],
    },
];

#[derive(Clone, Default)]
pub struct EmojiPickerState {
    pub is_open: bool,
    pub selected_category_idx: usize,
    pub filter_query: String,
}

impl EmojiPickerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn current_emojis(&self) -> Vec<&'static str> {
        if let Some(cat) = EMOJI_CATEGORIES.get(self.selected_category_idx) {
            cat.emojis.to_vec()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_picker_categories() {
        let mut picker = EmojiPickerState::new();
        assert!(!picker.is_open);
        picker.toggle();
        assert!(picker.is_open);

        let emojis = picker.current_emojis();
        assert!(!emojis.is_empty());
        assert_eq!(emojis[0], "😀");

        picker.selected_category_idx = 1;
        let gestures = picker.current_emojis();
        assert_eq!(gestures[0], "👍");
    }
}
