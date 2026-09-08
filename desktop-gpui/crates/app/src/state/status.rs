use wabot_backend_client::dto::{StatusEntry, StatusGroup};

#[derive(Clone, Debug, Default)]
pub struct StatusStore {
    pub groups: Vec<StatusGroup>,
}

impl StatusStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_groups(&mut self, groups: Vec<StatusGroup>) {
        self.groups = groups;
    }

    pub fn add_status_entry(&mut self, entry: StatusEntry) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.sender == entry.sender) {
            group.latest_time = entry.timestamp.max(group.latest_time);
            if !entry.viewed {
                group.all_viewed = false;
            }
            group.statuses.push(entry);
        } else {
            let sender = entry.sender.clone();
            let name = entry.sender_name.clone();
            let latest_time = entry.timestamp;
            let all_viewed = entry.viewed;
            self.groups.push(StatusGroup {
                sender,
                name,
                avatar: None,
                all_viewed,
                statuses: vec![entry],
                latest_time,
            });
        }
    }

    pub fn sorted_groups(&self) -> Vec<&StatusGroup> {
        let mut sorted: Vec<&StatusGroup> = self.groups.iter().collect();
        // Unviewed first, then by latest_time descending
        sorted.sort_by(|a, b| {
            match (a.all_viewed, b.all_viewed) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => b.latest_time.cmp(&a.latest_time),
            }
        });
        sorted
    }

    pub fn mark_as_viewed(&mut self, sender: &str, status_id: &str) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.sender == sender) {
            for entry in &mut group.statuses {
                if entry.id == status_id {
                    entry.viewed = true;
                }
            }
            group.all_viewed = group.statuses.iter().all(|s| s.viewed);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StatusViewerState {
    pub is_open: bool,
    pub active_group_sender: Option<String>,
    pub current_index: usize,
    pub auto_advance_ms: u64,
}

impl StatusViewerState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_group_sender: None,
            current_index: 0,
            auto_advance_ms: 5000,
        }
    }

    pub fn open(&mut self, sender: &str, start_index: usize) {
        self.is_open = true;
        self.active_group_sender = Some(sender.to_string());
        self.current_index = start_index;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.active_group_sender = None;
        self.current_index = 0;
    }

    pub fn next(&mut self, total: usize) -> bool {
        if self.current_index + 1 < total {
            self.current_index += 1;
            true
        } else {
            self.close();
            false
        }
    }

    pub fn prev(&mut self) -> bool {
        if self.current_index > 0 {
            self.current_index -= 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_status(id: &str, sender: &str, ts: i64, viewed: bool) -> StatusEntry {
        StatusEntry {
            id: id.to_string(),
            sender: sender.to_string(),
            sender_name: Some(sender.to_string()),
            content: Some("Status content".to_string()),
            media_url: None,
            status_type: "text".to_string(),
            timestamp: ts,
            expires_at: ts + 86400,
            viewed,
        }
    }

    #[test]
    fn test_status_grouping_and_sorting() {
        let mut store = StatusStore::new();
        // Alice has viewed status
        let s_alice = make_status("s1", "alice", 100, true);
        store.add_status_entry(s_alice);

        // Bob has unviewed status with earlier timestamp
        let s_bob = make_status("s2", "bob", 50, false);
        store.add_status_entry(s_bob);

        let sorted = store.sorted_groups();
        assert_eq!(sorted.len(), 2);
        // Bob must come first because Bob is unviewed!
        assert_eq!(sorted[0].sender, "bob");
        assert_eq!(sorted[1].sender, "alice");

        // Mark Bob as viewed
        store.mark_as_viewed("bob", "s2");
        let sorted_after = store.sorted_groups();
        // Now Alice has higher latest_time (100 vs 50)
        assert_eq!(sorted_after[0].sender, "alice");
        assert_eq!(sorted_after[1].sender, "bob");
    }

    #[test]
    fn test_status_viewer_navigation() {
        let mut viewer = StatusViewerState::new();
        viewer.open("alice", 0);
        assert!(viewer.is_open);
        assert_eq!(viewer.current_index, 0);

        // Advance next (total 3 stories)
        assert!(viewer.next(3));
        assert_eq!(viewer.current_index, 1);

        assert!(viewer.next(3));
        assert_eq!(viewer.current_index, 2);

        // Reaching end closes viewer
        assert!(!viewer.next(3));
        assert!(!viewer.is_open);
    }
}
