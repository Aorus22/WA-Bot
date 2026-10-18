import QtQuick
import QtQuick.Layouts
import "../../ui"

Item {
    id: root
    objectName: "channel-row-" + String(root.channel.jid || "")
    property var channel: ({})
    property bool selected: false
    signal opened()
    implicitHeight: Math.max(68, details.implicitHeight + 8)

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        radius: Theme.rem * 0.75
        color: root.selected ? Theme.alpha(Theme.primary, 0.14)
            : mouse.containsMouse ? Theme.alpha(Theme.muted, 0.4) : "transparent"
    }
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 12
        Avatar {
            chatId: String(root.channel.jid || "")
            name: String(root.channel.name || "")
            avatar: String(root.channel.avatar || "")
            size: 44
            variant: "tinted"
        }
        ColumnLayout {
            id: details
            objectName: "channelRowDetails"
            Layout.fillWidth: true
            spacing: 2
            RowLayout {
                id: nameRow
                objectName: "channelRowNameRow"
                Layout.fillWidth: true
                spacing: 4
                AppText {
                    id: nameText
                    objectName: "channelRowNameText"
                    Layout.fillWidth: true
                    text: String(root.channel.name || "")
                    size: 0.875
                    semibold: true
                    wrapMode: Text.WordWrap
                    lineHeight: 1.65
                    lineHeightMode: Text.ProportionalHeight
                }
                AppText { visible: Boolean(root.channel.verified); text: "✓"; size: 0.75; semibold: true; color: Theme.primary }
            }
            AppText {
                Layout.fillWidth: true
                text: String(root.channel.subscribers || 0) + " subscribers" + (root.channel.muted ? " · muted" : "")
                size: 0.6875
                color: Theme.mutedForeground
                elide: Text.ElideRight
            }
        }
    }
    MouseArea { id: mouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.opened() }
}
