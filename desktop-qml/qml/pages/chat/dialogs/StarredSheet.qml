import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    property bool open: false
    property bool closing: false
    property bool openedFullWidth: false
    property bool fullWidth: false
    signal closed()
    visible: open || closing
    z: open ? 70 : 60

    onOpenChanged: {
        if (!open) {
            if (openedFullWidth) {
                closing = false
                closeAnimationTimer.stop()
                return
            }
            closing = true
            closeAnimationTimer.restart()
            return
        }
        openedFullWidth = fullWidth
        closeAnimationTimer.stop()
        closing = false
    }

    onFullWidthChanged: if (open && fullWidth) openedFullWidth = true

    Timer {
        id: closeAnimationTimer
        interval: 280
        onTriggered: if (!root.open) root.closing = false
    }

    function chatLabel(message, chat) {
        const messageName = String(message.chatName || "").trim()
        const rowName = String((chat || {}).name || "").trim()
        return messageName || rowName || String(message.chatId || "").split("@")[0]
    }

    Rectangle {
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        x: root.fullWidth || root.open ? root.width - width : root.width
        width: root.fullWidth ? parent.width : Math.min(320, Math.max(0, root.width - 20))
        color: Theme.background
        border.width: 0
        Behavior on x {
            enabled: !root.fullWidth && !root.openedFullWidth
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
                    spacing: 8
                    Icon { name: "star"; size: 18; color: Theme.primary }
                    AppText { Layout.fillWidth: true; text: "Starred messages"; size: 1; semibold: true }
                    IconButton {
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 24
                        icon: "x"
                        iconColor: Theme.mutedForeground
                        hoverColor: Theme.alpha(Theme.muted, 0.65)
                        pad: 4
                        radius: 12
                        tooltip: "Close"
                        onClicked: root.close()
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.alpha(Theme.border, 0.4)
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: starredList
                    objectName: "starred-sheet-results-list"
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    model: Chats.starred
                    spacing: 4
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ThinScrollBar {}

                    delegate: Item {
                        id: resultRow
                        required property var modelData
                        width: starredList.width
                        height: resultContent.implicitHeight + 24
                        property bool hovered: false
                        readonly property string chatId: String(modelData.chatId || "")
                        readonly property string messageId: String(modelData.id || "")
                        readonly property var chat: Chats.get(chatId)

                        Rectangle {
                            anchors.fill: parent
                            radius: 12
                            color: resultRow.hovered ? Theme.alpha(Theme.muted, 0.5) : "transparent"
                        }
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 1
                            color: Theme.alpha(Theme.border, 0.15)
                        }
                        ColumnLayout {
                            id: resultContent
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 4
                            RowLayout {
                                Layout.fillWidth: true
                                AppText {
                                    Layout.fillWidth: true
                                    text: root.chatLabel(modelData, resultRow.chat)
                                    size: 0.75
                                    semibold: true
                                    color: Theme.primary
                                    elide: Text.ElideRight
                                }
                                AppText {
                                    text: Chats.fmtDate(Number(modelData.timestamp || 0))
                                    size: 0.75
                                    medium: true
                                    color: Theme.mutedForeground
                                }
                            }
                            AppText {
                                Layout.fillWidth: true
                                text: Chats.senderLabel(modelData)
                                size: 0.75
                                color: Theme.mutedForeground
                                elide: Text.ElideRight
                            }
                            AppText {
                                Layout.fillWidth: true
                                text: Chats.snippetOf(modelData)
                                size: 0.75
                                color: Theme.foreground
                                maximumLineCount: 2
                                wrapMode: Text.WordWrap
                                elide: Text.ElideRight
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                AppText {
                                    Layout.fillWidth: true
                                    text: Chats.fmtTime(Number(modelData.timestamp || 0))
                                    size: 0.75
                                    color: Theme.alpha(Theme.mutedForeground, 0.7)
                                    horizontalAlignment: Text.AlignRight
                                }
                                Item {
                                    width: 20
                                    height: 20
                                    Icon { anchors.centerIn: parent; name: "star-filled"; size: 13; color: Theme.primary }
                                    MouseArea {
                                        anchors.fill: parent
                                        z: 1
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        Accessible.role: Accessible.Button
                                        Accessible.name: "Unstar message"
                                        onClicked: {
                                            mouse.accepted = true
                                            Chats.unstar(resultRow.chatId, resultRow.messageId)
                                        }
                                    }
                                }
                            }
                        }
                        MouseArea {
                            id: resultHit
                            z: -1
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onEntered: resultRow.hovered = true
                            onExited: resultRow.hovered = false
                            onClicked: {
                                Chats.openMessage(resultRow.chatId, resultRow.messageId)
                                root.close()
                            }
                        }
                    }

                    footer: Item {
                        width: starredList.width
                        height: Chats.starredHasMore ? 44 : 0
                        Btn {
                            visible: Chats.starredHasMore
                            anchors.centerIn: parent
                            text: Chats.starredLoading ? "Loading..." : "Load more"
                            variant: "ghost"
                            size: 0.75
                            enabled: !Chats.starredLoading
                            onClicked: Chats.loadStarred(true)
                        }
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 12
                    visible: Chats.starred.length === 0
                    Rectangle {
                        visible: !Chats.starredLoading
                        width: 56
                        height: 56
                        radius: 16
                        color: Theme.alpha(Theme.muted, 0.3)
                        anchors.horizontalCenter: parent.horizontalCenter
                        Icon { anchors.centerIn: parent; name: "star"; size: 26; color: Theme.alpha(Theme.mutedForeground, 0.7) }
                    }
                    Spinner {
                        visible: Chats.starredLoading
                        anchors.horizontalCenter: parent.horizontalCenter
                        size: 24
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Chats.starredLoading ? "Loading..." : "No starred messages"
                        size: 0.875
                        semibold: true
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: !Chats.starredLoading
                        text: "Right-click a message and choose Star to keep it here."
                        size: 0.75
                        color: Theme.mutedForeground
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                        width: Math.min(320, parent.parent.width - 48)
                    }
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

    function close() {
        root.open = false
        root.closed()
    }
}
