import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import "../../ui"

Popup {
    id: root
    objectName: "statusComposer"
    property real backgroundArgb: 0xff075e54
    property string draft: ""

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(420, parent ? parent.width - 32 : 420)
    modal: true
    dim: true
    padding: 20
    closePolicy: Popup.CloseOnEscape
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 136 / 255) }
    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: 220
            easing.type: Easing.OutQuint
        }
    }
    exit: Transition {
        NumberAnimation { property: "opacity"; to: 0; duration: 100 }
    }
    background: Item {
        Rectangle {
            id: composerSurface
            anchors.fill: parent
            color: Theme.card
            radius: Theme.rem
            border.color: Theme.border
        }
        MultiEffect {
            anchors.fill: composerSurface
            source: composerSurface
            shadowEnabled: true
            shadowColor: "#66000000"
            shadowBlur: 0.5
            shadowVerticalOffset: 8
            z: -1
        }
    }

    function colorFor(argb) {
        return Qt.rgba(((argb >>> 16) & 255) / 255, ((argb >>> 8) & 255) / 255, (argb & 255) / 255, ((argb >>> 24) & 255) / 255)
    }

    contentItem: ColumnLayout {
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            AppText { Layout.fillWidth: true; text: "New status"; size: 1.125; font.weight: Font.Bold }
            IconButton {
                objectName: "statusComposerClose"
                icon: "x"
                iconSize: 16
                circular: true
                iconColor: Theme.mutedForeground
                hoverColor: Theme.alpha(Theme.muted, 0.5)
                pad: 4
                tooltip: "Close"
                onClicked: root.close()
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 160
            radius: Theme.rem * 0.75
            color: root.colorFor(root.backgroundArgb)
            TextArea {
                id: editor
                objectName: "statusComposerEditor"
                anchors.fill: parent
                anchors.margins: 16
                placeholderText: "Type a status…"
                placeholderTextColor: Theme.alpha("#ffffff", 0.68)
                color: "#ffffff"
                selectionColor: Theme.alpha("#ffffff", 0.35)
                selectedTextColor: "#ffffff"
                font.pixelSize: Math.round(Theme.rem * 1.125)
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                background: Item {}
                onTextChanged: root.draft = text
            }
        }

        Row {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: Statuses.backgrounds
                delegate: Rectangle {
                    required property var modelData
                    width: 28
                    height: 28
                    radius: 14
                    color: root.colorFor(modelData)
                    border.width: root.backgroundArgb === modelData ? 2 : 0
                    border.color: Theme.primary
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.backgroundArgb = modelData
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 8
            spacing: 8
            Item { Layout.fillWidth: true }
            Rectangle {
                implicitWidth: cancelLabel.implicitWidth + 32
                implicitHeight: cancelLabel.implicitHeight + 16
                radius: 12
                color: cancelMouse.containsMouse ? Theme.alpha(Theme.muted, 0.5) : "transparent"
                AppText {
                    id: cancelLabel
                    anchors.centerIn: parent
                    text: "Cancel"
                    size: 0.875
                    medium: true
                }
                MouseArea {
                    id: cancelMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.close()
                }
            }
            Rectangle {
                readonly property bool canPost: root.draft.trim() !== "" && !Statuses.posting
                implicitWidth: postLabel.implicitWidth + 40
                implicitHeight: postLabel.implicitHeight + 16
                radius: 12
                color: Theme.alpha(Theme.primary, canPost ? 1 : 0.5)
                AppText {
                    id: postLabel
                    anchors.centerIn: parent
                    text: Statuses.posting ? "Posting…" : "Post"
                    size: 0.875
                    font.weight: Font.Bold
                    color: Theme.primaryForeground
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: parent.canPost ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: if (parent.canPost) Statuses.postText(root.draft, root.backgroundArgb)
                }
            }
        }
    }

    Connections {
        target: Statuses
        function onPosted() {
            root.draft = ""
            editor.clear()
            root.close()
        }
    }

    onOpened: editor.forceActiveFocus()
}
