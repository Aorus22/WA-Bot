import QtQuick
import QtQuick.Controls
import "../../../ui"

Item {
    id: root
    property bool composerFocused: false

    function chatScrollAvailable() {
        if (AppState.section !== "chat" || Chats.activeChatId === ""
                || Messages.viewerOpen || messages.count === 0)
            return false
        return true
    }

    function pageConversation(direction) {
        if (Messages.loading || messages.count === 0)
            return
        const minimum = messages.originY
        const maximum = Math.max(minimum, messages.contentHeight - messages.height)
        messages.contentY = Math.max(minimum, Math.min(maximum, messages.contentY + direction * messages.height * 0.9))
        Qt.callLater(() => messages.pageAtEdge())
    }

    function showOldest() {
        if (messages.count === 0)
            return
        messages.positionViewAtIndex(messages.count - 1, ListView.Beginning)
        Qt.callLater(() => messages.pageAtEdge())
    }

    function handleComposerScrollKey(key) {
        if (!root.chatScrollAvailable())
            return
        if (key === Qt.Key_PageUp)
            root.pageConversation(-1)
        else if (key === Qt.Key_PageDown)
            root.pageConversation(1)
        else if (key === Qt.Key_Home)
            root.showOldest()
        else if (key === Qt.Key_End)
            Messages.jumpToLatest()
    }

    ListView {
        id: messages
        objectName: "messageList"
        anchors.fill: parent
        clip: true
        focus: true
        activeFocusOnTab: true
        model: Messages.model
        verticalLayoutDirection: ListView.BottomToTop
        boundsBehavior: Flickable.StopAtBounds
        spacing: 0
        ScrollBar.vertical: ThinScrollBar {}
        function pageAtEdge() {
            if (Messages.loading || messages.count === 0)
                return
            // When a context window is shorter than the viewport, both flags are
            // true; leave paging to the explicit edge buttons in that case.
            if (messages.atYBeginning === messages.atYEnd)
                return
            if (messages.atYBeginning && Messages.hasMore && !Messages.loadingMore)
                Messages.loadOlder()
            else if (messages.atYEnd && Messages.hasMoreNext && !Messages.loadingNewer)
                Messages.loadNewer()
        }
        Keys.onPressed: event => {
            if (root.composerFocused || !root.chatScrollAvailable())
                return
            if (event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown
                    || event.key === Qt.Key_Home || event.key === Qt.Key_End) {
                root.handleComposerScrollKey(event.key)
                event.accepted = true
            }
        }
        onMovementEnded: pageAtEdge()
        Connections {
            target: Messages
            function onRevealRequested(index, messageId) {
                Qt.callLater(() => {
                    if (index >= 0 && index < messages.count)
                        messages.positionViewAtIndex(index, ListView.Center)
                })
            }
        }
        footer: Item {
            width: messages.width
            height: Messages.hasMore ? 48 : 8
            Btn {
                anchors.centerIn: parent
                visible: Messages.hasMore
                enabled: !Messages.loadingMore
                text: Messages.loadingMore ? "Loading older messages…" : "Load older messages"
                variant: "ghost"
                size: 0.75
                onClicked: Messages.loadOlder()
            }
        }

        delegate: Item {
            id: messageRow
            property var rowData: model
            width: messages.width
            implicitHeight: messageContent.implicitHeight

            Rectangle {
                anchors.fill: parent
                visible: Messages.selectMode && Boolean(messageRow.rowData.selected)
                color: Theme.alpha(Theme.primary, 0.14)
            }

            Column {
                id: messageContent
                x: Messages.selectMode ? 44 : 0
                width: parent.width - x
                spacing: 4

                Item {
                    visible: Boolean(messageRow.rowData.showDay)
                    width: parent.width
                    height: visible ? 38 : 0
                    Rectangle {
                        anchors.centerIn: parent
                        width: dayLabel.implicitWidth + 20
                        height: 24
                        radius: 12
                        color: Theme.alpha(Theme.card, 0.9)
                        border.width: 1
                        border.color: Theme.alpha(Theme.border, 0.35)
                        AppText {
                            id: dayLabel
                            anchors.centerIn: parent
                            text: String(messageRow.rowData.dayLabel || "")
                            size: 0.6875
                            medium: true
                            color: Theme.mutedForeground
                        }
                    }
                }

                MessageBubble {
                    width: parent.width
                    message: messageRow.rowData
                }
            }

            Item {
                visible: Messages.selectMode
                x: 14
                width: 20
                height: 20
                anchors.verticalCenter: parent.verticalCenter
                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    border.width: 1
                    border.color: Theme.primary
                    color: messageRow.rowData.selected ? Theme.primary : "transparent"
                    Icon {
                        anchors.centerIn: parent
                        visible: Boolean(messageRow.rowData.selected)
                        name: "check"
                        size: 12
                        color: "#ffffff"
                    }
                }
            }

            MouseArea {
                visible: Messages.selectMode
                anchors.fill: parent
                z: 2
                acceptedButtons: Qt.LeftButton
                cursorShape: Qt.PointingHandCursor
                onClicked: Messages.toggleSelect(String(messageRow.rowData.id || ""))
            }
        }
    }

    Btn {
        id: latestButton
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: 16
        anchors.bottomMargin: 16
        z: 3
        width: 40
        height: 40
        radius: 20
        text: ""
        icon: "chevron-down"
        variant: "outline"
        visible: Messages.model.count > 0 && (Messages.hasMoreNext || !messages.atYEnd)
        enabled: !Messages.loading && !Messages.loadingNewer
        loading: Messages.loadingNewer
        Accessible.role: Accessible.Button
        Accessible.name: "Scroll to newest messages"
        onClicked: Messages.jumpToLatest()
    }

    Column {
        anchors.centerIn: parent
        spacing: 8
        visible: Messages.loading && Messages.model.count === 0
        Spinner { anchors.horizontalCenter: parent.horizontalCenter; size: 22 }
        AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "Loading messages..."; size: 0.75; color: Theme.mutedForeground }
    }

    Connections {
        target: Messages
        function onScrollToBottomRequested() {
            Qt.callLater(() => messages.positionViewAtIndex(0, ListView.End))
        }
    }

    Component.onCompleted: Qt.callLater(() => {
        if (Messages.model.count > 0)
            messages.positionViewAtIndex(0, ListView.End)
    })
}
