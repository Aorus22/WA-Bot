use wabot_backend_client::dto::{Chat, HistorySyncStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarFilterMode {
    Active,
    Archived,
}

pub struct ChatSidebarComponent {
    pub search_query: String,
    pub filter_mode: SidebarFilterMode,
    pub history_sync: HistorySyncStatus,
    pub is_syncing: bool,
    pub active_chat_id: Option<String>,
}

impl ChatSidebarComponent {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            filter_mode: SidebarFilterMode::Active,
            history_sync: HistorySyncStatus {
                state: "idle".to_string(),
                pending_chats: 0,
                pending_messages: 0,
                chats_total: 0,
                chats_processed: 0,
                messages_added: 0,
                errors: Vec::new(),
                started_at: None,
                finished_at: None,
                last_run_at: None,
            },
            is_syncing: false,
            active_chat_id: None,
        }
    }

    pub fn filtered_chats<'a>(&self, all_chats: &'a [Chat]) -> Vec<&'a Chat> {
        let query = self.search_query.trim().to_lowercase();

        let mut filtered: Vec<&'a Chat> = all_chats
            .iter()
            .filter(|c| match self.filter_mode {
                SidebarFilterMode::Active => !c.archived,
                SidebarFilterMode::Archived => c.archived,
            })
            .filter(|c| {
                if query.is_empty() {
                    true
                } else {
                    c.name.to_lowercase().contains(&query)
                        || c.id.to_lowercase().contains(&query)
                        || c.last_msg.to_lowercase().contains(&query)
                }
            })
            .collect();

        // Sort: pinned first (descending pinned_at), then by last_time descending
        filtered.sort_by(|a, b| {
            match (a.pinned_at, b.pinned_at) {
                (Some(p_a), Some(p_b)) => p_b.cmp(&p_a),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.last_time.cmp(&a.last_time),
            }
        });

        filtered
    }

    pub fn archived_count(&self, all_chats: &[Chat]) -> usize {
        all_chats.iter().filter(|c| c.archived).count()
    }

    pub fn format_timestamp(ts: i64) -> String {
        if ts == 0 {
            return String::new();
        }
        let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
        let hours = (ts_sec / 3600) % 24;
        let mins = (ts_sec / 60) % 60;
        format!("{:02}:{:02}", hours, mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chat(id: &str, name: &str, last_time: i64, archived: bool, pinned_at: Option<i64>) -> Chat {
        Chat {
            id: id.to_string(),
            name: name.to_string(),
            avatar: String::new(),
            last_msg: "Hi".to_string(),
            last_time,
            unread: 0,
            is_active: false,
            is_group: false,
            archived,
            pinned_at,
            mute_mode: "off".to_string(),
            muted_until: None,
        }
    }

    #[test]
    fn test_sidebar_filtering_active_and_archived() {
        let mut sidebar = ChatSidebarComponent::new();
        let c1 = make_chat("c1", "Active Chat", 100, false, None);
        let c2 = make_chat("c2", "Archived Chat", 200, true, None);
        let all = vec![c1, c2];

        assert_eq!(sidebar.archived_count(&all), 1);

        sidebar.filter_mode = SidebarFilterMode::Active;
        let active = sidebar.filtered_chats(&all);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "c1");

        sidebar.filter_mode = SidebarFilterMode::Archived;
        let archived = sidebar.filtered_chats(&all);
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "c2");
    }

    #[test]
    fn test_sidebar_search_and_pin_sorting() {
        let mut sidebar = ChatSidebarComponent::new();
        let c1 = make_chat("c1", "Alice Bob", 500, false, None);
        let c2 = make_chat("c2", "Charlie", 100, false, Some(1000));
        let c3 = make_chat("c3", "Bob Dylan", 300, false, None);
        let all = vec![c1, c2, c3];

        // Pinned c2 must be first despite lower last_time
        let sorted = sidebar.filtered_chats(&all);
        assert_eq!(sorted[0].id, "c2");
        assert_eq!(sorted[1].id, "c1");
        assert_eq!(sorted[2].id, "c3");

        // Search query "bob" should match c1 and c3
        sidebar.search_query = "bob".to_string();
        let searched = sidebar.filtered_chats(&all);
        assert_eq!(searched.len(), 2);
        assert_eq!(searched[0].id, "c1");
        assert_eq!(searched[1].id, "c3");
    }

    #[test]
    fn test_sidebar_timestamp_format() {
        assert_eq!(ChatSidebarComponent::format_timestamp(0), "");
        // 3661 seconds = 1 hour, 1 minute, 1 second -> "01:01"
        assert_eq!(ChatSidebarComponent::format_timestamp(3661), "01:01");
    }
}

