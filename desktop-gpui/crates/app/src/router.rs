//! Router and navigation state for WA Bot desktop.

use gpui::{App, Global};

/// App routes matching web client navigation 1:1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AppRoute {
    #[default]
    Chat,
    ChatDetail(String),
    Status,
    Channels,
    Calls,
    Triggers,
    TriggerDetail(String),
    TriggerNew,
    Cron,
    CronDetail(String),
    CronNew,
    Webhooks,
    WebhookDetail(String),
    WebhookNew,
    WebhookLogs,
    Documentation,
    Settings,
}

impl AppRoute {
    pub fn path_prefix(&self) -> &'static str {
        match self {
            Self::Chat | Self::ChatDetail(_) => "/chat",
            Self::Status => "/status",
            Self::Channels => "/channels",
            Self::Calls => "/calls",
            Self::Triggers | Self::TriggerDetail(_) | Self::TriggerNew => "/triggers",
            Self::Cron | Self::CronDetail(_) | Self::CronNew => "/cron",
            Self::Webhooks | Self::WebhookDetail(_) | Self::WebhookNew | Self::WebhookLogs => "/webhooks",
            Self::Documentation => "/documentation",
            Self::Settings => "/settings",
        }
    }

    pub fn is_active(&self, prefix: &str) -> bool {
        self.path_prefix() == prefix
    }
}

/// Global router state.
#[derive(Debug, Default)]
pub struct Router {
    pub current_route: AppRoute,
    pub history: Vec<AppRoute>,
}

impl Global for Router {}

impl Router {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn navigate(&mut self, route: AppRoute) {
        if self.current_route != route {
            self.history.push(self.current_route.clone());
            self.current_route = route;
        }
    }

    pub fn back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.current_route = prev;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_prefixes() {
        assert_eq!(AppRoute::Chat.path_prefix(), "/chat");
        assert_eq!(AppRoute::ChatDetail("123".into()).path_prefix(), "/chat");
        assert!(AppRoute::Status.is_active("/status"));
        assert!(AppRoute::Settings.is_active("/settings"));
    }
}
