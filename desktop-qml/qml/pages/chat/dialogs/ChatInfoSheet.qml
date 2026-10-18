import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    objectName: "chatInfoSheet"
    property bool open: false
    property bool closing: false
    property bool fullWidth: false
    property bool openedFullWidth: false
    signal closed()
    property string tab: "info"
    property string mediaTab: "media"
    readonly property var chat: Chats.activeChat
    readonly property var group: Chats.group
    readonly property string chatId: Chats.activeChatId
    property bool confirmLeaveGroup: false
    property var selectedMemberJids: []
    property bool addingMembers: false

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
        closeAnimationTimer.stop()
        closing = false
        openedFullWidth = fullWidth
        confirmLeaveGroup = false
        tab = "info"
    }

    onFullWidthChanged: if (open && fullWidth) openedFullWidth = true

    function activateQuickAction(action) {
        if (action === "voice" || action === "video") {
            const video = action === "video"
            if (root.chat.isGroup)
                Calls.startGroupCall(root.chatId, [], video ? "group_video" : "group_audio")
            else
                Calls.startCall(root.chatId, video ? "video" : "audio")
        } else if (action === "add") {
            root.openAddMembers()
        } else if (action === "search") {
            Chats.requestSheet("search", root.chatId)
        }
    }

    function openAddMembers() {
        addDialog.open()
    }

    function muteSelected(mode) {
        const current = String(root.chat.muteMode || "off")
        if (mode === "off") return current === "off" || current === ""
        if (mode === "forever") return current === "forever" || current === "always"
        // GPUI leaves the 8-hour and 1-week chips unselected for a timed mute.
        return false
    }

    function mediaCountLabel() {
        const media = Chats.infoMedia.length
        const docs = Chats.infoDocs.length
        const links = Chats.infoLinks.length
        const total = media + docs + links
        if (Chats.infoLoading && total === 0) return ""
        return (media >= 30 || docs >= 30 || links >= 30) ? String(total) + "+" : String(total)
    }

    function selectMediaTab(name) {
        root.mediaTab = name
        root.tab = name
    }

    function infoSubtitle() {
        if (root.chat.isGroup) {
            const members = root.groupMemberCount()
            return members > 0 ? "Group · " + members + " members" : "Group"
        }
        return "+" + root.chatId.split("@")[0]
    }

    function groupMemberCount() {
        const participants = root.group.participants || []
        return Math.max(Number(root.group.participantCount || 0), participants.length)
    }

    function addMemberCandidates() {
        // The proxy's stats signal also fires when source chat roles change.
        const chatRowsRevision = ChatList.pinnedCount
        const existing = (root.group.participants || []).map(member => String(member.jid || ""))
        const candidates = []
        const chats = Chats.model
        for (let i = 0; i < chats.count; ++i) {
            const chat = chats.get(i)
            if (!chat || Boolean(chat.isGroup) || Boolean(chat.archived) || existing.indexOf(String(chat.id || "")) >= 0)
                continue
            candidates.push(chat)
        }
        return candidates
    }

    function toggleMember(jid) {
        const selected = root.selectedMemberJids.slice()
        const index = selected.indexOf(jid)
        if (index >= 0)
            selected.splice(index, 1)
        else
            selected.push(jid)
        root.selectedMemberJids = selected
    }

    function openInfoMedia(item) {
        if (!item) return
        const mediaUrl = String(item.mediaUrl || "")
        if (String(item.type || "") === "image")
            Messages.openViewerFromList(root.chatId, Chats.infoMedia, String(item.id || ""))
        else if (mediaUrl !== "")
            Qt.openUrlExternally(Api.mediaUrl(mediaUrl))
    }

    function openInfoDocument(item) {
        const mediaUrl = String((item || {}).mediaUrl || "")
        if (mediaUrl !== "") Qt.openUrlExternally(Api.mediaUrl(mediaUrl))
    }

    function infoLinkRows() {
        const rows = []
        const messages = Chats.infoLinks
        for (let i = 0; i < messages.length; ++i) {
            const message = messages[i]
            const content = String(message.content || "")
            const parts = content.split(/\s+/)
            for (let j = 0; j < parts.length; ++j) {
                const url = parts[j]
                if (url.startsWith("http://") || url.startsWith("https://"))
                    rows.push({ id: String(message.id || i) + ":" + j, content: url, timestamp: message.timestamp })
            }
        }
        return rows
    }

    function infoDate(timestamp) {
        return Chats.fmtDate(Number(timestamp || 0))
    }

    visible: (open || closing) && chatId !== ""
    z: open ? 61 : 60

    Timer {
        id: closeAnimationTimer
        interval: 280
        onTriggered: if (!root.open) root.closing = false
    }

    Rectangle {
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        x: root.fullWidth || root.open ? root.width - width : root.width
        width: root.fullWidth ? parent.width : Math.min(320, Math.max(0, root.width - 20))
        color: Theme.background
        border.width: 0
        z: 1
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
                    AppText {
                        Layout.fillWidth: true
                        text: root.tab === "info" ? (root.chat.isGroup ? "Group info" : "Contact info") : "Media, links and docs"
                        size: 1
                        semibold: true
                    }
                    IconButton {
                        objectName: "chatInfoBack"
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 24
                        icon: root.tab === "info" ? "x" : "arrow-left"
                        iconColor: Theme.mutedForeground
                        hoverColor: Theme.alpha(Theme.muted, 0.65)
                        pad: 4
                        radius: 12
                        tooltip: root.tab === "info" ? "Close" : "Back"
                        Accessible.name: tooltip
                        onClicked: root.tab === "info" ? root.closed() : root.tab = "info"
                    }
                }
            }
            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Flickable {
                    objectName: "chatInfoFlickable"
                    anchors.fill: parent
                    anchors.margins: 16
                    clip: true
                    contentWidth: width
                    contentHeight: body.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds
                    ColumnLayout {
                        id: body
                        width: parent.width
                        spacing: root.tab === "info" ? 12 : 16

                        ColumnLayout {
                            visible: root.tab === "info"
                            Layout.fillWidth: true
                            Layout.topMargin: 8
                            Layout.bottomMargin: 8
                            spacing: 6
                            Avatar {
                                Layout.alignment: Qt.AlignHCenter
                                chatId: root.chatId
                                name: String(root.chat.name || "")
                                avatar: String(root.chat.avatar || "")
                                isGroup: Boolean(root.chat.isGroup)
                                size: 96
                                variant: "tinted"
                            }
                            AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: String(root.chat.name || ""); size: 1.25; semibold: true; elide: Text.ElideRight }
                            AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: root.infoSubtitle(); size: 0.875; color: Theme.mutedForeground; elide: Text.ElideMiddle }
                        }

                        RowLayout {
                            visible: root.tab === "info"
                            Layout.fillWidth: true
                            spacing: 0
                            Item { Layout.fillWidth: true }
                            RowLayout {
                                spacing: 16
                                Repeater {
                                    model: Boolean(root.chat.isGroup)
                                        ? [ { action: "voice", label: "Voice", icon: "phone" },
                                            { action: "video", label: "Video", icon: "video" },
                                            { action: "add", label: "Add", icon: "plus" },
                                            { action: "search", label: "Search", icon: "search" } ]
                                        : [ { action: "voice", label: "Voice", icon: "phone" },
                                            { action: "video", label: "Video", icon: "video" },
                                            { action: "search", label: "Search", icon: "search" } ]
                                    delegate: ColumnLayout {
                                        required property var modelData
                                        Layout.preferredWidth: 48
                                        spacing: 4
                                        IconButton {
                                            objectName: modelData.action === "add" ? "groupAddMemberButton" : "chatInfoAction_" + modelData.action
                                            Layout.alignment: Qt.AlignHCenter
                                            Layout.preferredWidth: 48
                                            Layout.preferredHeight: 48
                                            icon: modelData.icon
                                            iconSize: 20
                                            pad: 14
                                            radius: width / 2
                                            color: Theme.alpha(Theme.muted, 0.5)
                                            hoverColor: Theme.alpha(Theme.muted, 0.85)
                                            Accessible.name: modelData.action === "voice" ? "Start voice call"
                                                : modelData.action === "video" ? "Start video call"
                                                : modelData.action === "add" ? "Add group member" : "Search in conversation"
                                            onClicked: root.activateQuickAction(modelData.action)
                                        }
                                        AppText {
                                            Layout.fillWidth: true
                                            horizontalAlignment: Text.AlignHCenter
                                            text: modelData.label
                                            size: 0.75
                                            color: Theme.mutedForeground
                                        }
                                    }
                                }
                            }
                            Item { Layout.fillWidth: true }
                        }

                        ColumnLayout {
                            visible: root.tab === "info" && Boolean(root.chat.isGroup) && String(root.group.description || "").trim() !== ""
                            Layout.fillWidth: true
                            Layout.leftMargin: 8
                            Layout.rightMargin: 8
                            spacing: 4
                            AppText { Layout.fillWidth: true; text: "Description"; size: 0.75; color: Theme.mutedForeground }
                            AppText { Layout.fillWidth: true; text: String(root.group.description || ""); size: 0.875; color: Theme.foreground; wrapMode: Text.WordWrap }
                        }

                        Rectangle { visible: root.tab === "info"; Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.45) }

                        ColumnLayout {
                            visible: root.tab === "info"
                            Layout.fillWidth: true
                            spacing: 12
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 8
                                Rectangle {
                                    objectName: "chatInfoMediaOpen"
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 36
                                    radius: 8
                                    property bool hovered: false
                                    color: hovered ? Theme.alpha(Theme.muted, 0.45) : "transparent"
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 8
                                        anchors.rightMargin: 8
                                        spacing: 12
                                        Icon { name: "image"; size: 18; color: Theme.mutedForeground }
                                        AppText { Layout.fillWidth: true; text: "Media, links and docs"; size: 0.875; color: Theme.foreground }
                                        AppText { text: root.mediaCountLabel(); size: 0.75; color: Theme.mutedForeground }
                                        Icon { name: "chevron-right"; size: 14; color: Theme.mutedForeground }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        onEntered: parent.hovered = true
                                        onExited: parent.hovered = false
                                        onClicked: root.tab = root.mediaTab
                                    }
                                }
                                Row {
                                    visible: Chats.infoMedia.length > 0
                                    Layout.fillWidth: true
                                    Layout.leftMargin: 8
                                    Layout.rightMargin: 8
                                    spacing: 8
                                    Repeater {
                                        model: Chats.infoMedia.slice(0, 4)
                                        delegate: Rectangle {
                                            required property var modelData
                                            objectName: "chatInfoMediaThumb_" + String(modelData.id || "")
                                            width: 62
                                            height: 62
                                            radius: 8
                                            clip: true
                                            color: Theme.alpha(Theme.muted, 0.5)
                                            border.width: 1
                                            border.color: Theme.alpha(Theme.border, 0.55)
                                            Image {
                                                anchors.fill: parent
                                                visible: String(modelData.type || "") === "image" && String(modelData.mediaUrl || "") !== ""
                                                source: visible ? Api.mediaUrl(String(modelData.mediaUrl || "")) : ""
                                                asynchronous: true
                                                fillMode: Image.PreserveAspectCrop
                                            }
                                            Icon {
                                                anchors.centerIn: parent
                                                visible: String(modelData.type || "") !== "image" || String(modelData.mediaUrl || "") === ""
                                                name: "video"
                                                size: 20
                                                color: Theme.mutedForeground
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                onClicked: root.openInfoMedia(modelData)
                                            }
                                        }
                                    }
                                }
                            }
                            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.3) }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                Rectangle {
                                objectName: "chatInfoStarred"
                                Layout.fillWidth: true
                                Layout.preferredHeight: 40
                                radius: 8
                                property bool hovered: false
                                color: hovered ? Theme.alpha(Theme.muted, 0.45) : "transparent"
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 8
                                    anchors.rightMargin: 8
                                    spacing: 12
                                    Icon { name: "star"; size: 18; color: Theme.mutedForeground }
                                    AppText { Layout.fillWidth: true; text: "Starred messages"; size: 0.875; color: Theme.foreground }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onEntered: parent.hovered = true
                                    onExited: parent.hovered = false
                                    onClicked: Chats.requestSheet("starred", root.chatId)
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 8
                                Layout.rightMargin: 8
                                Layout.topMargin: 8
                                Layout.bottomMargin: 8
                                spacing: 8
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 12
                                    Icon { name: "volume-x"; size: 18; color: Theme.mutedForeground }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 1
                                        AppText { Layout.fillWidth: true; text: "Notifications"; size: 0.875 }
                                        AppText { Layout.fillWidth: true; text: root.chat.muteMode && root.chat.muteMode !== "off" ? "Muted" : "On"; size: 0.75; color: Theme.mutedForeground }
                                    }
                                }
                                Rectangle {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 34
                                    radius: 12
                                    color: Theme.alpha(Theme.muted, 0.5)
                                    border.width: 1
                                    border.color: Theme.alpha(Theme.border, 0.3)
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 4
                                        spacing: 0
                                        Repeater {
                                            model: [
                                                { mode: "off", label: "On", name: "On" },
                                                { mode: "8h", label: "8 hours", name: "8 hours" },
                                                { mode: "1w", label: "1 week", name: "1 week" },
                                                { mode: "forever", label: "Always off", name: "Always off" }
                                            ]
                                            delegate: Rectangle {
                                                required property var modelData
                                                objectName: "chatInfoMute_" + modelData.mode
                                                Layout.fillWidth: true
                                                Layout.fillHeight: true
                                                radius: 8
                                                color: root.muteSelected(modelData.mode) ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                                                Accessible.role: Accessible.RadioButton
                                                Accessible.name: "Notifications: " + modelData.name
                                                Accessible.checkable: true
                                                Accessible.checked: root.muteSelected(modelData.mode)
                                                AppText {
                                                    anchors.centerIn: parent
                                                    text: modelData.label
                                                    size: 0.75
                                                    semibold: root.muteSelected(modelData.mode)
                                                    color: root.muteSelected(modelData.mode) ? Theme.primary : Theme.mutedForeground
                                                }
                                                MouseArea {
                                                    anchors.fill: parent
                                                    onClicked: Chats.setMute(root.chatId, modelData.mode)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 8
                                Layout.rightMargin: 8
                                Layout.topMargin: 8
                                Layout.bottomMargin: 8
                                spacing: 8
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 12
                                    Icon { name: "clock"; size: 18; color: Theme.mutedForeground }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 1
                                        AppText { Layout.fillWidth: true; text: "Disappearing messages"; size: 0.875 }
                                        AppText { Layout.fillWidth: true; text: Chats.disappearingLabel(Number(root.chat.disappearingSecs || 0)); size: 0.75; color: Theme.mutedForeground }
                                    }
                                }
                                Rectangle {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 34
                                    radius: 12
                                    color: Theme.alpha(Theme.muted, 0.5)
                                    border.width: 1
                                    border.color: Theme.alpha(Theme.border, 0.3)
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 4
                                        spacing: 0
                                        Repeater {
                                            model: [
                                                { seconds: 0, label: "Off" },
                                                { seconds: 86400, label: "24 hours" },
                                                { seconds: 604800, label: "7 days" },
                                                { seconds: 7776000, label: "90 days" }
                                            ]
                                            delegate: Rectangle {
                                                required property var modelData
                                                objectName: "chatInfoDisappearing_" + modelData.seconds
                                                Layout.fillWidth: true
                                                Layout.fillHeight: true
                                                radius: 8
                                                color: Number(root.chat.disappearingSecs || 0) === Number(modelData.seconds)
                                                    ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                                                Accessible.role: Accessible.RadioButton
                                                Accessible.name: "Disappearing messages: " + modelData.label
                                                Accessible.checkable: true
                                                Accessible.checked: Number(root.chat.disappearingSecs || 0) === Number(modelData.seconds)
                                                AppText {
                                                    anchors.centerIn: parent
                                                    text: modelData.label
                                                    size: 0.75
                                                    semibold: Number(root.chat.disappearingSecs || 0) === Number(modelData.seconds)
                                                    color: Number(root.chat.disappearingSecs || 0) === Number(modelData.seconds)
                                                        ? Theme.primary : Theme.mutedForeground
                                                }
                                                MouseArea {
                                                    anchors.fill: parent
                                                    onClicked: Chats.setDisappearing(root.chatId, Number(modelData.seconds))
                                                }
                                            }
                                        }
                                    }
                                }
                                AppText {
                                    Layout.fillWidth: true
                                    text: "Messages stay in this app either way; only phones remove them."
                                    size: 0.5625
                                    color: Theme.mutedForeground
                                    wrapMode: Text.WordWrap
                                }
                            }
                                RowLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 8
                                Layout.rightMargin: 8
                                Layout.topMargin: 10
                                Layout.bottomMargin: 10
                                spacing: 12
                                Icon { name: "check-check"; size: 18; color: Theme.mutedForeground }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    AppText { Layout.fillWidth: true; text: "Encryption"; size: 0.875 }
                                    AppText { Layout.fillWidth: true; text: "Messages are end-to-end encrypted."; size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                                }
                            }
                            }
                            Rectangle { visible: Boolean(root.chat.isGroup); Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.3) }
                            ColumnLayout {
                                visible: Boolean(root.chat.isGroup)
                                Layout.fillWidth: true
                                spacing: 4
                                RowLayout {
                                    Layout.fillWidth: true
                                    AppText {
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 8
                                        Layout.rightMargin: 8
                                        text: root.groupMemberCount() > 0
                                            ? String(root.groupMemberCount()) + " members"
                                            : "Members"
                                        size: 0.75
                                        color: Theme.mutedForeground
                                    }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 4
                                    ColumnLayout {
                                        visible: Chats.groupLoading
                                        Layout.fillWidth: true
                                        Layout.topMargin: 24
                                        Layout.bottomMargin: 24
                                        Layout.alignment: Qt.AlignHCenter
                                        spacing: 8
                                        BusyIndicator { running: visible; Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.alignment: Qt.AlignHCenter }
                                        AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "Loading members…"; size: 0.75; color: Theme.mutedForeground }
                                    }
                                    AppText {
                                        visible: !Chats.groupLoading && root.group.participants === undefined
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 8
                                        Layout.rightMargin: 8
                                        Layout.topMargin: 16
                                        Layout.bottomMargin: 16
                                        text: "Group members unavailable"
                                        size: 0.75
                                        color: Theme.mutedForeground
                                    }
                                    ListView {
                                        visible: !Chats.groupLoading && root.group.participants !== undefined
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: contentHeight
                                        clip: false
                                        interactive: false
                                        model: root.group.participants || []
                                        delegate: Item {
                                            required property var modelData
                                            readonly property string memberName: String(modelData.name || "").trim()
                                                || String(modelData.jid || "").split("@")[0]
                                            width: ListView.view.width
                                            height: 52
                                            RowLayout {
                                                anchors.fill: parent
                                                anchors.leftMargin: 8
                                                anchors.rightMargin: 8
                                                spacing: 12
                                                Avatar { Layout.preferredWidth: 36; Layout.preferredHeight: 36; size: 36; chatId: String(modelData.jid || ""); name: memberName }
                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 1
                                                    AppText { Layout.fillWidth: true; text: memberName; size: 0.875; elide: Text.ElideRight }
                                                    AppText { Layout.fillWidth: true; text: String(modelData.jid || ""); size: 0.75; color: Theme.mutedForeground; elide: Text.ElideRight }
                                                }
                                                Rectangle {
                                                    visible: Boolean(modelData.isAdmin)
                                                    Layout.preferredWidth: adminLabel.implicitWidth + 16
                                                    Layout.preferredHeight: adminLabel.implicitHeight + 4
                                                    radius: 4
                                                    color: Theme.alpha(Theme.primary, 0.15)
                                                    AppText { id: adminLabel; anchors.centerIn: parent; text: "Group admin"; size: 0.65625; color: Theme.primary }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ColumnLayout {
                                visible: Boolean(root.chat.isGroup)
                                Layout.fillWidth: true
                                spacing: 12
                                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.3) }
                                Rectangle {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 40
                                    radius: 8
                                    property bool hovered: false
                                    color: hovered ? Theme.alpha(Theme.muted, 0.45) : "transparent"
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 8
                                        anchors.rightMargin: 8
                                        spacing: 12
                                        Icon { name: "user-minus"; size: 18; color: Theme.destructive }
                                        AppText {
                                            Layout.fillWidth: true
                                            text: root.confirmLeaveGroup ? "Click again to leave this group" : "Exit group"
                                            size: 0.875
                                            color: Theme.destructive
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        onEntered: parent.hovered = true
                                        onExited: parent.hovered = false
                                        onClicked: {
                                            if (root.confirmLeaveGroup) {
                                                root.confirmLeaveGroup = false
                                                Chats.leaveGroup(root.chatId)
                                            } else {
                                                root.confirmLeaveGroup = true
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        ColumnLayout {
                            visible: root.tab === "media" || root.tab === "docs" || root.tab === "links"
                            Layout.fillWidth: true
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 32
                                radius: 12
                                color: Theme.alpha(Theme.muted, 0.5)
                                border.width: 1
                                border.color: Theme.alpha(Theme.border, 0.3)
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.margins: 4
                                    spacing: 0
                                    Item { Layout.fillWidth: true }
                                    Rectangle {
                                        objectName: "chatInfoTab_media"
                                        Layout.preferredWidth: mediaTabLabel.implicitWidth + 24
                                        Layout.fillHeight: true
                                        radius: 8
                                        color: root.tab === "media" ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                                        AppText { id: mediaTabLabel; anchors.centerIn: parent; text: "Media"; size: 0.75; semibold: root.tab === "media"; color: root.tab === "media" ? Theme.primary : Theme.mutedForeground }
                                        MouseArea { anchors.fill: parent; onClicked: root.selectMediaTab("media") }
                                    }
                                    Item { Layout.fillWidth: true }
                                    Rectangle {
                                        objectName: "chatInfoTab_docs"
                                        Layout.preferredWidth: docsTabLabel.implicitWidth + 24
                                        Layout.fillHeight: true
                                        radius: 8
                                        color: root.tab === "docs" ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                                        AppText { id: docsTabLabel; anchors.centerIn: parent; text: "Docs"; size: 0.75; semibold: root.tab === "docs"; color: root.tab === "docs" ? Theme.primary : Theme.mutedForeground }
                                        MouseArea { anchors.fill: parent; onClicked: root.selectMediaTab("docs") }
                                    }
                                    Item { Layout.fillWidth: true }
                                    Rectangle {
                                        objectName: "chatInfoTab_links"
                                        Layout.preferredWidth: linksTabLabel.implicitWidth + 24
                                        Layout.fillHeight: true
                                        radius: 8
                                        color: root.tab === "links" ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                                        AppText { id: linksTabLabel; anchors.centerIn: parent; text: "Links"; size: 0.75; semibold: root.tab === "links"; color: root.tab === "links" ? Theme.primary : Theme.mutedForeground }
                                        MouseArea { anchors.fill: parent; onClicked: root.selectMediaTab("links") }
                                    }
                                    Item { Layout.fillWidth: true }
                                }
                            }
                            ColumnLayout {
                                visible: root.tab === "media" && Chats.infoMedia.length === 0
                                Layout.fillWidth: true
                                Layout.topMargin: 32
                                Layout.bottomMargin: 32
                                Layout.alignment: Qt.AlignHCenter
                                spacing: 8
                                Spinner { visible: Chats.infoLoading; size: 24; Layout.alignment: Qt.AlignHCenter }
                                Icon { visible: !Chats.infoLoading; name: "image"; size: 48; color: Theme.alpha(Theme.mutedForeground, 0.4); Layout.alignment: Qt.AlignHCenter }
                                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; visible: Chats.infoLoading; text: "Loading media..."; size: 0.75; color: Theme.mutedForeground }
                                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; visible: !Chats.infoLoading; text: "No media shared yet"; size: 0.875; color: Theme.alpha(Theme.mutedForeground, 0.4) }
                            }
                            GridLayout {
                                visible: root.tab === "media" && Chats.infoMedia.length > 0
                                Layout.fillWidth: true
                                columns: 3
                                columnSpacing: 8
                                rowSpacing: 8
                                Repeater {
                                    model: Chats.infoMedia
                                    delegate: Rectangle {
                                        required property var modelData
                                        objectName: "chatInfoMediaTile_" + String(modelData.id || "")
                                        Layout.preferredWidth: 88
                                        Layout.preferredHeight: 88
                                        radius: 8
                                        clip: true
                                        color: Theme.alpha(Theme.muted, 0.2)
                                        border.width: 1
                                        border.color: Theme.alpha(Theme.border, 0.4)
                                        Image {
                                            anchors.fill: parent
                                            visible: String(modelData.type || "") === "image" && String(modelData.mediaUrl || "") !== ""
                                            source: visible ? Api.mediaUrl(String(modelData.mediaUrl || "")) : ""
                                            asynchronous: true
                                            fillMode: Image.PreserveAspectCrop
                                        }
                                        Icon {
                                            anchors.centerIn: parent
                                            visible: String(modelData.type || "") !== "image"
                                            name: "video"
                                            size: 28
                                            color: Theme.alpha(Theme.primary, 0.4)
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            onClicked: root.openInfoMedia(modelData)
                                        }
                                    }
                                }
                            }
                            ColumnLayout {
                                visible: root.tab === "docs" && Chats.infoDocs.length === 0
                                Layout.fillWidth: true
                                Layout.topMargin: 32
                                Layout.bottomMargin: 32
                                Layout.alignment: Qt.AlignHCenter
                                spacing: 8
                                Icon { name: "file-text"; size: 48; color: Theme.alpha(Theme.mutedForeground, 0.4); Layout.alignment: Qt.AlignHCenter }
                                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "No documents shared yet"; size: 0.875; color: Theme.alpha(Theme.mutedForeground, 0.4) }
                            }
                            ColumnLayout {
                                visible: root.tab === "links" && Chats.infoLinks.length === 0
                                Layout.fillWidth: true
                                Layout.topMargin: 32
                                Layout.bottomMargin: 32
                                Layout.alignment: Qt.AlignHCenter
                                spacing: 8
                                Icon { name: "link"; size: 48; color: Theme.alpha(Theme.mutedForeground, 0.4); Layout.alignment: Qt.AlignHCenter }
                                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "No links shared yet"; size: 0.875; color: Theme.alpha(Theme.mutedForeground, 0.4) }
                            }
                            Repeater {
                                model: root.tab === "docs" ? Chats.infoDocs : root.tab === "links" ? root.infoLinkRows() : []
                                delegate: Rectangle {
                                    required property var modelData
                                    property bool hovered: false
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 64
                                    radius: 12
                                    color: hovered ? Theme.alpha(Theme.muted, 0.5) : "transparent"
                                    border.width: 1
                                    border.color: Theme.alpha(Theme.border, 0.4)
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 12
                                        spacing: 12
                                        Rectangle {
                                            Layout.preferredWidth: 40
                                            Layout.preferredHeight: 40
                                            radius: 8
                                            color: root.tab === "docs" ? "#1a3b82f6" : "#1a22c55e"
                                            Icon {
                                                anchors.centerIn: parent
                                                name: root.tab === "docs" ? "file-text" : "link"
                                                size: 20
                                                color: root.tab === "docs" ? "#2563eb" : "#16a34a"
                                            }
                                        }
                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2
                                            AppText {
                                                Layout.fillWidth: true
                                                text: root.tab === "docs" ? "Document" : String(modelData.content || "")
                                                size: 0.875
                                                semibold: root.tab === "docs"
                                                color: root.tab === "links" ? "#3b82f6" : Theme.foreground
                                                elide: Text.ElideRight
                                            }
                                            AppText { Layout.fillWidth: true; text: root.infoDate(modelData.timestamp); size: 0.75; color: Theme.mutedForeground }
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        onEntered: parent.hovered = true
                                        onExited: parent.hovered = false
                                        onClicked: root.tab === "docs" ? root.openInfoDocument(modelData) : Qt.openUrlExternally(String(modelData.content || ""))
                                    }
                                }
                            }
                        }
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

    AppDialog {
        id: addDialog
        objectName: "groupAddDialog"
        title: ""
        cardWidth: 400
        cardPadding: Theme.rem * 1.25
        bodySpacing: Theme.rem
        cardColor: Theme.card
        cardRadius: Theme.rem
        RowLayout {
            width: parent.width
            AppText { Layout.fillWidth: true; text: "Add members"; size: 1.125; semibold: true }
            IconButton {
                objectName: "groupAddClose"
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
                icon: "x"
                iconSize: 16
                pad: 4
                radius: 12
                tooltip: "Close"
                onClicked: addDialog.close()
            }
        }
        ListView {
            objectName: "groupAddContactList"
            width: parent.width
            height: Math.min(220, contentHeight)
            clip: true
            model: root.addMemberCandidates()
            spacing: 4
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: memberRow
                objectName: "groupAddContact_" + String(modelData.id || "")
                required property var modelData
                width: ListView.view.width
                height: 36
                readonly property string jid: String(modelData.id || "")
                readonly property bool selected: root.selectedMemberJids.indexOf(jid) >= 0
                property bool hovered: false
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: memberRow.hovered ? Theme.alpha(Theme.muted, 0.4)
                        : memberRow.selected ? Theme.alpha(Theme.primary, 0.12) : "transparent"
                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        AppText { Layout.fillWidth: true; text: String(modelData.name || modelData.id || ""); size: 0.875; color: Theme.foreground; elide: Text.ElideRight }
                        Rectangle {
                            Layout.preferredWidth: 20
                            Layout.preferredHeight: 20
                            radius: 4
                            border.width: 1
                            border.color: Theme.primary
                            color: memberRow.selected ? Theme.primary : "transparent"
                            Icon { visible: memberRow.selected; anchors.centerIn: parent; name: "check"; size: 12; color: Theme.primaryForeground }
                        }
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onEntered: memberRow.hovered = true
                    onExited: memberRow.hovered = false
                    onClicked: root.toggleMember(memberRow.jid)
                }
            }
        }
        footer: [
            Btn {
                objectName: "groupAddMemberSave"
                text: root.addingMembers ? "Adding..." : "Add"
                variant: "primary"
                size: 0.75
                horizontalPadding: 16
                implicitHeight: 28
                radius: 12
                boldText: true
                onClicked: {
                    if (root.addingMembers) return
                    root.addingMembers = true
                    Chats.addMembers(root.chatId, root.selectedMemberJids)
                }
            },
            Btn { text: "Cancel"; variant: "outline"; size: 0.75; horizontalPadding: 16; implicitHeight: 28; radius: 12; onClicked: addDialog.close() }
        ]
        onOpened: {
            root.selectedMemberJids = []
            root.addingMembers = false
        }
    }

    Connections {
        target: Chats
        function onAddMembersFinished(success) {
            if (!root.addingMembers) return
            root.addingMembers = false
            if (success) addDialog.close()
        }
    }

}
