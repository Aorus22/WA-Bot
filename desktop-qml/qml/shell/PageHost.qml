import QtQuick

// Router outlet. Pages are created on first visit and kept alive afterwards (state survives
// navigation, like the GPUI shell which owns one view entity per route).
// To add a page, add a row to `pages`: `section` is the first path segment, `file` is relative to qml/.
Item {
    id: root

    // Sections that show WhatsApp data give way to the login while the account is unlinked.
    readonly property var pages: [
        { section: "chat", file: "pages/chat/ChatPage.qml", whatsapp: true },
        { section: "status", file: "pages/status/StatusPage.qml", whatsapp: true },
        { section: "channels", file: "pages/channels/ChannelsPage.qml", whatsapp: true },
        { section: "calls", file: "pages/calls/CallsPage.qml", whatsapp: true },
        { section: "triggers", file: "pages/bot/TriggersPage.qml" },
        { section: "cron", file: "pages/bot/CronPage.qml" },
        { section: "webhooks", file: "pages/bot/WebhooksPage.qml" },
        { section: "documentation", file: "pages/bot/DocumentationPage.qml" },
        { section: "settings", file: "pages/settings/SettingsPage.qml" }
    ]

    Repeater {
        model: root.pages
        delegate: Loader {
            id: ld
            required property var modelData
            readonly property bool current: AppState.section === modelData.section
            property bool visited: false
            anchors.fill: parent
            visible: current && !(modelData.whatsapp && AppState.unlinked)
            onCurrentChanged: if (current) visited = true
            Component.onCompleted: if (current) visited = true
            active: visited
            source: Qt.resolvedUrl("../" + modelData.file)
            opacity: visible ? 1 : 0
        }
    }

    Loader {
        id: login
        anchors.fill: parent
        readonly property var cur: root.pages.find(p => p.section === AppState.section)
        active: AppState.unlinked && !!cur && !!cur.whatsapp
        visible: active
        source: Qt.resolvedUrl("../pages/auth/LoginPage.qml")
    }
}
