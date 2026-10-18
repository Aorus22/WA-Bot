# Bot route parity

Status: **matched to the current GPUI source.** Triggers, Cron Jobs, Webhooks, and Documentation render the same centered title and description as `desktop-gpui/crates/app/src/views/shell.rs`.

| GPUI route | QML page | Visible behavior | Status |
|---|---|---|---|
| Triggers | `qml/pages/bot/TriggersPage.qml` | “Bot Triggers” and “Configure automated bot keyword responses” | matched |
| Cron Jobs | `qml/pages/bot/CronPage.qml` | “Cron Jobs” and “Schedule periodic automated messages and actions” | matched |
| Webhooks | `qml/pages/bot/WebhooksPage.qml` | “Webhooks” and “Forward WhatsApp events to external services” | matched |
| Documentation | `qml/pages/bot/DocumentationPage.qml` | “Documentation” and “API documentation and guides from backend” | matched |

The current GPUI routes are placeholders. React-only admin CRUD, webhook logs, API documentation rendering, and AI assistant interactions are outside this GPUI-to-QML parity scope.
