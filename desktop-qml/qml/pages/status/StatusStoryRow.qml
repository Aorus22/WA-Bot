import QtQuick
import QtQuick.Layouts
import "../../ui"

Item {
    id: root
    property var group: ({})
    property bool own: false
    signal opened()
    objectName: "statusRow_" + (own ? "own" : String(group.sender || ""))
    implicitHeight: 72

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        radius: Theme.rem * 0.75
        color: mouse.containsMouse ? Theme.alpha(Theme.muted, 0.4) : "transparent"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 12

        Item {
            Layout.preferredWidth: 48
            Layout.preferredHeight: 48
            Rectangle {
                visible: root.own
                anchors.fill: parent
                radius: width / 2
                color: Theme.alpha(Theme.primary, 0.12)
            }
            Icon { visible: root.own; anchors.centerIn: parent; name: "file-text"; size: 20; color: Theme.primary }
            Rectangle {
                visible: !root.own
                anchors.centerIn: parent
                width: 46
                height: 46
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: root.group.allViewed ? Theme.alpha(Theme.mutedForeground, 0.3) : Theme.primary
            }
            Avatar {
                visible: !root.own
                anchors.centerIn: parent
                chatId: String(root.group.sender || "")
                name: String(root.group.displayName || "")
                avatar: String(root.group.avatar || "")
                size: 38
                variant: "solid"
            }
            Rectangle {
                visible: root.own && Number(root.group.count || 0) === 0
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.rightMargin: -2
                anchors.bottomMargin: -2
                width: 20
                height: 20
                radius: 10
                color: Theme.primary
                border.width: 2
                border.color: Theme.background
                Icon { anchors.centerIn: parent; name: "plus"; size: 12; color: Theme.primaryForeground }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2
            AppText {
                Layout.fillWidth: true
                text: root.own ? "Status Saya" : String(root.group.displayName || "")
                size: 0.875
                font.weight: Font.Bold
                elide: Text.ElideRight
            }
            AppText {
                Layout.fillWidth: true
                text: root.own && Number(root.group.count || 0) === 0
                    ? "Klik untuk tambah status" : String(root.group.timeLabel || "")
                size: 0.75
                color: Theme.mutedForeground
                elide: Text.ElideRight
            }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.opened()
    }
}
