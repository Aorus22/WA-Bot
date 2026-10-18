import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"
import "."

// Chat list column, following GPUI's Messages header, archive row and 72 px chat rows.
Rectangle {
    id: root
    objectName: "chatSidebar"
    implicitWidth: 360
    color: Theme.glass(Theme.background)
    property bool selectionMode: false
    property var selectedChatIds: []
    property var selectedNewGroupJids: []
    property bool creatingGroup: false
    readonly property int selectedCount: selectedChatIds.length

    function showNewMenu() { sidebarMenu.popupAt(moreMenuButton, true, false) }
    function showGroupDialog() { groupDialog.open() }
    function showJoinDialog() { joinDialog.open() }
    function enterSelectionMode() {
        selectionMode = true
        selectedChatIds = []
    }
    function toggleSelectChat(chatId) {
        const id = String(chatId || "")
        if (id === "") return
        const ids = selectedChatIds.slice()
        const index = ids.indexOf(id)
        if (index >= 0) ids.splice(index, 1)
        else ids.push(id)
        selectedChatIds = ids
    }
    function selectedChatRows() {
        const rows = []
        const chats = Chats.model
        for (let i = 0; i < chats.count; ++i) {
            const chat = chats.get(i)
            if (chat && selectedChatIds.indexOf(String(chat.id || "")) >= 0) rows.push(chat)
        }
        return rows
    }
    function exitSelection() {
        selectionMode = false
        selectedChatIds = []
    }
    function focusChatRow(index) {
        if (index < 0 || index >= ChatList.count)
            return
        conversations.positionViewAtIndex(index, ListView.Contain)
        function focusWhenReady() {
            const row = conversations.itemAtIndex(index)
            if (row && row.focusButton)
                row.focusButton.forceActiveFocus()
            else
                Qt.callLater(focusWhenReady)
        }
        Qt.callLater(focusWhenReady)
    }
    function markSelectedRead() {
        const rows = selectedChatRows()
        rows.forEach(chat => { if (Number(chat.unread || 0) > 0) Chats.markRead(String(chat.id || "")) })
        exitSelection()
    }
    function archiveSelected() {
        const rows = selectedChatRows()
        const allArchived = rows.length > 0 && rows.every(chat => Boolean(chat.archived))
        rows.forEach(chat => {
            if (Boolean(chat.archived) === allArchived)
                Chats.setArchived(String(chat.id || ""), !allArchived)
        })
        exitSelection()
    }
    function pinSelected() {
        const rows = selectedChatRows()
        const allPinned = rows.length > 0 && rows.every(chat => Boolean(chat.pinnedAt))
        rows.forEach(chat => {
            if (Boolean(chat.pinnedAt) === allPinned)
                Chats.setPinned(String(chat.id || ""), !allPinned)
        })
        exitSelection()
    }
    function muteSelected() {
        const rows = selectedChatRows()
        const allMuted = rows.length > 0 && rows.every(chat => chat.muteMode && chat.muteMode !== "off")
        rows.forEach(chat => Chats.setMute(String(chat.id || ""), allMuted ? "off" : "forever"))
        exitSelection()
    }
    function newGroupCandidates() {
        // The proxy's stats signal also fires when source chat roles change.
        const chatRowsRevision = ChatList.pinnedCount
        const rows = []
        const chats = Chats.model
        for (let i = 0; i < chats.count; ++i) {
            const chat = chats.get(i)
            if (chat && !Boolean(chat.isGroup) && !Boolean(chat.archived)) rows.push(chat)
        }
        return rows
    }
    function toggleNewGroupMember(jid) {
        const ids = selectedNewGroupJids.slice()
        const index = ids.indexOf(jid)
        if (index >= 0) ids.splice(index, 1)
        else ids.push(jid)
        selectedNewGroupJids = ids
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            visible: !ChatList.archivedMode && !root.selectionMode
            Layout.fillWidth: true
            Layout.margins: 16
            Layout.bottomMargin: 16
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                AppText {
                    Layout.fillWidth: true
                    Layout.topMargin: 3
                    text: "Messages"
                    size: 1.5
                    semibold: true
                }
                IconButton { icon: "users"; iconSize: 18; tooltip: "New group"; onClicked: groupDialog.open() }
                IconButton { id: moreMenuButton; icon: "more-vertical"; iconSize: 18; tooltip: "More chat actions"; onClicked: root.showNewMenu() }
            }

            AppTextField {
                id: searchField
                objectName: "chatSearchField"
                Layout.fillWidth: true
                Layout.topMargin: 3
                Layout.preferredHeight: 36
                leadingIcon: "search"
                placeholder: "Search conversations..."
                clearable: true
                nextTabItem: archiveButton
                onTextChanged: ChatList.query = text
            }
        }

        RowLayout {
            visible: root.selectionMode
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 16
            Layout.bottomMargin: 8
            spacing: 4

            IconButton { icon: "x"; iconSize: 18; tooltip: "Cancel selection"; onClicked: root.exitSelection() }
            AppText { Layout.fillWidth: true; text: String(root.selectedCount) + " selected"; size: 1; medium: true }
            IconButton { icon: "check-check"; iconSize: 18; tooltip: "Mark as read"; pointerEnabled: root.selectedCount > 0; hoverColor: root.selectedCount > 0 ? Theme.alpha(Theme.muted, 0.65) : "transparent"; opacity: root.selectedCount > 0 ? 1 : 0.35; onClicked: root.markSelectedRead() }
            IconButton { icon: "archive"; iconSize: 18; tooltip: "Archive"; pointerEnabled: root.selectedCount > 0; hoverColor: root.selectedCount > 0 ? Theme.alpha(Theme.muted, 0.65) : "transparent"; opacity: root.selectedCount > 0 ? 1 : 0.35; onClicked: root.archiveSelected() }
            IconButton { icon: "pin"; iconSize: 18; tooltip: "Pin"; pointerEnabled: root.selectedCount > 0; hoverColor: root.selectedCount > 0 ? Theme.alpha(Theme.muted, 0.65) : "transparent"; opacity: root.selectedCount > 0 ? 1 : 0.35; onClicked: root.pinSelected() }
            IconButton { icon: "volume-x"; iconSize: 18; tooltip: "Mute"; pointerEnabled: root.selectedCount > 0; hoverColor: root.selectedCount > 0 ? Theme.alpha(Theme.muted, 0.65) : "transparent"; opacity: root.selectedCount > 0 ? 1 : 0.35; onClicked: root.muteSelected() }
        }

        ColumnLayout {
            visible: ChatList.archivedMode
            Layout.fillWidth: true
            Layout.margins: 16
            Layout.bottomMargin: 10
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                IconButton {
                    icon: "arrow-left"
                    tooltip: "Back"
                    pad: 6
                    onClicked: ChatList.archivedMode = false
                }
                AppText { text: "Archived"; size: 1; medium: true }
            }
            AppText {
                Layout.fillWidth: true
                text: "These chats stay archived when new messages are received. To change this experience, go to Settings > Chats on your phone."
                size: 0.8125
                color: Theme.mutedForeground
                wrapMode: Text.WordWrap
                lineHeight: 1.4
                lineHeightMode: Text.ProportionalHeight
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: Theme.alpha(Theme.foreground, 0.16)
            }
        }

        Item {
            visible: !ChatList.archivedMode && ChatList.query.trim() === ""
            Layout.fillWidth: true
            Layout.topMargin: visible ? 6 : 0
            Layout.preferredHeight: visible ? 52 : 0

            Rectangle {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                radius: Theme.rem * 0.75
                color: archiveButton.hovered ? Theme.alpha(Theme.muted, 0.4) : "transparent"
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12
                    Rectangle {
                        Layout.preferredWidth: 32
                        Layout.preferredHeight: 32
                        radius: width / 2
                        color: Theme.alpha("#00a884", 0.12)
                        Icon { anchors.centerIn: parent; name: "archive"; size: 16; color: "#00a884" }
                    }
                    AppText { Layout.fillWidth: true; text: "Archived"; size: 0.875; semibold: true }
                    AppText {
                        visible: Chats.archivedCount > 0
                        text: String(Chats.archivedCount)
                        size: 0.75
                        semibold: true
                        color: "#00a884"
                    }
                }
                Btn {
                    id: archiveButton
                    objectName: "archiveChatsButton"
                    anchors.fill: parent
                    accessibleName: "Open archived chats"
                    variant: "link"
                    onClicked: ChatList.archivedMode = true
                    Keys.onTabPressed: event => {
                        if (ChatList.count > 0) {
                            root.focusChatRow(0)
                            event.accepted = true
                        }
                    }
                }
            }
        }

        RowLayout {
            visible: !ChatList.archivedMode && ChatList.query.trim() === "" && ChatList.pinnedCount > 0
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 8
            Layout.bottomMargin: 4
            spacing: 8
            Icon { name: "pin"; size: 13; color: Theme.mutedForeground }
            AppText { text: "Pinned"; size: 0.75; semibold: true; color: Theme.mutedForeground }
            AppText { text: String(ChatList.pinnedCount); size: 0.75; color: Theme.alpha(Theme.mutedForeground, 0.7) }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: !ChatList.archivedMode && ChatList.query.trim() === "" ? 9 : 0

            ListView {
                id: conversations
                anchors.fill: parent
                clip: true
                model: ChatList
                spacing: 0
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ThinScrollBar {}
                delegate: ChatListRow {
                    chat: model
                    rowIndex: index
                    selectionMode: root.selectionMode
                    selectionChecked: root.selectedChatIds.indexOf(String(model.id || "")) >= 0
                    onFocusMoveRequested: index => root.focusChatRow(index)
                    onSelectRequested: chatId => root.toggleSelectChat(chatId)
                    width: conversations.width
                    height: 72
                }
            }

            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 32
                spacing: 8
                visible: ChatList.count === 0

                Spinner {
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: Chats.loading && !Chats.loaded
                    size: 22
                }
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: !Chats.loading || Chats.loaded
                    name: "message-square"
                    size: 30
                    color: Theme.mutedForeground
                }
                AppText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Chats.loading && !Chats.loaded ? "Loading conversations..."
                        : ChatList.query.trim() === "" ? "No messages yet" : "No results found"
                    size: 0.875
                    semibold: true
                }
                AppText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: ChatList.query.trim() === "" ? "Incoming messages will appear here."
                        : "Try another search keyword."
                    size: 0.75
                    color: Theme.mutedForeground
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    AppMenu {
        id: sidebarMenu
        minWidth: 252
        model: [
            { label: "New group", icon: "users", action: "group" },
            { label: "Join group via link", icon: "link", action: "join" },
            { separator: true },
            { label: "Starred messages", icon: "star", action: "starred" },
            { label: "Select chats", icon: "check", action: "select" },
            { label: "Mark all as read", icon: "check-check", action: "read-all", primary: true },
            { separator: true },
            { label: "Log out", icon: "arrow-left", action: "logout", danger: true }
        ]
        onTriggered: item => {
            if (item.action === "group") groupDialog.open()
            else if (item.action === "join") joinDialog.open()
            else if (item.action === "starred") Chats.requestSheet("starred", "")
            else if (item.action === "select") root.enterSelectionMode()
            else if (item.action === "read-all") Chats.markAllRead()
            else if (item.action === "logout") logoutDialog.open()
        }
    }

    AppDialog {
        id: logoutDialog
        title: "Log out of WhatsApp?"
        description: "This disconnects this desktop session from WhatsApp."
        cardWidth: 400
        footer: [
            Btn { text: "Cancel"; variant: "outline"; onClicked: logoutDialog.close() },
            Btn { text: "Log out"; variant: "destructive"; onClicked: { System.logout(); logoutDialog.close() } }
        ]
    }

    AppDialog {
        id: groupDialog
        title: ""
        description: ""
        cardWidth: 400
        cardPadding: Theme.rem * 1.25
        bodySpacing: Theme.rem
        cardColor: Theme.card
        cardRadius: Theme.rem
        overlayColor: Qt.rgba(0, 0, 0, 136 / 255)
        closePolicy: Popup.CloseOnEscape
        RowLayout {
            width: parent.width
            AppText { Layout.fillWidth: true; text: "New group"; size: 1.125; semibold: true }
            IconButton {
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
                icon: "x"
                iconColor: Theme.mutedForeground
                hoverColor: Theme.alpha(Theme.muted, 0.65)
                pad: 4
                radius: 12
                tooltip: "Close"
                onClicked: groupDialog.close()
            }
        }
        AppTextField { id: groupName; width: parent.width; placeholder: "Group name..." }
        ListView {
            objectName: "newGroupContactList"
            width: parent.width
            height: Math.min(220, contentHeight)
            clip: true
            model: root.newGroupCandidates()
            spacing: 4
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: newGroupMemberRow
                required property var modelData
                width: ListView.view.width
                height: 36
                readonly property string jid: String(modelData.id || "")
                readonly property bool selected: root.selectedNewGroupJids.indexOf(jid) >= 0
                property bool hovered: false
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: newGroupMemberRow.hovered ? Theme.alpha(Theme.muted, 0.4)
                        : newGroupMemberRow.selected ? Theme.alpha(Theme.primary, 0.12) : "transparent"
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    AppText {
                        Layout.fillWidth: true
                        text: String(modelData.name || modelData.id || "")
                        size: 0.875
                        color: Theme.foreground
                        elide: Text.ElideRight
                    }
                    Rectangle {
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 20
                        radius: 4
                        border.width: 1
                        border.color: Theme.primary
                        color: newGroupMemberRow.selected ? Theme.primary : "transparent"
                        Icon { visible: newGroupMemberRow.selected; anchors.centerIn: parent; name: "check"; size: 12; color: Theme.primaryForeground }
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onEntered: newGroupMemberRow.hovered = true
                    onExited: newGroupMemberRow.hovered = false
                    onClicked: root.toggleNewGroupMember(newGroupMemberRow.jid)
                }
            }
        }
        footer: [
            Btn { text: root.creatingGroup ? "Creating..." : "Create Group"; variant: "primary"; size: 0.75; horizontalPadding: 16; implicitHeight: 28; radius: 12; boldText: true; onClicked: { root.creatingGroup = true; Chats.createGroup(groupName.text, root.selectedNewGroupJids) } },
            Btn { text: "Cancel"; variant: "outline"; size: 0.75; horizontalPadding: 16; implicitHeight: 28; radius: 12; onClicked: groupDialog.close() }
        ]
        onOpened: groupName.forceActiveFocus()
    }

    AppDialog {
        id: joinDialog
        title: "Join a group"
        description: "Paste the WhatsApp group invite link."
        cardWidth: 420
        AppTextField { id: groupLink; width: parent.width; placeholder: "https://chat.whatsapp.com/…" }
        footer: [
            Btn { text: "Cancel"; variant: "outline"; onClicked: joinDialog.close() },
            Btn { text: "Join group"; enabled: groupLink.text.trim() !== ""; onClicked: Chats.joinGroup(groupLink.text) }
        ]
        onOpened: groupLink.forceActiveFocus()
    }

    Connections {
        target: Chats
        function onCreateGroupFinished(ok, jid) {
            root.creatingGroup = false
            if (ok) {
                root.selectedNewGroupJids = []
                groupName.text = ""
                groupDialog.close()
            }
        }
        function onJoinGroupFinished(ok) {
            if (ok) joinDialog.close()
        }
    }
}
