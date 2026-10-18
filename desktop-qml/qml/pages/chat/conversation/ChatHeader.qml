import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

// Conversation identity row, sized like GPUI's 56 px chat header.
Rectangle {
    id: root
    implicitHeight: 56
    property bool sidebarCollapsed: false
    property bool muteExpanded: false
    property bool confirmExitGroup: false
    readonly property bool pagesLayout: System.chatLayout === "pages"
        || (System.chatLayout === "auto" && Platform.popoverMode)
    color: Theme.glass(Theme.hasChat ? Theme.chatHeader : Theme.background)
    border.width: 0
    signal toggleSidebarRequested()

    function startCall(video) {
        if (Boolean(Chats.activeChat.isGroup)) {
            Calls.startGroupCall(Chats.activeChatId, [], video ? "group_video" : "group_audio")
        } else {
            Calls.startCall(Chats.activeChatId, video ? "video" : "audio")
        }
    }

    function headerMenuItems() {
        const items = []
        const chat = Chats.activeChat || ({})
        const group = Boolean(chat.isGroup)
        const muted = Boolean(chat.muteMode && chat.muteMode !== "off")

        if (group) items.push({ label: "Add member", icon: "plus", action: "add" })
        items.push({ label: group ? "Group info" : "Contact info", icon: "user", action: "info" })
        items.push({ label: "Search", icon: "search", action: "search" })
        items.push({ label: "Select messages", icon: "check", action: "select" })
        if (muted) {
            items.push({ label: "Unmute notifications", icon: "volume-x", action: "unmute" })
        } else {
            items.push({ label: "Mute notifications", icon: "volume-x", chevron: true, action: "mute-expand", keepOpen: true })
            if (muteExpanded) {
                items.push({ label: "8 hours", action: "mute-8h", indent: true, small: true })
                items.push({ label: "1 week", action: "mute-1w", indent: true, small: true })
                items.push({ label: "Always", action: "mute-forever", indent: true, small: true })
            }
        }
        items.push({ label: "Disappearing messages", icon: "clock", action: "disappearing" })
        items.push({ label: "Export chat", icon: "download", action: "export" })
        items.push({ label: "Close chat", icon: "x", action: "close" })
        if (group) {
            items.push({ separator: true })
            items.push({ label: confirmExitGroup ? "Click again to exit group" : "Exit group",
                         icon: "user-minus", danger: true, action: "exit", keepOpen: !confirmExitGroup })
        }
        return items
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 20
        anchors.rightMargin: 20
        spacing: 4

        IconButton {
            objectName: "chatSidebarToggleButton"
            Layout.preferredWidth: 34
            Layout.preferredHeight: 34
            iconSize: 18
            pad: 8
            radius: width / 2
            iconColor: Theme.mutedForeground
            hoverColor: Theme.alpha(Theme.muted, 0.65)
            icon: root.pagesLayout ? "chevron-left" : "sidebar"
            tooltip: root.pagesLayout ? "Back to chats" : root.sidebarCollapsed ? "Show chats" : "Hide chats"
            Accessible.name: tooltip
            onClicked: {
                if (root.pagesLayout) AppState.openChat("")
                else root.toggleSidebarRequested()
            }
        }

        Rectangle {
            id: profileArea
            objectName: "chatHeaderProfile"
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            Layout.leftMargin: 8
            radius: 12
            color: profileMouse.containsMouse ? Theme.alpha(Theme.muted, 0.6) : "transparent"
            border.width: 0
            RowLayout {
                anchors.fill: parent
                spacing: 12
                Avatar {
                    Layout.alignment: Qt.AlignVCenter
                    chatId: Chats.activeChatId
                    name: String(Chats.activeChat.name || "")
                    avatar: String(Chats.activeChat.avatar || "")
                    isGroup: Boolean(Chats.activeChat.isGroup)
                    size: 40
                    variant: "tinted"
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 5
                        AppText {
                            Layout.fillWidth: true
                            text: String(Chats.activeChat.name || "")
                            size: 1
                            semibold: true
                            elide: Text.ElideRight
                        }
                        Icon {
                            objectName: "chatHeaderDisappearing"
                            readonly property int seconds: Number(Chats.activeChat.disappearingSecs || 0)
                            visible: seconds > 0
                            name: "clock"
                            size: 14
                            color: Theme.mutedForeground
                            Accessible.role: Accessible.StaticText
                            Accessible.name: "Disappearing messages: " + Chats.disappearingLabel(seconds)
                        }
                    }
                    AppText {
                        readonly property string typingText: String(Chats.activeChat.typing || "")
                        visible: Chats.activeChatId !== "" || typingText !== ""
                        text: typingText !== "" ? typingText : String(Chats.activeChatId)
                        size: 0.75
                        color: typingText !== "" ? (Theme.isDark ? "#25d366" : "#008069") : Theme.mutedForeground
                        elide: Text.ElideRight
                    }
                }
            }
            MouseArea {
                id: profileMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                Accessible.name: "Chat info"
                onClicked: Chats.requestSheet("info", Chats.activeChatId)
            }
        }

        Rectangle {
            objectName: "chatHeaderCallButton"
            Layout.preferredWidth: 50
            Layout.preferredHeight: 34
            radius: 17
            color: callMouse.containsMouse ? Theme.alpha(Theme.muted, 0.65) : "transparent"
            Row {
                anchors.centerIn: parent
                spacing: 4
                Icon { name: "video"; size: 18; color: Theme.mutedForeground }
                Icon { name: "chevron-right"; size: 11; color: Theme.mutedForeground }
            }
            MouseArea {
                id: callMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                Accessible.name: Boolean(Chats.activeChat.isGroup) ? "Group call" : "Call"
                onClicked: callMenu.popupAt(callMouse, true, false)
            }
            ToolTipBubble {
                visible: callMouse.containsMouse
                text: Boolean(Chats.activeChat.isGroup) ? "Group call" : "Call"
            }
        }
        IconButton {
            icon: "search"
            tooltip: "Search in conversation"
            onClicked: Chats.requestSheet("search", Chats.activeChatId)
        }
        IconButton {
            id: chatMenuButton
            icon: "more-vertical"
            tooltip: "Menu"
            objectName: "chatHeaderMenuButton"
            onClicked: {
                root.muteExpanded = false
                root.confirmExitGroup = false
                chatMenu.popupAt(chatMenuButton, true, false)
            }
        }
    }

    AppMenu {
        id: callMenu
        minWidth: 252
        model: [
            { label: "Voice call", icon: "phone", action: "voice" },
            { label: "Video call", icon: "video", action: "video" }
        ]
        onTriggered: item => root.startCall(item.action === "video")
    }

    AppMenu {
        id: chatMenu
        minWidth: 252
        model: root.headerMenuItems()
        onOpened: { root.muteExpanded = false; root.confirmExitGroup = false }
        onClosed: {
            root.muteExpanded = false
            Qt.callLater(() => root.confirmExitGroup = false)
        }
        onTriggered: item => {
            if (item.action === "add") Chats.requestAddMembers(Chats.activeChatId)
            else if (item.action === "info" || item.action === "search") Chats.requestSheet(item.action, Chats.activeChatId)
            else if (item.action === "select") Messages.enterSelect()
            else if (item.action === "mute-expand") root.muteExpanded = !root.muteExpanded
            else if (item.action === "unmute") Chats.setMute(Chats.activeChatId, "off")
            else if (item.action === "mute-8h") Chats.setMute(Chats.activeChatId, "8h")
            else if (item.action === "mute-1w") Chats.setMute(Chats.activeChatId, "1w")
            else if (item.action === "mute-forever") Chats.setMute(Chats.activeChatId, "forever")
            else if (item.action === "disappearing") Chats.requestSheet("info", Chats.activeChatId)
            else if (item.action === "export") Chats.exportChat(Chats.activeChatId, String(Chats.activeChat.name || "Chat"))
            else if (item.action === "close") AppState.openChat("")
            else if (item.action === "exit") {
                if (root.confirmExitGroup) {
                    root.confirmExitGroup = false
                    Chats.leaveGroup(Chats.activeChatId)
                } else {
                    root.confirmExitGroup = true
                }
            }
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.alpha(Theme.border, 0.45)
    }
}
