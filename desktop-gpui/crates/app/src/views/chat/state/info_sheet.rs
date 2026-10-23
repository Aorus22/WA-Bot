use wabot_backend_client::dto::{GroupCache, GroupParticipantInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoSheetTab {
    Media,
    Docs,
    Links,
    GroupSettings,
}

#[derive(Clone, Debug)]
pub struct ChatInfoSheetState {
    pub is_open: bool,
    pub active_tab: InfoSheetTab,
    pub group_info: Option<GroupCache>,
    pub invite_link: Option<String>,
}

impl Default for ChatInfoSheetState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: InfoSheetTab::Media,
            group_info: None,
            invite_link: None,
        }
    }
}

impl ChatInfoSheetState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, is_group: bool) {
        self.is_open = true;
        self.active_tab = if is_group {
            InfoSheetTab::GroupSettings
        } else {
            InfoSheetTab::Media
        };
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.group_info = None;
        self.invite_link = None;
    }

    pub fn set_tab(&mut self, tab: InfoSheetTab) {
        self.active_tab = tab;
    }

    pub fn is_admin(&self) -> bool {
        if let Some(info) = &self.group_info {
            info.own_role == "admin" || info.own_role == "superadmin"
        } else {
            false
        }
    }

    pub fn participants(&self) -> &[GroupParticipantInfo] {
        if let Some(info) = &self.group_info {
            &info.participants
        } else {
            &[]
        }
    }

    pub fn promote_participant(&mut self, jid: &str) {
        if let Some(info) = &mut self.group_info {
            if let Some(p) = info.participants.iter_mut().find(|p| p.jid == jid) {
                p.is_admin = true;
            }
        }
    }

    pub fn demote_participant(&mut self, jid: &str) {
        if let Some(info) = &mut self.group_info {
            if let Some(p) = info.participants.iter_mut().find(|p| p.jid == jid) {
                p.is_admin = false;
                p.is_super_admin = Some(false);
            }
        }
    }

    pub fn remove_participant(&mut self, jid: &str) {
        if let Some(info) = &mut self.group_info {
            info.participants.retain(|p| p.jid != jid);
            info.participant_count = info.participants.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_info_sheet_admin_permissions() {
        let mut sheet = ChatInfoSheetState::new();
        assert!(!sheet.is_admin());

        sheet.group_info = Some(GroupCache {
            jid: "group-1@g.us".to_string(),
            name: "Dev Team".to_string(),
            description: Some("Engineering discussions".to_string()),
            owner: Some("owner@s.whatsapp.net".to_string()),
            locked: false,
            announce: false,
            join_approval: false,
            member_add_mode: Some("all_member_add".to_string()),
            own_role: "admin".to_string(),
            participant_count: 2,
            participants: vec![
                GroupParticipantInfo {
                    jid: "alice@s.whatsapp.net".to_string(),
                    name: Some("Alice".to_string()),
                    is_admin: false,
                    is_super_admin: None,
                },
                GroupParticipantInfo {
                    jid: "bob@s.whatsapp.net".to_string(),
                    name: Some("Bob".to_string()),
                    is_admin: true,
                    is_super_admin: None,
                },
            ],
            updated_at: 1700000000,
        });

        assert!(sheet.is_admin());
        assert_eq!(sheet.participants().len(), 2);

        // Promote Alice
        sheet.promote_participant("alice@s.whatsapp.net");
        assert!(sheet.participants()[0].is_admin);

        // Demote Alice
        sheet.demote_participant("alice@s.whatsapp.net");
        assert!(!sheet.participants()[0].is_admin);

        // Remove Bob
        sheet.remove_participant("bob@s.whatsapp.net");
        assert_eq!(sheet.participants().len(), 1);
    }
}
