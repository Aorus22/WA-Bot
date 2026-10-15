import QtQuick
import QtQuick.Layouts
import "../../ui"
import "sidebar"
import "conversation"

// TODO(chat-list agent): real layout (sidebar | conversation, or pages per the chat_layout setting).
Rectangle {
    color: Theme.glass(Theme.background)
    RowLayout {
        anchors.fill: parent
        spacing: 0
        ChatSidebar { Layout.preferredWidth: 320; Layout.fillHeight: true }
        ConversationPane { Layout.fillWidth: true; Layout.fillHeight: true }
    }
}
