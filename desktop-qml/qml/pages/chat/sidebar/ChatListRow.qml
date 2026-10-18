import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

// One fixed-height conversation row, matching GPUI's chat list.
Item {
    id: root
    objectName: "chat-row-" + root.chatId
    property var chat
    property bool selectionMode: false
    property bool selectionChecked: false
    property int rowIndex: -1
    readonly property string chatId: chat ? String(chat.id || "") : ""
    readonly property bool selected: selectionMode ? selectionChecked : chatId !== "" && AppState.activeChatId === chatId
    readonly property int unreadCount: chat ? Number(chat.unread || 0) : 0
    signal selectRequested(string chatId, var chat)
    signal focusMoveRequested(int rowIndex)
    property alias focusButton: openButton
    property bool muteExpanded: false
    property bool confirmLeave: false

    function showContextMenu(mouse) {
        muteExpanded = false
        confirmLeave = false
        const pos = root.mapToItem(Overlay.overlay, mouse.x, mouse.y)
        chatMenu.popup(pos.x + 8, pos.y + 8)
    }

    function contextMenuItems() {
        const items = []
        const muted = root.chat && root.chat.muteMode && root.chat.muteMode !== "off"
        if (root.unreadCount > 0) items.push({ label: "Mark as read", icon: "check-check", action: "read", primary: true })
        items.push({ label: root.chat && root.chat.archived ? "Unarchive chat" : "Archive chat", icon: "archive", action: "archive" })
        if (muted) items.push({ label: "Unmute notifications", icon: "volume-x", action: "unmute" })
        else {
            items.push({ label: "Mute notifications", icon: "volume-x", chevron: true, action: "mute-expand", keepOpen: true })
            if (muteExpanded) {
                items.push({ label: "8 hours", action: "mute-8h", indent: true, small: true })
                items.push({ label: "1 week", action: "mute-1w", indent: true, small: true })
                items.push({ label: "Always", action: "mute-forever", indent: true, small: true })
            }
        }
        items.push({ label: root.chat && root.chat.pinned ? "Unpin chat" : "Pin chat", icon: "pin", action: "pin" })
        if (root.chat && Boolean(root.chat.isGroup)) {
            items.push({ separator: true })
            items.push({ label: confirmLeave ? "Click again to exit group" : "Exit group", icon: "user-minus",
                         danger: true, action: "exit", keepOpen: !confirmLeave })
        }
        return items
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        radius: Theme.rem * 0.75
        color: root.selected
            ? Theme.hasChat ? Theme.chatListSelected : Theme.alpha(Theme.primary, openButton.hovered ? 0.22 : 0.14)
            : openButton.hovered ? (Theme.hasChat ? Theme.chatListHover : Theme.alpha(Theme.muted, 0.65)) : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.fastMs } }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 12

        Rectangle {
            visible: root.selectionMode
            Layout.preferredWidth: visible ? 20 : 0
            Layout.preferredHeight: 20
            radius: width / 2
            color: root.selectionChecked ? Theme.primary : "transparent"
            border.width: 1
            border.color: Theme.primary
            Icon { anchors.centerIn: parent; visible: root.selectionChecked; name: "check"; size: 12; color: Theme.primaryForeground }
        }

        Avatar {
            Layout.alignment: Qt.AlignVCenter
            chatId: root.chatId
            name: root.chat ? String(root.chat.name || "") : ""
            avatar: root.chat ? String(root.chat.avatar || "") : ""
            isGroup: root.chat ? Boolean(root.chat.isGroup) : false
            size: 48
            variant: "solid"
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: 3

            RowLayout {
                Layout.fillWidth: true
                spacing: 5
                AppText {
                    Layout.fillWidth: true
                    text: root.chat ? String(root.chat.name || "") : ""
                    size: 0.875
                    semibold: root.unreadCount > 0
                    medium: root.unreadCount === 0
                    elide: Text.ElideRight
                }
                Icon { visible: root.chat ? Boolean(root.chat.muted) : false; name: "volume-x"; size: 13; color: Theme.mutedForeground }
                Icon { visible: root.chat ? Boolean(root.chat.pinned) : false; name: "pin"; size: 12; color: Theme.mutedForeground }
            }

            AppText {
                Layout.fillWidth: true
                text: {
                    if (!root.chat) return ""
                    const typing = String(root.chat.typing || "")
                    if (typing !== "") return typing
                    const snippet = String(root.chat.snippet || "").trim()
                    return snippet !== "" ? snippet : "Tap to chat"
                }
                size: 0.75
                color: root.chat && String(root.chat.typing || "") !== "" ? (Theme.isDark ? "#25d366" : "#008069")
                    : root.unreadCount > 0 ? Theme.foreground : Theme.mutedForeground
                medium: root.chat ? String(root.chat.typing || "") !== "" : false
                elide: Text.ElideRight
            }
        }

        ColumnLayout {
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: 54
            spacing: 5

            AppText {
                Layout.alignment: Qt.AlignRight
                text: root.chat ? String(root.chat.timeLabel || "") : ""
                size: 0.6875
                color: root.unreadCount > 0 ? "#00a884" : Theme.mutedForeground
                semibold: root.unreadCount > 0
            }

            RowLayout {
                visible: root.unreadCount > 0
                Layout.alignment: Qt.AlignRight
                spacing: 4
                Rectangle {
                    visible: root.chat ? Boolean(root.chat.hasUnreadMention) : false
                    Layout.preferredWidth: 19
                    Layout.preferredHeight: 19
                    radius: width / 2
                    color: "#00a884"
                    AppText { anchors.centerIn: parent; text: "@"; size: 0.6875; semibold: true; color: "#111b21" }
                }
                Rectangle {
                    Layout.preferredHeight: 20
                    Layout.minimumWidth: 20
                    Layout.preferredWidth: badgeText.implicitWidth + 12
                    radius: 10
                    color: "#00a884"
                    AppText {
                        id: badgeText
                        anchors.centerIn: parent
                        text: root.unreadCount > 99 ? "99+" : String(root.unreadCount)
                        size: 0.6875
                        semibold: true
                        color: "#111b21"
                    }
                }
            }
        }
    }

    Btn {
        id: openButton
        objectName: "chatRowOpen_" + root.chatId
        anchors.fill: parent
        accessibleName: "Open chat " + String(root.chat ? root.chat.name || root.chatId : root.chatId)
        variant: "link"
        onClicked: {
            if (root.chatId === "") return
            if (root.selectionMode) root.selectRequested(root.chatId, root.chat)
            else Chats.open(root.chatId)
        }
        Keys.onUpPressed: event => {
            if (root.rowIndex > 0) {
                root.focusMoveRequested(root.rowIndex - 1)
                event.accepted = true
            }
        }
        Keys.onDownPressed: event => {
            root.focusMoveRequested(root.rowIndex + 1)
            event.accepted = true
        }
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        z: 2
        onClicked: mouse => root.showContextMenu(mouse)
    }

    AppMenu {
        id: chatMenu
        minWidth: 232
        model: root.contextMenuItems()
        onClosed: {
            root.muteExpanded = false
            Qt.callLater(() => root.confirmLeave = false)
        }
        onTriggered: item => {
            if (item.action === "read") Chats.markRead(root.chatId)
            else if (item.action === "archive") Chats.toggleArchive(root.chatId)
            else if (item.action === "mute-expand") root.muteExpanded = !root.muteExpanded
            else if (item.action === "unmute") Chats.setMute(root.chatId, "off")
            else if (item.action === "mute-8h") Chats.setMute(root.chatId, "8h")
            else if (item.action === "mute-1w") Chats.setMute(root.chatId, "1w")
            else if (item.action === "mute-forever") Chats.setMute(root.chatId, "forever")
            else if (item.action === "pin") Chats.togglePin(root.chatId)
            else if (item.action === "exit") {
                if (root.confirmLeave) {
                    root.confirmLeave = false
                    Chats.leaveGroup(root.chatId)
                } else {
                    root.confirmLeave = true
                }
            }
        }
    }
}
