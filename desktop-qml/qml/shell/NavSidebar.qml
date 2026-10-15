import QtQuick
import "../ui"

// 68px navigation rail (web NavigationSidebar). Icons are 16px in 40px pills (p-3, rounded-2xl).
Rectangle {
    id: root
    implicitWidth: 68
    color: Theme.glass(Theme.background)
    Rectangle { anchors.right: parent.right; height: parent.height; width: 1; color: Theme.alpha(Theme.border, 0.4) }

    readonly property var navItems: [
        { label: "Chats", section: "chat", icon: "inbox" },
        { label: "Status", section: "status", icon: "circle-check" },
        { label: "Channels", section: "channels", icon: "bell" },
        { label: "Calls", section: "calls", icon: "network" },
        { label: "Triggers", section: "triggers", icon: "bot" },
        { label: "Cron Jobs", section: "cron", icon: "calendar" },
        { label: "Webhooks", section: "webhooks", icon: "globe" },
        { label: "Documentation", section: "documentation", icon: "file-text" }
    ]

    component NavItem: Item {
        id: item
        required property var d
        readonly property bool active: AppState.section === d.section
        width: root.width; height: 40
        Rectangle {
            visible: item.active
            width: 3; height: 24; radius: 1.5
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.primary
            // flat left edge: only the right side is rounded
            Rectangle { width: 2; height: parent.height; color: parent.color }
        }
        Rectangle {
            id: pill
            anchors.centerIn: parent
            width: 40; height: 40; radius: 16
            color: item.active ? (ma.containsMouse ? Theme.alpha(Theme.primary, 0.85) : Theme.primary)
                               : (ma.containsMouse ? Theme.alpha(Theme.muted, 0.8) : "transparent")
            Behavior on color { ColorAnimation { duration: Theme.baseMs; easing.type: Easing.OutQuint } }
            Icon {
                anchors.centerIn: parent
                name: item.d.icon
                color: item.active ? Theme.primaryForeground : (ma.containsMouse ? Theme.foreground : Theme.mutedForeground)
            }
            MouseArea {
                id: ma; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                onClicked: AppState.navigate(item.d.section)
            }
            ToolTipBubble { visible: ma.containsMouse; text: item.d.label; x: parent.width + 8; y: (parent.height - height) / 2 }
        }
    }

    Column {
        anchors.top: parent.top; anchors.topMargin: 24
        spacing: 12
        Repeater { model: root.navItems; delegate: NavItem { required property var modelData; d: modelData } }
    }
    NavItem {
        anchors.bottom: parent.bottom; anchors.bottomMargin: 24
        d: ({ label: "Settings", section: "settings", icon: "settings" })
    }
}
