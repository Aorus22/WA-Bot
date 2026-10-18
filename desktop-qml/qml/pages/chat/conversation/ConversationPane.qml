import QtQuick
import QtQuick.Layouts
import "../../../ui"
import "../composer"
import "../messages"

Rectangle {
    id: root
    color: Theme.glass(Theme.hasChat ? Theme.chatChatBg : Theme.background)
    property bool sidebarCollapsed: false
    readonly property bool composerFocused: composer.editorActiveFocus
    signal toggleSidebarRequested()

    function handleConversationScrollKey(key) {
        messageList.handleComposerScrollKey(key)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        visible: Chats.activeChatId !== ""

        ChatHeader {
            id: chatHeader
            Layout.fillWidth: true
            Layout.preferredHeight: 56
            visible: !Messages.selectMode
            sidebarCollapsed: root.sidebarCollapsed
            onToggleSidebarRequested: root.toggleSidebarRequested()
        }
        SelectionBar { Layout.fillWidth: true; Layout.preferredHeight: 50; visible: Messages.selectMode }
        MessageList {
            id: messageList
            Layout.fillWidth: true
            Layout.fillHeight: true
            composerFocused: composer.editorActiveFocus
        }
        ComposerBar {
            id: composer
            Layout.fillWidth: true
            onScrollKeyRequested: key => messageList.handleComposerScrollKey(key)
        }
    }

    Column {
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 520)
        spacing: 12
        visible: Chats.activeChatId === ""

        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 72
            height: 72
            radius: Theme.rem * 1.125
            color: Theme.alpha(Theme.primary, 0.1)
            Icon { anchors.centerIn: parent; name: "message-square"; size: 32; color: Theme.primary }
        }
        AppText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "WA Bot Desktop"
            size: 1.25
            semibold: true
        }
        AppText {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: "Select a conversation from the left to start chatting."
            size: 0.875
            color: Theme.mutedForeground
            wrapMode: Text.WordWrap
        }
    }

}
