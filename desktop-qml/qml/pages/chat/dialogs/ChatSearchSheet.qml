import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    property bool open: false
    property bool closing: false
    property string chatId: ""
    signal closed()

    readonly property var results: {
        const term = query.text.trim().toLowerCase()
        if (term === "" || !Messages.model) return []

        const rows = []
        const seen = Object.create(null)

        function add(message) {
            if (!message) return
            const id = String(message.id || "")
            if (id === "" || seen[id]) return
            const content = String(Chats.decode(String(message.content || ""))).toLowerCase()
            if (!content.includes(term)) return
            seen[id] = true
            rows.push(message)
        }

        if (Messages.chatId === root.chatId) {
            const local = Messages.model
            for (let i = 0; i < local.count; ++i)
                add(local.get(i))
        }
        const remote = Chats.searchResults || []
        for (let i = 0; i < remote.length; ++i)
            add(remote[i])

        rows.sort((a, b) => Number(b.timestamp || 0) - Number(a.timestamp || 0))
        return rows
    }

    function searchSnippet(message) {
        const decoded = String(Chats.decode(String(message.content || "")))
        if (decoded.trim() !== "") return decoded
        const type = String(message.type || "Message")
        return "[" + (type !== "" ? type : "Message") + "]"
    }

    visible: (open || closing) && chatId !== ""
    z: open ? 61 : 60

    Timer {
        id: closeAnimationTimer
        interval: 280
        onTriggered: if (!root.open) root.closing = false
    }
    Keys.priority: Keys.BeforeItem
    Keys.onEscapePressed: event => {
        root.close()
        event.accepted = true
    }
    onOpenChanged: {
        if (!open) {
            closing = true
            closeAnimationTimer.restart()
            return
        }
        closeAnimationTimer.stop()
        closing = false
        query.forceActiveFocus()
        if (query.text.trim() !== "") Chats.search(chatId, query.text)
    }
    onChatIdChanged: {
        if (query) query.text = ""
        Chats.clearSearch()
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
                implicitHeight: 48 + query.implicitHeight + 12 + 1
                color: Theme.alpha(Theme.muted, 0.2)
                ColumnLayout {
                    id: searchHeaderContent
                    anchors.fill: parent
                    spacing: 0
                    RowLayout {
                        id: searchHeader
                        Layout.fillWidth: true
                        Layout.preferredHeight: 48
                        Layout.leftMargin: 16
                        Layout.rightMargin: 16
                        spacing: 8
                        Icon { name: "search"; size: 18; color: Theme.primary }
                        AppText { Layout.fillWidth: true; text: "Search Messages"; size: 1; semibold: true }
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

                    AppTextField {
                        id: query
                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        Layout.rightMargin: 12
                        Layout.bottomMargin: 12
                        leadingIcon: "search"
                        placeholder: "Search messages..."
                        clearable: true
                        onTextChanged: Chats.search(root.chatId, text)
                        onAccepted: Chats.search(root.chatId, text)
                    }

                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: resultList
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    model: root.results
                    spacing: 4
                    ScrollBar.vertical: ThinScrollBar {}

                    delegate: Item {
                        id: resultRow
                        required property var modelData
                        width: resultList.width
                        height: resultContent.implicitHeight + 24
                        property bool hovered: false

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
                                    text: Chats.senderLabel(modelData)
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
                                text: root.searchSnippet(modelData)
                                size: 0.75
                                color: Theme.foreground
                                maximumLineCount: 2
                                wrapMode: Text.WordWrap
                                elide: Text.ElideRight
                            }
                            AppText {
                                Layout.fillWidth: true
                                text: Chats.fmtTime(Number(modelData.timestamp || 0))
                                size: 0.75
                                color: Theme.alpha(Theme.mutedForeground, 0.7)
                                horizontalAlignment: Text.AlignRight
                            }
                        }
                        MouseArea {
                            id: resultHit
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onEntered: resultRow.hovered = true
                            onExited: resultRow.hovered = false
                            onClicked: {
                                Chats.openMessage(root.chatId, String(modelData.id || ""))
                                root.close()
                            }
                        }
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 12
                    visible: query.text.trim() === "" && !Chats.searching
                    Rectangle {
                        width: 56
                        height: 56
                        radius: 16
                        color: Theme.alpha(Theme.muted, 0.3)
                        anchors.horizontalCenter: parent.horizontalCenter
                        Icon { anchors.centerIn: parent; name: "search"; size: 26; color: Theme.alpha(Theme.mutedForeground, 0.7) }
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Search for keywords, dates, or phrases"
                        size: 0.75
                        medium: true
                        color: Theme.mutedForeground
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 12
                    visible: query.text.trim() !== "" && root.results.length === 0 && Chats.searching
                    Spinner { anchors.horizontalCenter: parent.horizontalCenter; size: 24 }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Searching history..."
                        size: 0.75
                        color: Theme.mutedForeground
                    }
                }

                Column {
                    anchors.centerIn: parent
                    width: Math.min(264, parent.width - 32)
                    spacing: 12
                    visible: query.text.trim() !== "" && root.results.length === 0 && !Chats.searching
                    Rectangle {
                        width: 56
                        height: 56
                        radius: 16
                        color: Theme.alpha(Theme.muted, 0.3)
                        anchors.horizontalCenter: parent.horizontalCenter
                        Icon { anchors.centerIn: parent; name: "message-square"; size: 26; color: Theme.alpha(Theme.mutedForeground, 0.7) }
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "No results found"
                        size: 0.875
                        semibold: true
                        horizontalAlignment: Text.AlignHCenter
                    }
                    AppText {
                        width: parent.width
                        text: "We couldn't find any messages matching \"" + query.text + "\""
                        size: 0.75
                        color: Theme.mutedForeground
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
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
