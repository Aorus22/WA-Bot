import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    objectName: "messageInfoSheet"
    property bool open: Messages.infoOpen
    property bool closing: false
    readonly property var info: Messages.info

    visible: open || closing
    z: open ? 85 : 60

    onOpenChanged: {
        if (!open) {
            closing = true
            closeAnimationTimer.restart()
            return
        }
        closeAnimationTimer.stop()
        closing = false
    }

    Timer {
        id: closeAnimationTimer
        interval: 280
        onTriggered: if (!root.open) root.closing = false
    }

    Rectangle {
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        x: root.open ? root.width - width : root.width
        width: Math.min(320, Math.max(0, root.width - 20))
        color: Theme.background
        border.width: 0
        z: 1
        Behavior on x {
            NumberAnimation { duration: 240; easing.type: Easing.OutQuint }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: mouse => mouse.accepted = true
        }

        ColumnLayout {
            z: 1
            anchors.fill: parent
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 48
                color: Theme.alpha(Theme.muted, 0.2)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    IconButton {
                        objectName: "messageInfoClose"
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 24
                        icon: "x"
                        iconSize: 16
                        pad: 4
                        radius: 12
                        iconColor: Theme.mutedForeground
                        hoverColor: Theme.alpha(Theme.muted, 0.65)
                        tooltip: "Close"
                        onClicked: Messages.closeInfo()
                    }
                    AppText { Layout.fillWidth: true; text: "Message info"; size: 1; semibold: true }
                }
            }
            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.45) }
            Flickable {
                objectName: "messageInfoFlickable"
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: width
                contentHeight: content.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ColumnLayout {
                    id: content
                    width: parent.width
                    spacing: 0
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.rightMargin: 16
                        Layout.topMargin: 16
                        Layout.bottomMargin: 16
                        Item { Layout.fillWidth: true }
                        Rectangle {
                            Layout.maximumWidth: 260
                            Layout.preferredWidth: Math.min(260, preview.implicitWidth + 24)
                            implicitHeight: preview.implicitHeight + 16
                            radius: 12
                            color: Theme.chatBubbleOut
                            ColumnLayout {
                                id: preview
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                anchors.topMargin: 8
                                anchors.bottomMargin: 8
                                AppText {
                                    Layout.fillWidth: true
                                    text: String(root.info.text || "Message")
                                    size: 0.875
                                    color: Theme.chatBubbleOutText
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }
                    ColumnLayout {
                        objectName: "messageInfoLoading"
                        visible: Boolean(root.info.loading)
                        Layout.fillWidth: true
                        Layout.topMargin: 32
                        Layout.bottomMargin: 32
                        spacing: 8
                        Spinner { Layout.alignment: Qt.AlignHCenter; size: 22 }
                        AppText { Layout.alignment: Qt.AlignHCenter; text: "Loading…"; size: 0.875; color: Theme.mutedForeground }
                    }
                    AppText {
                        objectName: "messageInfoEmpty"
                        visible: !root.info.loading && Boolean(root.info.isGroup) && Boolean(root.info.empty)
                        Layout.fillWidth: true
                        Layout.leftMargin: 16
                        Layout.topMargin: 32
                        Layout.bottomMargin: 32
                        text: "No receipts reported yet."
                        size: 0.875
                        color: Theme.mutedForeground
                    }
                    ColumnLayout {
                        visible: !root.info.loading && !root.info.isGroup
                        Layout.fillWidth: true
                        Repeater {
                            model: root.info.stages || []
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Layout.leftMargin: 16
                                Layout.rightMargin: 16
                                spacing: 12
                                Layout.topMargin: 8
                                Layout.bottomMargin: 8
                                Icon {
                                    name: modelData.label === "Read" ? "check-check" : "check-check"
                                    size: 16
                                    color: modelData.blue ? "#53bdeb" : Theme.mutedForeground
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    AppText { Layout.fillWidth: true; text: String(modelData.label || ""); size: 0.875 }
                                    AppText { Layout.fillWidth: true; text: String(modelData.at || ""); size: 0.75; color: Theme.mutedForeground }
                                }
                            }
                        }
                        AppText {
                            visible: !root.info.loading && Boolean(root.info.notDelivered)
                            Layout.fillWidth: true
                            Layout.leftMargin: 16
                            Layout.topMargin: 32
                            Layout.bottomMargin: 32
                            text: "Not delivered yet."
                            size: 0.875
                            color: Theme.mutedForeground
                        }
                    }
                    ColumnLayout {
                        visible: !root.info.loading && Boolean(root.info.isGroup)
                        Layout.fillWidth: true
                        Repeater {
                            model: [
                                { key: "read", label: "Read by", icon: "check-check", color: "#53bdeb" },
                                { key: "delivered", label: "Delivered to", icon: "check-check", color: Theme.mutedForeground },
                                { key: "remaining", label: "Remaining", icon: "check", color: Theme.mutedForeground }
                            ]
                            delegate: ColumnLayout {
                                required property var modelData
                                property var rows: root.info[modelData.key] || []
                                visible: rows.length > 0
                                Layout.fillWidth: true
                                spacing: 2
                                RowLayout {
                                    Layout.fillWidth: true
                                    Layout.leftMargin: 16
                                    Layout.rightMargin: 16
                                    Layout.topMargin: 16
                                    Layout.bottomMargin: 4
                                    spacing: 12
                                    Icon { name: modelData.icon; size: 16; color: modelData.color }
                                    AppText { Layout.fillWidth: true; text: modelData.label; size: 0.875; color: Theme.mutedForeground }
                                }
                                Repeater {
                                    model: parent.rows
                                    delegate: RowLayout {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 16
                                        Layout.rightMargin: 16
                                        Layout.topMargin: 8
                                        Layout.bottomMargin: 8
                                        spacing: 12
                                        Avatar {
                                            Layout.preferredWidth: 40
                                            Layout.preferredHeight: 40
                                            size: 40
                                            chatId: String(modelData.jid || "")
                                            name: String(modelData.name || "")
                                        }
                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 0
                                            AppText {
                                                Layout.fillWidth: true
                                                text: String(modelData.name || modelData.jid || "")
                                                size: 0.875
                                                elide: Text.ElideRight
                                            }
                                            AppText {
                                                visible: String(modelData.atText || "") !== ""
                                                text: String(modelData.atText || "—")
                                                size: 0.75
                                                color: Theme.mutedForeground
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Item { Layout.preferredHeight: 16 }
                }
            }
        }
        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            width: 1
            color: Theme.alpha(Theme.border, 0.4)
            z: 2
        }
    }
}
