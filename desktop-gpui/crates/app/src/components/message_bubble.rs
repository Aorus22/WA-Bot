use wabot_backend_client::dto::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageTicks {
    None,
    Sent,
    Delivered,
    Read,
    /// Delivery failed (local pending attachment rejected by the backend).
    Failed,
}

pub struct MessageBubbleHelper;

impl MessageBubbleHelper {
    pub fn is_from_me(msg: &Message, current_user: &str) -> bool {
        msg.from == current_user
            || msg.id.starts_with("temp-")
            || msg.status == "pending"
    }

    pub fn ticks(msg: &Message, is_from_me: bool) -> MessageTicks {
        if !is_from_me {
            return MessageTicks::None;
        }
        match msg.status.as_str() {
            "read" => MessageTicks::Read,
            "delivered" => MessageTicks::Delivered,
            "sent" | "pending" => MessageTicks::Sent,
            "failed" => MessageTicks::Failed,
            _ => MessageTicks::Sent,
        }
    }

    pub fn ticks_display(ticks: MessageTicks) -> &'static str {
        match ticks {
            MessageTicks::None => "",
            MessageTicks::Sent => "✓",
            MessageTicks::Delivered => "✓✓",
            MessageTicks::Read => "✓✓",
            MessageTicks::Failed => "!",
        }
    }

    pub fn format_time(ts: i64) -> String {
        if ts == 0 {
            return String::new();
        }
        let ts_sec = if ts > 10_000_000_000 { ts / 1000 } else { ts };
        let hours = (ts_sec / 3600) % 24;
        let mins = (ts_sec / 60) % 60;
        format!("{:02}:{:02}", hours, mins)
    }

    /// Day bucket (days since the Unix epoch) of a second/millisecond
    /// timestamp, used to decide where a date separator belongs.
    pub fn day_index(timestamp: i64) -> i64 {
        let ts_sec = if timestamp > 10_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        };
        ts_sec.div_euclid(86_400)
    }

    /// Web's `formatDate`: "Today", "Yesterday" or "7 September 2026".
    pub fn format_date_label(timestamp: i64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self::format_date_label_at(timestamp, now)
    }

    /// Same as [`Self::format_date_label`] with an injectable "now" for tests.
    pub fn format_date_label_at(timestamp: i64, now: i64) -> String {
        let days = Self::day_index(timestamp);
        let today = Self::day_index(now);
        if days == today {
            return "Today".to_string();
        }
        if days == today - 1 {
            return "Yesterday".to_string();
        }
        let (year, month, day) = Self::civil_from_days(days);
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        format!("{day} {} {year}", MONTHS[(month - 1) as usize])
    }

    /// Days since the Unix epoch → (year, month, day).
    /// Howard Hinnant's `civil_from_days` algorithm.
    fn civil_from_days(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (y + if m <= 2 { 1 } else { 0 }, m, d)
    }

    /// First message index of the day group that contains `msg_idx`.
    pub fn day_group_start(timestamps: &[i64], msg_idx: usize) -> usize {
        if timestamps.is_empty() {
            return 0;
        }
        let mut start = msg_idx.min(timestamps.len() - 1);
        while start > 0 && Self::day_index(timestamps[start]) == Self::day_index(timestamps[start - 1]) {
            start -= 1;
        }
        start
    }

    /// Index of the in-flow day pill heading the group containing `msg_idx`:
    /// list item 0 for the leading pill, otherwise one past the group's first
    /// message (the pill renders above its first message).
    pub fn day_group_pill_index(timestamps: &[i64], msg_idx: usize) -> usize {
        let start = Self::day_group_start(timestamps, msg_idx);
        if start == 0 {
            0
        } else {
            start + 1
        }
    }

    /// Index of the in-flow day pill heading the first group *after* the one
    /// containing `msg_idx`, if any.
    pub fn next_day_group_pill_index(timestamps: &[i64], msg_idx: usize) -> Option<usize> {
        let start = Self::day_group_start(timestamps, msg_idx);
        let day = Self::day_index(*timestamps.get(start)?);
        (start + 1..timestamps.len())
            .find(|&i| Self::day_index(timestamps[i]) != day)
            .map(|i| i + 1)
    }

    pub fn is_markdown(content: &str) -> bool {
        let t = content.trim();
        t.starts_with("{{md:") && t.ends_with("}}")
    }

    pub fn decode_content(content: &str) -> String {
        let t = content.trim();
        if t.starts_with("{{md:") && t.ends_with("}}") {
            let inner = &t[5..t.len() - 2];
            // Simple base64 decoding helper
            if let Ok(decoded) = Self::simple_base64_decode(inner) {
                if let Ok(s) = String::from_utf8(decoded) {
                    return s;
                }
            }
            inner.to_string()
        } else {
            content.to_string()
        }
    }

    fn simple_base64_decode(input: &str) -> Result<Vec<u8>, ()> {
        const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut buf: u32 = 0;
        let mut bits: u32 = 0;
        let mut out = Vec::new();

        for &b in input.as_bytes() {
            if b == b'=' {
                break;
            }
            let val = match B64_TABLE.iter().position(|&c| c == b) {
                Some(p) => p as u32,
                None => continue,
            };
            buf = (buf << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buf >> bits) & 0xFF) as u8);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(id: &str, from: &str, status: &str, content: &str) -> Message {
        Message {
            id: id.to_string(),
            chat_id: "c1".to_string(),
            from: from.to_string(),
            to: "peer".to_string(),
            content: content.to_string(),
            timestamp: 1700000000,
            status: status.to_string(),
            message_type: "text".to_string(),
            media_url: None,
            is_automatic: None,
            sender_name: None,
            reply_to_id: None,
            forwarded: None,
            reactions: None,
            extra: None,
        }
    }

    #[test]
    fn test_ticks_resolution() {
        let m_pending = make_msg("temp-1", "user", "pending", "hi");
        assert_eq!(MessageBubbleHelper::ticks(&m_pending, true), MessageTicks::Sent);

        let m_delivered = make_msg("m1", "user", "delivered", "hi");
        assert_eq!(MessageBubbleHelper::ticks(&m_delivered, true), MessageTicks::Delivered);
        assert_eq!(MessageBubbleHelper::ticks_display(MessageTicks::Delivered), "✓✓");

        let m_read = make_msg("m2", "user", "read", "hi");
        assert_eq!(MessageBubbleHelper::ticks(&m_read, true), MessageTicks::Read);

        // Incoming message should have None ticks
        assert_eq!(MessageBubbleHelper::ticks(&m_read, false), MessageTicks::None);
    }

    #[test]
    fn test_day_index_buckets_messages_by_utc_day() {
        // 2024-01-01T00:00:00Z
        let midnight = 1_704_067_200i64;
        assert_eq!(MessageBubbleHelper::day_index(midnight), 19_723);
        assert_eq!(MessageBubbleHelper::day_index(midnight + 86_399), 19_723);
        assert_eq!(MessageBubbleHelper::day_index(midnight + 86_400), 19_724);
        // Millisecond timestamps take the same bucket as their second form.
        assert_eq!(MessageBubbleHelper::day_index(midnight * 1000 + 500), 19_723);
    }

    #[test]
    fn test_format_date_label_matches_web_strings() {
        let today = 1_704_067_200_000i64; // 2024-01-01 (ms)
        let yesterday = today - 86_400_000;
        let older = 1_701_000_000_000i64; // 2023-11-26

        assert_eq!(MessageBubbleHelper::format_date_label_at(today + 3_600_000, today), "Today");
        assert_eq!(MessageBubbleHelper::format_date_label_at(yesterday, today), "Yesterday");
        assert_eq!(MessageBubbleHelper::format_date_label_at(older, today), "26 November 2023");
        // Far-future/older buckets use the day-month-year form too.
        assert_eq!(MessageBubbleHelper::format_date_label_at(today, today - 86_400_000 * 400), "1 January 2024");
    }

    #[test]
    fn test_day_group_pill_indices_follow_day_boundaries() {
        // Days 10, 10, 11, 11, 12.
        let d = 86_400i64;
        let t = vec![10 * d, 10 * d + 60, 11 * d, 11 * d + 60, 12 * d];

        assert_eq!(MessageBubbleHelper::day_group_start(&t, 0), 0);
        assert_eq!(MessageBubbleHelper::day_group_start(&t, 3), 2);
        assert_eq!(MessageBubbleHelper::day_group_start(&t, 4), 4);

        // Pills: leading pill is list item 0, then message `i` is item `i + 1`.
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&t, 0), 0);
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&t, 1), 0);
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&t, 2), 3);
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&t, 4), 5);

        assert_eq!(MessageBubbleHelper::next_day_group_pill_index(&t, 0), Some(3));
        assert_eq!(MessageBubbleHelper::next_day_group_pill_index(&t, 3), Some(5));
        assert_eq!(MessageBubbleHelper::next_day_group_pill_index(&t, 4), None);

        // Single-day and empty lists degrade gracefully.
        assert_eq!(MessageBubbleHelper::next_day_group_pill_index(&t[0..2], 0), None);
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&[], 0), 0);
        assert_eq!(MessageBubbleHelper::next_day_group_pill_index(&[], 0), None);
        assert_eq!(MessageBubbleHelper::day_group_pill_index(&t, 99), 5);
    }

    #[test]
    fn test_markdown_wrapper_detection_and_decode() {
        assert!(!MessageBubbleHelper::is_markdown("Hello world"));
        // "SGVsbG8=" is "Hello" in base64
        let md_wrapped = "{{md:SGVsbG8=}}";
        assert!(MessageBubbleHelper::is_markdown(md_wrapped));
        assert_eq!(MessageBubbleHelper::decode_content(md_wrapped), "Hello");
    }
}
