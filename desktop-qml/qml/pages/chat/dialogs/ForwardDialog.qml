import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    objectName: "forwardDialog"
    property string sourceChat: ""
    property var messageIds: []
    property var selectedTargets: []
    property bool sending: false

    function openFor(chatId, ids) {
        sourceChat = String(chatId || "")
        messageIds = (ids || []).map(id => String(id)).filter(id => id !== "")
        selectedTargets = []
        query.text = ""
        dialog.open()
    }

    function toggleTarget(chatId) {
        const next = selectedTargets.slice()
        const i = next.indexOf(chatId)
        if (i >= 0) next.splice(i, 1)
        else next.push(chatId)
        selectedTargets = next
    }

    function matchesChat(chat) {
        const q = query.text.trim().toLowerCase()
        const id = String(chat.id || "")
        return q === "" || String(chat.name || "").toLowerCase().includes(q) || id.toLowerCase().includes(q)
    }

    function firstMatchingIndex() {
        for (let i = 0; i < Chats.model.count; i++) {
            if (matchesChat(Chats.model.get(i)))
                return i
        }
        return -1
    }

    function moveCurrent(delta) {
        for (let i = targetList.currentIndex + delta; i >= 0 && i < targetList.count; i += delta) {
            if (matchesChat(Chats.model.get(i))) {
                targetList.currentIndex = i
                targetList.positionViewAtIndex(i, ListView.Contain)
                return
            }
        }
    }

    function toggleCurrent() {
        if (targetList.currentIndex < 0 || targetList.currentIndex >= Chats.model.count)
            return
        const chat = Chats.model.get(targetList.currentIndex)
        if (chat && matchesChat(chat))
            toggleTarget(String(chat.id || ""))
    }

    AppDialog {
        id: dialog
        objectName: "forwardMessageDialog"
        focus: true
        onOpened: query.forceActiveFocus()
        title: "Forward message" + (root.messageIds.length > 1 ? "s" : "")
        description: "Choose one or more chats to receive " + String(root.messageIds.length) + " message" + (root.messageIds.length === 1 ? "." : "s.")
        cardWidth: 440

        AppTextField {
            id: query
            objectName: "forwardChatSearch"
            width: parent.width
            leadingIcon: "search"
            placeholder: "Search chats"
            clearable: true
            nextTabItem: targetList
        }

        ListView {
            id: targetList
            objectName: "forwardTargetsList"
            width: parent.width
            // Keep full chat rows and the footer on screen at the minimum window size.
            height: Math.max(168, Math.floor(Math.min(336,
                (dialog.parent ? dialog.parent.height : 720) - 256) / 56) * 56)
            clip: true
            model: Chats.model
            boundsBehavior: Flickable.StopAtBounds
            activeFocusOnTab: true
            currentIndex: root.firstMatchingIndex()
            Accessible.role: Accessible.List
            Accessible.name: currentIndex >= 0
                ? "Forward targets; current chat " + String(Chats.model.get(currentIndex).name || Chats.model.get(currentIndex).id || "")
                : "No matching chats"
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                    root.moveCurrent(event.key === Qt.Key_Up ? -1 : 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.toggleCurrent()
                    event.accepted = true
                }
            }
            Keys.onTabPressed: event => {
                forwardButton.forceActiveFocus()
                event.accepted = true
            }
            Keys.onBacktabPressed: event => {
                query.forceActiveFocus()
                event.accepted = true
            }
            ScrollBar.vertical: ThinScrollBar {}
            delegate: Item {
                id: row
                required property var model
                required property int index
                readonly property string chatId: String(model.id || "")
                readonly property bool matches: root.matchesChat(model)
                objectName: "forwardTargetRow_" + chatId
                width: parent ? parent.width : 0
                height: matches ? 56 : 0
                visible: matches
                Accessible.role: Accessible.CheckBox
                Accessible.name: "Forward to " + String(model.name || chatId)
                Accessible.description: root.selectedTargets.indexOf(chatId) >= 0 ? "Selected" : "Not selected"
                Accessible.checkable: true
                Accessible.checked: root.selectedTargets.indexOf(chatId) >= 0
                Accessible.onPressAction: root.toggleTarget(chatId)
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: root.selectedTargets.indexOf(row.chatId) >= 0 ? Theme.alpha(Theme.primary, 0.14)
                        : targetList.currentIndex === row.index ? Theme.alpha(Theme.muted, 0.4)
                        : hit.containsMouse ? Theme.alpha(Theme.muted, 0.4) : "transparent"
                    border.width: targetList.activeFocus && targetList.currentIndex === row.index ? 1 : 0
                    border.color: Theme.ring
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 10
                    spacing: 10
                    Avatar {
                        Layout.preferredWidth: 38
                        Layout.preferredHeight: 38
                        size: 38
                        chatId: row.chatId
                        name: String(row.model.name || "")
                        avatar: String(row.model.avatar || "")
                        isGroup: Boolean(row.model.isGroup)
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        AppText { Layout.fillWidth: true; text: String(row.model.name || row.chatId); size: 0.8125; medium: true; elide: Text.ElideRight }
                        AppText { Layout.fillWidth: true; text: String(row.model.lastMsg || row.model.snippet || ""); size: 0.6875; color: Theme.mutedForeground; elide: Text.ElideRight }
                    }
                    AppText { text: root.selectedTargets.indexOf(row.chatId) >= 0 ? "✓" : ""; size: 1; semibold: true; color: Theme.primary }
                }
                MouseArea {
                    id: hit
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.toggleTarget(row.chatId)
                }
            }
        }

        footer: [
            Btn { text: "Cancel"; variant: "outline"; enabled: !root.sending; onClicked: dialog.close() },
            Btn {
                id: forwardButton
                objectName: "forwardSubmitButton"
                text: root.sending ? "Forwarding…" : "Forward" + (root.selectedTargets.length > 0 ? " (" + root.selectedTargets.length + ")" : "")
                variant: "primary"
                enabled: !root.sending && root.selectedTargets.length > 0
                onClicked: {
                    root.sending = true
                    Chats.forwardMessages(root.sourceChat, root.messageIds, root.selectedTargets)
                }
            }
        ]
        onClosed: root.sending = false
    }

    Connections {
        target: Chats
        function onForwardFinished(ok) {
            root.sending = false
            if (ok) dialog.close()
        }
    }
}
