import QtQuick
import QtQuick.Layouts
import "../../ui"

Rectangle {
    id: root
    color: "#0b141a"
    readonly property var entry: Statuses.viewerEntry
    readonly property var group: Statuses.viewerGroup
    readonly property string displayName: String(group.displayName || "")

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Row {
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 12
            spacing: 6
            Repeater {
                model: Statuses.viewerCount
                delegate: Rectangle {
                    required property int index
                    width: Math.max(8, (root.width - 32 - (Statuses.viewerCount - 1) * 6) / Math.max(1, Statuses.viewerCount))
                    height: 3
                    radius: 2
                    color: index <= Statuses.viewerIndex ? "#ffffff" : "#55ffffff"
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 60
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            spacing: 12

            Avatar {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                chatId: String(root.group.sender || "")
                name: root.displayName
                avatar: String(root.group.avatar || "")
                size: 36
                variant: "solid"
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                AppText { Layout.fillWidth: true; text: root.displayName; size: 0.875; font.weight: Font.Bold; color: "#ffffff"; elide: Text.ElideRight }
                AppText { Layout.fillWidth: true; text: String(root.entry.timeLabel || ""); size: 0.75; color: "#99ffffff" }
            }
            Rectangle {
                id: closeViewerAction
                Layout.preferredWidth: closeViewerLabel.implicitWidth + 24
                Layout.preferredHeight: closeViewerLabel.implicitHeight + 12
                radius: Theme.rem * 0.5
                color: closeViewerMouse.containsMouse ? "#33ffffff" : "transparent"
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Tutup"
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        Statuses.close()
                        event.accepted = true
                    }
                }
                AppText {
                    id: closeViewerLabel
                    anchors.centerIn: parent
                    text: "Tutup"
                    size: 0.75
                    medium: true
                    color: closeViewerMouse.containsMouse ? "#ffffff" : "#b3ffffff"
                }
                MouseArea {
                    id: closeViewerMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Statuses.close()
                }
            }
        }

        Item {
            id: storyBody
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            Rectangle {
                anchors.fill: parent
                visible: root.entry.type !== "image" || String(root.entry.src || "") === ""
                color: "#075e54"
                AppText {
                    anchors.centerIn: parent
                width: Math.min(parent.width - 80, 500)
                    text: String(root.entry.content || "")
                    size: 1.5
                    medium: true
                    color: "#ffffff"
                    wrapMode: Text.Wrap
                    horizontalAlignment: Text.AlignHCenter
                }
            }

            Image {
                id: picture
                anchors.fill: parent
                visible: root.entry.type === "image" && String(root.entry.src || "") !== ""
                source: String(root.entry.src || "")
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: true
                sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
            }

            MouseArea {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 120
                cursorShape: Qt.PointingHandCursor
                onClicked: Statuses.prev()
                Icon {
                    anchors.left: parent.left
                    anchors.leftMargin: 16
                    anchors.verticalCenter: parent.verticalCenter
                    visible: Statuses.viewerIndex > 0
                    name: "chevron-left"
                    size: 32
                    color: "#80ffffff"
                }
            }
            MouseArea {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 120
                cursorShape: Qt.PointingHandCursor
                onClicked: Statuses.next()
                Icon {
                    anchors.right: parent.right
                    anchors.rightMargin: 16
                    anchors.verticalCenter: parent.verticalCenter
                    visible: Statuses.viewerIndex + 1 < Statuses.viewerCount
                    name: "chevron-right"
                    size: 32
                    color: "#80ffffff"
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 12
            Layout.bottomMargin: 12
            spacing: 8
            visible: !Statuses.viewerIsOwn

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                radius: 6
                color: "#1affffff"
                border.width: 1
                border.color: "#26ffffff"
                TextInput {
                    objectName: "statusReplyInput"
                    id: replyInput
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: TextInput.AlignVCenter
                    color: "#ffffff"
                    selectionColor: "#59ffffff"
                    selectedTextColor: "#ffffff"
                    font.pixelSize: Math.round(Theme.rem * 0.875)
                    clip: true
                    selectByMouse: true
                    Accessible.role: Accessible.EditableText
                    Accessible.name: "Reply to status"
                    onAccepted: if (text.trim() !== "" && !Statuses.replying) Statuses.reply(text)
                }
                AppText {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    visible: replyInput.text === "" && !replyInput.preeditText
                    text: "Reply…"
                    size: 0.875
                    color: "#99ffffff"
                }
            }
            Rectangle {
                objectName: "statusReplySend"
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 18
                readonly property bool canSend: replyInput.text.trim() !== "" && !Statuses.replying
                color: Theme.alpha(Theme.primary, canSend ? 1 : 0.4)
                Icon { anchors.centerIn: parent; name: "send"; size: 16; color: Theme.primaryForeground }
                MouseArea {
                    anchors.fill: parent
                    enabled: parent.canSend
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Statuses.reply(replyInput.text)
                }
            }
        }
    }

    Connections {
        target: Statuses
        function onReplySent(chatId) { replyInput.clear() }
        function onViewerChanged() { replyInput.text = "" }
    }
}
