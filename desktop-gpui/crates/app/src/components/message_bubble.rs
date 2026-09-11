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
    fn test_markdown_wrapper_detection_and_decode() {
        assert!(!MessageBubbleHelper::is_markdown("Hello world"));
        // "SGVsbG8=" is "Hello" in base64
        let md_wrapped = "{{md:SGVsbG8=}}";
        assert!(MessageBubbleHelper::is_markdown(md_wrapped));
        assert_eq!(MessageBubbleHelper::decode_content(md_wrapped), "Hello");
    }
}
