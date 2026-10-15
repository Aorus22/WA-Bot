import QtQuick
import QtQuick.Layouts
import "../../../ui"
import "../composer"

// TODO(chat-list agent): header + MessageList (chat-messages agent) + ComposerBar (composer agent).
Rectangle {
    color: "transparent"
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        ChatHeader { Layout.fillWidth: true; Layout.preferredHeight: 56 }
        MessageList { Layout.fillWidth: true; Layout.fillHeight: true }
        ComposerBar { Layout.fillWidth: true; Layout.preferredHeight: 64 }
    }
}
