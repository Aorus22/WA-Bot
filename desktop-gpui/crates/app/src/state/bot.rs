use wabot_backend_client::dto::{CronJob, Trigger, Webhook, WebhookLog};

#[derive(Clone, Debug, Default)]
pub struct BotStore {
    pub triggers: Vec<Trigger>,
    pub crons: Vec<CronJob>,
    pub webhooks: Vec<Webhook>,
    pub webhook_logs: Vec<WebhookLog>,
}

impl BotStore {
    pub fn new() -> Self {
        Self::default()
    }

    // Triggers
    pub fn set_triggers(&mut self, triggers: Vec<Trigger>) {
        self.triggers = triggers;
    }

    pub fn upsert_trigger(&mut self, trigger: Trigger) {
        if let Some(pos) = self.triggers.iter().position(|t| t.id == trigger.id) {
            self.triggers[pos] = trigger;
        } else {
            self.triggers.push(trigger);
        }
    }

    pub fn delete_trigger(&mut self, id: &str) {
        self.triggers.retain(|t| t.id != id);
    }

    pub fn delete_all_triggers(&mut self) {
        self.triggers.clear();
    }

    // Crons
    pub fn set_crons(&mut self, crons: Vec<CronJob>) {
        self.crons = crons;
    }

    pub fn upsert_cron(&mut self, cron: CronJob) {
        if let Some(pos) = self.crons.iter().position(|c| c.id == cron.id) {
            self.crons[pos] = cron;
        } else {
            self.crons.push(cron);
        }
    }

    pub fn delete_cron(&mut self, id: &str) {
        self.crons.retain(|c| c.id != id);
    }

    pub fn delete_all_crons(&mut self) {
        self.crons.clear();
    }

    // Webhooks
    pub fn set_webhooks(&mut self, webhooks: Vec<Webhook>) {
        self.webhooks = webhooks;
    }

    pub fn upsert_webhook(&mut self, webhook: Webhook) {
        if let Some(pos) = self.webhooks.iter().position(|w| w.id == webhook.id) {
            self.webhooks[pos] = webhook;
        } else {
            self.webhooks.push(webhook);
        }
    }

    pub fn delete_webhook(&mut self, id: &str) {
        self.webhooks.retain(|w| w.id != id);
    }

    pub fn delete_all_webhooks(&mut self) {
        self.webhooks.clear();
    }

    // Webhook logs
    pub fn set_logs(&mut self, logs: Vec<WebhookLog>) {
        self.webhook_logs = logs;
    }

    pub fn filtered_logs<'a>(&'a self, webhook_id: Option<&str>, status: Option<i32>) -> Vec<&'a WebhookLog> {
        self.webhook_logs
            .iter()
            .filter(|log| {
                if let Some(wid) = webhook_id {
                    log.webhook_id == wid
                } else {
                    true
                }
            })
            .filter(|log| {
                if let Some(code) = status {
                    log.status_code == code
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn clear_all_logs(&mut self) {
        self.webhook_logs.clear();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AIMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct AIAssistantState {
    pub is_open: bool,
    pub selected_model: String,
    pub messages: Vec<AIMessage>,
}

impl Default for AIAssistantState {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_model: "gemini-1.5-flash".to_string(),
            messages: Vec::new(),
        }
    }
}

impl AIAssistantState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(AIMessage {
            role: "user".to_string(),
            content: content.to_string(),
        });
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(AIMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
        });
    }

    pub fn extract_code_blocks(text: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut in_block = false;
        let mut current = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                if in_block {
                    blocks.push(current.join("\n"));
                    current.clear();
                    in_block = false;
                } else {
                    in_block = true;
                }
            } else if in_block {
                current.push(line);
            }
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_store_crud() {
        let mut store = BotStore::new();
        let t1 = Trigger {
            id: "t1".to_string(),
            name: "Auto reply".to_string(),
            pattern: "^hello".to_string(),
            script: "send('world')".to_string(),
            priority: 1,
            is_active: true,
            description: None,
            created_at: None,
            updated_at: None,
        };

        store.upsert_trigger(t1.clone());
        assert_eq!(store.triggers.len(), 1);

        // Update trigger
        let mut t1_mod = t1.clone();
        t1_mod.name = "Auto reply updated".to_string();
        store.upsert_trigger(t1_mod);
        assert_eq!(store.triggers.len(), 1);
        assert_eq!(store.triggers[0].name, "Auto reply updated");

        // Delete trigger
        store.delete_trigger("t1");
        assert!(store.triggers.is_empty());
    }

    #[test]
    fn test_ai_assistant_code_extraction() {
        let text = "Here is the trigger script you asked for:\n\n```js\nif (msg.body === 'ping') {\n    reply('pong');\n}\n```\n\nHope this helps!";
        let blocks = AIAssistantState::extract_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("reply('pong')"));
    }
}
