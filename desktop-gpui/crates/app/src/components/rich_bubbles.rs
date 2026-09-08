use std::collections::HashMap;
use wabot_backend_client::dto::{ContactMeta, LocationMeta, PollMeta, ViewOnceMeta};

pub struct PollHelper;

impl PollHelper {
    /// Computes votes count per option and total number of voters.
    pub fn compute_tallies(poll: &PollMeta) -> (HashMap<String, usize>, usize) {
        let mut counts = HashMap::new();
        let total_voters = poll.votes.as_ref().map(|v| v.len()).unwrap_or(0);

        if let Some(votes) = &poll.votes {
            for selected_options in votes.values() {
                for opt in selected_options {
                    *counts.entry(opt.clone()).or_insert(0) += 1;
                }
            }
        }

        (counts, total_voters)
    }

    /// Returns vote percentage for an option (0 to 100).
    pub fn option_percentage(count: usize, total_voters: usize) -> u32 {
        if total_voters == 0 {
            0
        } else {
            ((count as f64 / total_voters as f64) * 100.0).round() as u32
        }
    }

    /// Toggles a vote option based on multi_select mode.
    pub fn toggle_vote(current_votes: &[String], option: &str, multi_select: bool) -> Vec<String> {
        if multi_select {
            let mut next: Vec<String> = current_votes.to_vec();
            if let Some(pos) = next.iter().position(|o| o == option) {
                next.remove(pos);
            } else {
                next.push(option.to_string());
            }
            next
        } else {
            if current_votes.contains(&option.to_string()) {
                Vec::new()
            } else {
                vec![option.to_string()]
            }
        }
    }
}

pub struct LocationHelper;

impl LocationHelper {
    pub fn google_maps_url(lat: f64, lng: f64) -> String {
        format!("https://maps.google.com/?q={},{}", lat, lng)
    }

    pub fn display_label(loc: &LocationMeta) -> String {
        if let Some(name) = &loc.name {
            if !name.trim().is_empty() {
                return name.clone();
            }
        }
        format!("{:.4}, {:.4}", loc.latitude, loc.longitude)
    }
}

pub struct ContactHelper;

impl ContactHelper {
    pub fn primary_phone(contact: &ContactMeta) -> Option<String> {
        if let Some(sub) = contact.contacts.first() {
            if let Some(vcard) = &sub.vcard {
                for line in vcard.lines() {
                    let upper = line.to_uppercase();
                    if upper.starts_with("TEL") {
                        if let Some(idx) = line.find(':') {
                            return Some(line[idx + 1..].trim().to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

pub struct ViewOnceHelper;

impl ViewOnceHelper {
    pub fn is_viewed(meta: &ViewOnceMeta) -> bool {
        meta.viewed
    }

    pub fn label(meta: &ViewOnceMeta) -> &'static str {
        match meta.media_type.as_str() {
            "video" => "Video",
            _ => "Photo",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabot_backend_client::dto::{PollOption, SubContact};

    #[test]
    fn test_poll_tallies_and_toggle() {
        let mut votes = HashMap::new();
        votes.insert("voter1".to_string(), vec!["Option A".to_string()]);
        votes.insert("voter2".to_string(), vec!["Option A".to_string(), "Option B".to_string()]);

        let poll = PollMeta {
            question: "Favorite language?".to_string(),
            options: vec![
                PollOption { name: "Option A".to_string() },
                PollOption { name: "Option B".to_string() },
            ],
            multi_select: true,
            votes: Some(votes),
        };

        let (counts, total) = PollHelper::compute_tallies(&poll);
        assert_eq!(total, 2);
        assert_eq!(counts["Option A"], 2);
        assert_eq!(counts["Option B"], 1);

        assert_eq!(PollHelper::option_percentage(2, total), 100);
        assert_eq!(PollHelper::option_percentage(1, total), 50);

        // Toggle vote multi-select
        let my_votes = vec!["Option A".to_string()];
        let updated = PollHelper::toggle_vote(&my_votes, "Option B", true);
        assert_eq!(updated, vec!["Option A".to_string(), "Option B".to_string()]);

        let untoggled = PollHelper::toggle_vote(&updated, "Option A", true);
        assert_eq!(untoggled, vec!["Option B".to_string()]);

        // Toggle vote single-select
        let single = PollHelper::toggle_vote(&["Option A".to_string()], "Option B", false);
        assert_eq!(single, vec!["Option B".to_string()]);
    }

    #[test]
    fn test_location_and_maps_url() {
        let loc = LocationMeta {
            latitude: -6.2088,
            longitude: 106.8456,
            name: Some("Jakarta".to_string()),
            address: Some("Indonesia".to_string()),
            live: Some(false),
            thumbnail_url: None,
        };

        assert_eq!(LocationHelper::display_label(&loc), "Jakarta");
        assert_eq!(
            LocationHelper::google_maps_url(loc.latitude, loc.longitude),
            "https://maps.google.com/?q=-6.2088,106.8456"
        );
    }

    #[test]
    fn test_contact_vcard_phone_extraction() {
        let vcard = "BEGIN:VCARD\nVERSION:3.0\nFN:Bima\nTEL;TYPE=CELL:+628123456789\nEND:VCARD".to_string();
        let contact = ContactMeta {
            display_name: Some("Bima".to_string()),
            contacts: vec![SubContact {
                display_name: "Bima".to_string(),
                vcard: Some(vcard),
            }],
        };

        assert_eq!(ContactHelper::primary_phone(&contact), Some("+628123456789".to_string()));
    }

    #[test]
    fn test_view_once_helper() {
        let unviewed = ViewOnceMeta {
            media_type: "video".to_string(),
            viewed: false,
        };
        assert!(!ViewOnceHelper::is_viewed(&unviewed));
        assert_eq!(ViewOnceHelper::label(&unviewed), "Video");

        let viewed = ViewOnceMeta {
            media_type: "image".to_string(),
            viewed: true,
        };
        assert!(ViewOnceHelper::is_viewed(&viewed));
        assert_eq!(ViewOnceHelper::label(&viewed), "Photo");
    }
}
