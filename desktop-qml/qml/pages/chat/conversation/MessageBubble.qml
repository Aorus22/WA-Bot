import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import "../messages/EmojiData.js" as EmojiData
import "../../../ui"

Item {
    id: root
    objectName: "messageBubbleRoot_" + String(root.message ? root.message.id || "" : "")
    property var message
    readonly property bool contextMenuOpen: messageMenu.visible || textMenu.visible
    property bool pinExpanded: false
    property real contextX: 0
    property real contextY: 0
    property string textMenuUrl: ""
    property string textMenuSelection: ""
    activeFocusOnTab: Messages.selectMode
    Accessible.role: Messages.selectMode ? Accessible.CheckBox : Accessible.Grouping
    Accessible.name: (Messages.selectMode ? "Select message from " : "Message from ")
        + String(root.message ? root.message.senderLabel || (root.mine ? "You" : "Contact") : "")
        + (root.message && root.message.body ? ": " + String(root.message.body).slice(0, 72) : "")
    Accessible.checkable: Messages.selectMode
    Accessible.checked: root.selected
    Accessible.onPressAction: if (Messages.selectMode) Messages.toggleSelect(String(root.message.id || ""))
    readonly property bool mine: message ? Boolean(message.isMine) : false
    readonly property bool selected: message ? Boolean(message.selected) : false
    readonly property bool highlighted: message && String(message.id || "") === Messages.highlightedId
    readonly property bool compactCard: message && (["document", "poll", "location", "contact"].indexOf(String(message.kind || "")) >= 0 || Boolean(message.linkPreview))
    readonly property color bubbleColor: Theme.hasChat
        ? (mine ? Theme.chatBubbleOut : Theme.chatBubbleIn)
        : (Theme.isDark ? (mine ? "#005c4b" : "#202c33") : (mine ? "#dcf8c6" : "#ffffff"))
    readonly property color bubbleTextColor: Theme.hasChat
        ? (mine ? Theme.chatBubbleOutText : Theme.chatBubbleInText)
        : (Theme.isDark ? "#e9edef" : "#303030")
    readonly property real maxBubbleWidth: Math.min(Math.max(0, width - 48 - (!mine && Messages.isGroup ? 42 : 0)), 520)
    readonly property bool richKind: message && (["image", "sticker", "document", "poll", "location", "contact"].indexOf(String(message.kind || "")) >= 0 || Boolean(message.linkPreview))
    readonly property real preferredBubbleWidth: message && message.kind === "image" ? Math.min(mediaContent.imageFrameWidth + 8, maxBubbleWidth)
        : message && message.kind === "sticker" ? Math.min(188, maxBubbleWidth)
        : message && message.kind === "document" ? Math.min(Math.max(238, mediaContent.implicitWidth + 28), maxBubbleWidth)
        : compactCard ? Math.min(288, maxBubbleWidth)
        : richKind ? Math.min(304, maxBubbleWidth)
        : message && message.hasText ? Math.min(Math.max(76, measuredText.implicitWidth + 28), maxBubbleWidth)
        : Math.min(144, maxBubbleWidth)
    implicitHeight: bubbleRow.implicitHeight + 10 + (reactionChips.visible ? reactionChips.implicitHeight + 4 : 0)

    function showContextMenu(mouse) {
        textMenu.close()
        const position = root.mapToItem(Overlay.overlay, mouse.x, mouse.y)
        root.contextX = position.x
        root.contextY = position.y
        root.pinExpanded = false
        messageMenu.popup(position.x + 8, position.y + 8)
    }

    function messageContextItems() {
        const messageId = String(root.message ? root.message.id || "" : "")
        if (root.pinExpanded) {
            return [
                { label: "Choose how long your pin lasts", small: true, semibold: true,
                  textColor: Theme.mutedForeground, disabled: true, action: "pin-title" },
                { label: "24 hours", icon: "clock", action: "pin-86400" },
                { label: "7 days", icon: "clock", action: "pin-604800", highlighted: true },
                { label: "30 days", icon: "clock", action: "pin-2592000" }
            ]
        }
        const items = [
            { label: "Reply", icon: "reply", action: "reply", objectName: "messageReplyAction_" + messageId },
            { label: "React", icon: "smile", action: "react", objectName: "messageReactAction_" + messageId }
        ]
        if (root.mine)
            items.push({ label: "Message info", icon: "eye", action: "info", objectName: "messageInfoAction_" + messageId })
        items.push({ label: "Select", icon: "check", action: "select",
                     objectName: "messageSelectAction_" + messageId })
        items.push({ label: "Forward", icon: "forward", action: "forward", objectName: "messageForwardAction_" + messageId })

        const settled = root.message && !Boolean(root.message.pending) && !messageId.startsWith("temp-")
        if (settled) {
            if (root.message.pinned)
                items.push({ label: "Unpin", icon: "pin", action: "unpin", objectName: "messagePinAction_" + messageId })
            else
                items.push({ label: "Pin", icon: "pin", chevron: true, action: "pin-expand", keepOpen: true,
                             objectName: "messagePinAction_" + messageId })
            items.push({ label: root.message.starred ? "Unstar" : "Star", icon: "star",
                         action: "star", objectName: "messageStarAction_" + messageId })
        }
        if (String(root.message ? root.message.copyText || "" : "") !== "")
            items.push({ label: "Copy", icon: "copy", action: "copy", objectName: "messageCopyAction_" + messageId })
        if (String(root.message ? root.message.mediaUrl || "" : "") !== "")
            items.push({ label: "Save as…", icon: "download", action: "save", objectName: "messageSaveAction_" + messageId })
        if (root.message && root.message.kind === "sticker" && String(root.message.mediaUrl || "") !== "")
            items.push({ label: "Add to favourites", icon: "sticker", action: "favorite", objectName: "messageFavoriteAction_" + messageId })
        if (root.message && root.message.canEdit)
            items.push({ label: "Edit", icon: "edit", action: "edit", objectName: "messageEditAction_" + messageId })
        if (root.message && root.mine && !Boolean(root.message.revokedAt)) {
            items.push({ separator: true })
            items.push({ label: "Delete", icon: "trash", action: "delete",
                         objectName: "messageDeleteAction_" + messageId })
        }
        return items
    }

    function textContextItems() {
        const items = []
        if (textMenuUrl !== "") {
            items.push({ label: "Open in browser", icon: "external-link", action: "open-link", value: textMenuUrl })
            items.push({ label: "Copy link", icon: "link", action: "copy-link", value: textMenuUrl })
        }
        if (textMenuSelection !== "")
            items.push({ label: "Copy", icon: "copy", action: "copy-selection", value: textMenuSelection })
        return items
    }

    function showTextMenu(mouse, url, selection) {
        messageMenu.close()
        textMenuUrl = String(url || "")
        textMenuSelection = String(selection || "")
        const position = root.mapToItem(Overlay.overlay, mouse.x, mouse.y)
        textMenu.popup(position.x + 8, position.y + 8)
    }
    Keys.onSpacePressed: event => {
        if (Messages.selectMode) {
            Messages.toggleSelect(String(root.message.id || ""))
            event.accepted = true
        }
    }

    Text {
        id: measuredText
        visible: false
        text: root.message ? String(root.message.body || "") : ""
        font.pixelSize: Math.round(Theme.rem * 0.875 * Theme.chatScale)
        wrapMode: Text.NoWrap
        textFormat: Text.PlainText
    }

    Row {
        id: reactionChips
        objectName: "messageReactions_" + String(root.message ? root.message.id || "" : "")
        visible: root.message && (root.message.chips || []).length > 0
        width: implicitWidth
        spacing: 4
        x: root.mine ? root.width - width - 24 : Messages.isGroup ? 66 : 24
        y: 0

        Repeater {
            model: root.message ? (root.message.chips || []) : []
            delegate: Rectangle {
                id: reactionChip
                required property var modelData
                objectName: "messageReactionChip_" + String(root.message ? root.message.id || "" : "") + "_" + String(modelData.emoji || "")
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "React with " + String(modelData.emoji || "")
                Accessible.description: modelData.mine ? "Remove your reaction" : "Add your reaction"
                Accessible.onPressAction: Messages.react(String(root.message.id || ""), String(modelData.emoji || ""))
                height: 24
                width: reactionText.implicitWidth + 16
                radius: 12
                color: modelData.mine ? Theme.alpha(Theme.primary, 0.2) : Theme.alpha(Theme.background, 0.4)
                border.width: modelData.mine || activeFocus ? 1 : 0
                border.color: activeFocus ? Theme.ring : Theme.primary
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        Messages.react(String(root.message.id || ""), String(modelData.emoji || ""))
                        event.accepted = true
                    }
                }
                AppText {
                    id: reactionText
                    anchors.centerIn: parent
                    text: String(modelData.emoji || "") + " " + String(modelData.count || 0)
                    size: 0.6875
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: reactionChip.clickedFromPointer()
                }
                function clickedFromPointer() {
                    Messages.react(String(root.message.id || ""), String(modelData.emoji || ""))
                }
            }
        }
    }

    RowLayout {
        id: bubbleRow
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        anchors.topMargin: reactionChips.visible ? reactionChips.implicitHeight + 4 : 0
        spacing: 0

        Item { Layout.fillWidth: root.mine }

        Item {
            visible: !root.mine && Messages.isGroup
            Layout.preferredWidth: 32
            Layout.preferredHeight: 32
            Layout.alignment: Qt.AlignTop
            Avatar {
                visible: root.message && Boolean(root.message.firstInGroup)
                anchors.fill: parent
                chatId: String(root.message ? root.message.senderJid || "" : "")
                name: String(root.message ? root.message.senderLabel || "" : "")
                avatar: String(root.message ? root.message.senderAvatar || "" : "")
                size: 32
            }
        }
        Item {
            visible: !root.mine && Messages.isGroup
            Layout.preferredWidth: 10
            Layout.preferredHeight: 1
        }

        ColumnLayout {
            id: messageColumn
            Layout.maximumWidth: root.maxBubbleWidth
            Layout.preferredWidth: root.preferredBubbleWidth
            Layout.minimumWidth: 52
            Layout.alignment: Qt.AlignTop
            spacing: 4

            AppText {
                visible: root.message && !root.mine && Messages.isGroup && root.message.firstInGroup
                Layout.fillWidth: true
                Layout.leftMargin: 4
                text: root.message ? String(root.message.senderLabel || "") : ""
                size: 0.75
                semibold: true
                color: root.message ? root.message.senderColor : Theme.primary
            }

            Rectangle {
                id: bubble
                Layout.fillWidth: true
            objectName: "messageBubble_" + String(root.message ? root.message.id || "" : "")
            implicitWidth: root.preferredBubbleWidth
            implicitHeight: contentColumn.implicitHeight + 16
            radius: Theme.rem * 0.75
            color: root.message && root.message.kind === "sticker" ? "transparent"
                : root.highlighted ? Theme.alpha(Theme.primary, 0.35) : root.bubbleColor
            border.width: root.highlighted ? 2 : 0
            border.color: Theme.primary

            Column {
                id: contentColumn
                anchors.fill: parent
                anchors.leftMargin: root.message && root.message.kind === "image" ? 4 : 14
                anchors.rightMargin: root.message && root.message.kind === "image" ? 4 : 14
                anchors.topMargin: root.message && root.message.kind === "image" ? 4 : 8
                anchors.bottomMargin: root.message && root.message.kind === "image" ? 4 : 6
                spacing: 5

                Row {
                    visible: root.message ? Boolean(root.message.forwarded) : false
                    spacing: 5
                    Icon { name: "forward"; size: 13; color: Theme.mutedForeground }
                    AppText { text: "Forwarded"; size: 0.6875; color: Theme.mutedForeground }
                }

                Rectangle {
                    visible: root.message && Boolean(root.message.quote)
                    width: parent.width
                    height: visible ? quoteColumn.implicitHeight + 10 : 0
                    radius: 5
                    color: Theme.alpha(Theme.background, 0.3)
                    border.width: 0
                    Rectangle {
                        width: 3
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        radius: 2
                        color: Theme.primary
                    }
                    Column {
                        id: quoteColumn
                        anchors.fill: parent
                        anchors.leftMargin: 11
                        anchors.rightMargin: 8
                        anchors.topMargin: 5
                        anchors.bottomMargin: 5
                        spacing: 2
                        AppText { text: root.message && root.message.quote ? String(root.message.quote.sender || "Message") : "Message"; size: 0.6875; semibold: true; color: Theme.primary }
                        AppText { width: parent.width; text: root.message && root.message.quote ? String(root.message.quote.text || "") : ""; size: 0.75; color: root.bubbleTextColor; elide: Text.ElideRight; maximumLineCount: 2 }
                    }
                }

                MessageMedia {
                    id: mediaContent
                    visible: root.message && root.richKind
                    width: parent.width
                    message: root.message || ({})
                    textColor: root.bubbleTextColor
                }

                TextEdit {
                    id: messageText
                    objectName: "messageText_" + String(root.message ? root.message.id || "" : "")
                    visible: root.message ? Boolean(root.message.hasText) : false
                    width: parent.width
                    text: {
                        if (!root.message) return ""
                        const esc = value => String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;")
                            .replace(/>/g, "&gt;").replace(/\"/g, "&quot;").replace(/'/g, "&#39;")
                        const segments = root.message.segments || []
                        if (!segments.length) return esc(root.message.body || "").replace(/\n/g, "<br/>")
                        return segments.map(segment => {
                            let part = esc(segment.t || "").replace(/\n/g, "<br/>")
                            if (segment.m) part = "<code>" + part + "</code>"
                            if (segment.b) part = "<b>" + part + "</b>"
                            if (segment.i) part = "<i>" + part + "</i>"
                            if (segment.s) part = "<s>" + part + "</s>"
                            if (segment.mention) part = "<font color=\"" + (Theme.isDark ? "#06cf9c" : "#008069") + "\">" + part + "</font>"
                            if (segment.link) part = "<a href=\"" + esc(segment.link) + "\" style=\"color:" + (Theme.isDark ? "#53bdeb" : "#027eb5") + "\">" + part + "</a>"
                            return part
                        }).join("")
                    }
                    font.pixelSize: Math.round(Theme.rem * 0.875 * Theme.chatScale)
                    color: root.bubbleTextColor
                    wrapMode: TextEdit.Wrap
                    textFormat: TextEdit.RichText
                    readOnly: true
                    selectByMouse: true
                    selectionColor: Theme.alpha(Theme.primary, 0.35)
                    selectedTextColor: root.bubbleTextColor
                    onLinkActivated: link => Messages.openUrl(link)
                }

                RowLayout {
                    visible: !(root.message && root.message.kind === "image" && !Boolean(root.message.hasText))
                    width: parent.width
                    spacing: 4
                    Item { Layout.fillWidth: true }
                    Item {
                        id: disappearingNote
                        objectName: "messageDisappearing_" + String(root.message ? root.message.id || "" : "")
                        readonly property var note: root.message ? (root.message.disappearing || null) : null
                        visible: note !== null
                        Layout.preferredWidth: visible ? 14 : 0
                        Layout.preferredHeight: 14
                        Accessible.role: Accessible.StaticText
                        Accessible.name: note ? String(note.tip || "Disappearing message") : ""
                        Icon {
                            anchors.centerIn: parent
                            name: "clock"
                            size: 12
                            color: disappearingNote.note && disappearingNote.note.expired ? Theme.destructive : Theme.alpha(root.bubbleTextColor, 0.72)
                        }
                        MouseArea {
                            id: disappearingTipArea
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.NoButton
                        }
                        ToolTipBubble {
                            visible: disappearingTipArea.containsMouse && disappearingNote.note !== null
                            text: disappearingNote.note ? String(disappearingNote.note.tip || "") : ""
                        }
                    }
                    AppText {
                        visible: root.message ? Boolean(root.message.edited) : false
                        text: "edited"
                        size: 0.625
                        color: Theme.alpha(root.bubbleTextColor, 0.72)
                    }
                    AppText {
                        text: root.message ? String(root.message.timeText || "") : ""
                        size: 0.625
                        color: Theme.alpha(root.bubbleTextColor, 0.72)
                    }
                    Icon {
                        visible: root.mine
                        name: root.message && root.message.failed ? "x"
                            : root.message && (root.message.ticks === "read" || root.message.ticks === "delivered") ? "check-check" : "check"
                        size: root.message && root.message.failed ? 12 : 14
                        color: root.message && root.message.failed ? "#ef4444"
                            : root.message && root.message.ticks === "read" ? "#53bdeb" : Theme.alpha(root.bubbleTextColor, 0.6)
                    }
                }

            }
            }
        }

        Item { Layout.fillWidth: !root.mine }
    }

    MultiEffect {
        x: bubble.mapToItem(root, 0, 0).x
        y: bubble.mapToItem(root, 0, 0).y
        width: bubble.width
        height: bubble.height
        source: bubble
        shadowEnabled: true
        shadowColor: "#33000000"
        shadowBlur: 0.22
        shadowVerticalOffset: 1
        z: -1
    }

    MouseArea {
        id: contextMouse
        anchors.fill: bubbleRow
        acceptedButtons: Qt.RightButton
        z: 20
        onClicked: mouse => {
            const frame = mediaContent.imageFrameItem
            const framePos = frame.mapToItem(root, 0, 0)
            const onImage = frame.visible && mouse.x >= framePos.x && mouse.y >= framePos.y
                && mouse.x <= framePos.x + frame.width && mouse.y <= framePos.y + frame.height
            if (onImage) {
                messageMenu.close()
                textMenu.close()
                const pos = root.mapToItem(Overlay.overlay, mouse.x, mouse.y)
                mediaContent.openImageMenuAt(pos.x, pos.y)
            } else {
                const textPos = messageText.mapFromItem(root, mouse.x, mouse.y)
                const url = messageText.linkAt(textPos.x, textPos.y)
                const selection = String(messageText.selectedText || "")
                if (url !== "" || selection !== "")
                    root.showTextMenu(mouse, url, selection)
                else
                    root.showContextMenu(mouse)
            }
        }
    }

    AppMenu {
        id: messageMenu
        objectName: "messageMenu_" + String(root.message ? root.message.id || "" : "")
        minWidth: 224
        model: root.messageContextItems()
        onClosed: root.pinExpanded = false
        onTriggered: item => {
            const messageId = String(root.message ? root.message.id || "" : "")
            if (item.action === "reply") Composer.startReply(messageId)
            else if (item.action === "react") reactionPicker.openNearContext()
            else if (item.action === "info") Messages.openInfo(messageId)
            else if (item.action === "select") Messages.enterSelect(messageId)
            else if (item.action === "forward") Messages.forward(messageId)
            else if (item.action === "pin-expand") root.pinExpanded = true
            else if (item.action === "unpin") Messages.unpin(messageId)
            else if (String(item.action || "").startsWith("pin-")) Messages.pin(messageId, Number(String(item.action).slice(4)))
            else if (item.action === "star") Messages.toggleStar(messageId)
            else if (item.action === "copy") Messages.copyMessage(messageId)
            else if (item.action === "save") mediaContent.saveFileDialog.open()
            else if (item.action === "favorite") Messages.favoriteSticker(messageId)
            else if (item.action === "edit") Composer.startEdit(messageId)
            else if (item.action === "delete") Messages.deleteMessage(messageId)
        }
    }

    AppMenu {
        id: textMenu
        objectName: "messageTextMenu_" + String(root.message ? root.message.id || "" : "")
        minWidth: 208
        model: root.textContextItems()
        onClosed: {
            root.textMenuUrl = ""
            root.textMenuSelection = ""
        }
        onTriggered: item => {
            if (item.action === "open-link") Messages.openUrl(String(item.value || ""))
            else if (item.action === "copy-link" || item.action === "copy-selection")
                Messages.copyString(String(item.value || ""), "Disalin ke clipboard")
        }
    }

    Popup {
        id: reactionPicker
        objectName: "messageReactionPicker_" + String(root.message ? root.message.id || "" : "")
        parent: Overlay.overlay
        width: 308
        height: Math.min(252, parent ? parent.height - 16 : 252)
        padding: 8
        modal: false
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        function openNearContext() {
            const maxX = Math.max(8, parent.width - width - 8)
            const maxY = Math.max(8, parent.height - height - 8)
            x = Math.max(8, Math.min(root.contextX + 8, maxX))
            y = Math.max(8, Math.min(root.contextY + 8, maxY))
            open()
        }
        background: Rectangle { color: Theme.card; radius: Theme.rem; border.color: Theme.alpha(Theme.border, 0.35) }
        contentItem: GridView {
            id: reactionGrid
            clip: true
            cellWidth: 34
            cellHeight: 34
            model: EmojiData.all
            delegate: Item {
                required property string modelData
                width: reactionGrid.cellWidth
                height: reactionGrid.cellHeight
                property bool hovered: false
                Rectangle { anchors.fill: parent; radius: 8; color: parent.hovered ? Theme.alpha(Theme.muted, 0.6) : "transparent" }
                AppText { anchors.centerIn: parent; text: modelData; size: 1.125 }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onEntered: parent.hovered = true
                    onExited: parent.hovered = false
                    onClicked: {
                        Messages.react(String(root.message.id || ""), modelData)
                        reactionPicker.close()
                    }
                }
            }
        }
    }

    Rectangle {
        id: quickReactionPill
        parent: Overlay.overlay
        visible: messageMenu.visible && !root.pinExpanded && !reactionPicker.visible
        x: Math.max(8, Math.min(root.contextX + 8, parent.width - width - 8))
        y: Math.max(8, root.contextY - height)
        width: quickReactionRow.implicitWidth + 16
        height: 42
        radius: 21
        color: Theme.card
        border.color: Theme.alpha(Theme.border, 0.35)
        z: 81

        Row {
            id: quickReactionRow
            anchors.centerIn: parent
            spacing: 2
            Repeater {
                model: ["👍", "❤️", "😂", "😮", "😢", "🙏"]
                delegate: Rectangle {
                    required property string modelData
                    property bool hovered: false
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: modelData
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                            Messages.react(String(root.message.id || ""), modelData)
                            messageMenu.close()
                            event.accepted = true
                        }
                    }
                    objectName: "messageReactionAction_" + String(root.message ? root.message.id || "" : "") + "_" + modelData
                    width: 34
                    height: 34
                    radius: 17
                    color: hovered ? Theme.alpha(Theme.muted, 0.6) : "transparent"
                    AppText { anchors.centerIn: parent; text: modelData; size: 1.25 }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: parent.hovered = true
                        onExited: parent.hovered = false
                        onClicked: {
                            Messages.react(String(root.message.id || ""), modelData)
                            messageMenu.close()
                        }
                    }
                }
            }
            Rectangle {
                width: 34
                height: 34
                radius: 17
                color: moreMouse.containsMouse ? Theme.alpha(Theme.muted, 0.6) : "transparent"
                Icon { anchors.centerIn: parent; name: "plus"; size: 16; color: Theme.foreground }
                MouseArea {
                    id: moreMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        messageMenu.close()
                        reactionPicker.openNearContext()
                    }
                }
            }
        }
    }

}
