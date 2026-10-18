import QtQuick
import QtQuick.Layouts
import "../../../ui"

Item {
    id: root
    objectName: "composeDialogs"
    property var pickerAnchor: null
    readonly property bool attachMenuVisible: attachMenu.visible
    readonly property bool pickerVisible: emojiPicker.visible || stickerPicker.visible
    signal emojiChosen(string emoji)
    signal attachmentRequested(string action)

    function positionPopup(popup, anchorItem) {
        const anchor = anchorItem || pickerAnchor
        if (!anchor) {
            popup.x = 12
            popup.y = Math.max(8, (Overlay.overlay ? Overlay.overlay.height : 720) - popup.height - 72)
            return
        }
        const point = anchor.mapToItem(Overlay.overlay, 0, anchor.height)
        const overlay = Overlay.overlay
        popup.x = Math.max(8, Math.min(point.x + 12, overlay.width - popup.width - 8))
        popup.y = Math.max(8, Math.min(point.y - popup.height - 8, overlay.height - popup.height - 8))
    }

    ListModel {
        id: pollOptions
        ListElement { text: "" }
        ListElement { text: "" }
    }

    function openPoll() {
        pollDialog.open()
    }
    function updateOption(index, value) {
        pollOptions.setProperty(index, "text", value)
    }
    function removeOption(index) {
        if (pollOptions.count > 2) pollOptions.remove(index)
    }
    function pollValues() {
        const values = []
        for (let i = 0; i < pollOptions.count; i++) values.push(pollOptions.get(i).text)
        return values
    }
    function openLocation() {
        locationDialog.open()
    }
    function openContact() {
        contactDialog.open()
    }
    function resetPoll() {
        pollOptions.clear()
        pollOptions.append({ text: "" })
        pollOptions.append({ text: "" })
        question.text = ""
        multi.checked = false
    }
    function resetLocation() {
        latitude.text = ""
        longitude.text = ""
        placeName.text = ""
        address.text = ""
        live.checked = false
    }
    function resetContact() {
        contactName.text = ""
        phone.text = ""
    }
    function openEmoji(anchorItem) {
        if (anchorItem) pickerAnchor = anchorItem
        if (stickerPicker.visible) stickerPicker.close()
        if (attachMenu.visible) attachMenu.close()
        positionPopup(emojiPicker, pickerAnchor)
        emojiPicker.open()
    }
    function openStickers(anchorItem) {
        if (anchorItem) pickerAnchor = anchorItem
        if (emojiPicker.visible) emojiPicker.close()
        if (attachMenu.visible) attachMenu.close()
        positionPopup(stickerPicker, pickerAnchor)
        stickerPicker.open()
    }
    function toggleEmoji(anchorItem) {
        if (emojiPicker.visible || stickerPicker.visible) {
            emojiPicker.close()
            stickerPicker.close()
        } else {
            openEmoji(anchorItem)
        }
    }
    function toggleAttachMenu(anchorItem) {
        if (emojiPicker.visible || stickerPicker.visible) {
            emojiPicker.close()
            stickerPicker.close()
            return
        }
        if (attachMenu.visible) {
            attachMenu.close()
            return
        }
        if (anchorItem) {
            const point = anchorItem.mapToItem(Overlay.overlay, 0, 0)
            attachMenu.popup(point.x - 8, point.y - attachMenu.implicitHeight - 8)
        } else {
            attachMenu.open()
        }
    }
    function openAttachMenu(anchorItem) {
        if (anchorItem) {
            const point = anchorItem.mapToItem(Overlay.overlay, 0, 0)
            attachMenu.popup(point.x - 8, point.y - attachMenu.implicitHeight - 8)
        } else {
            const anchor = parent
            const point = anchor.mapToItem(Overlay.overlay, 0, 0)
            attachMenu.popup(point.x + 12,
                point.y + anchor.height - attachMenu.implicitHeight - 8)
        }
    }
    function switchToEmoji() {
        const anchor = pickerAnchor
        stickerPicker.close()
        Qt.callLater(() => root.openEmoji(anchor))
    }
    function switchToStickers() {
        const anchor = pickerAnchor
        emojiPicker.close()
        Qt.callLater(() => root.openStickers(anchor))
    }

    EmojiPicker {
        id: emojiPicker
        onChosen: emoji => root.emojiChosen(emoji)
        onSwitchToStickers: root.switchToStickers()
    }
    StickerPicker { id: stickerPicker; onSwitchToEmoji: root.switchToEmoji() }
    AppMenu {
        id: attachMenu
        objectName: "attachMenu"
        minWidth: 216
        rowHeight: 34
        smallRowHeight: 28
        horizontalInset: 12
        itemSpacing: 12
        outerVerticalPadding: 4
        hoverOpacity: 0.5
        model: [
            { label: "Emoji", icon: "smile", iconColor: "#eab308", iconSize: 16, action: "emoji" },
            { label: "Sticker", icon: "sticker", iconColor: "#a855f7", iconSize: 16, action: "stickers" },
            { label: "Media", icon: "paperclip", iconColor: "#3b82f6", iconSize: 16, action: "media" },
            { label: "Document", icon: "file-text", iconColor: "#f97316", iconSize: 16, action: "document" },
            { label: "Audio", icon: "mic", iconColor: "#ef4444", iconSize: 16, action: "audio" },
            { label: "GIF", icon: "image-play", iconColor: "#ec4899", iconSize: 16, action: "gif" },
            { label: "Poll", icon: "chart-column", iconColor: "#06b6d4", iconSize: 16, action: "poll" },
            { label: "Location", icon: "map-pin", iconColor: "#10b981", iconSize: 16, action: "location" },
            { label: "Contact", icon: "user", iconColor: "#6366f1", iconSize: 16, action: "contact" },
            { label: "Markdown " + (Composer.markdownMode ? "(on)" : "(off)"), icon: "file-text",
              iconColor: Composer.markdownMode ? Theme.primary : Theme.mutedForeground,
              textColor: Composer.markdownMode ? Theme.primary : Theme.foreground,
              iconSize: 16, action: "markdown" }
        ]
        onTriggered: item => root.attachmentRequested(String(item.action || ""))
    }

    AppDialog {
        id: pollDialog
        title: ""
        cardWidth: 420
        cardPadding: 20
        bodySpacing: 12
        cardColor: Theme.card
        cardRadius: Theme.rem
        overlayColor: Qt.rgba(0, 0, 0, 0.5333)
        closePolicy: Popup.CloseOnEscape
        onOpened: question.forceActiveFocus()
        RowLayout {
            width: parent.width
            AppText { Layout.fillWidth: true; text: "Buat poll"; size: 1.125; semibold: true; font.weight: Font.Bold }
            IconButton {
                icon: "x"; iconSize: 15; pad: 4; circular: true
                iconColor: Theme.mutedForeground; hoverColor: Theme.alpha(Theme.muted, 0.5)
                tooltip: "Tutup"
                onClicked: pollDialog.close()
            }
        }
        AppTextField { id: question; width: parent.width; placeholder: "Pertanyaan" }
        Repeater {
            model: pollOptions
            delegate: RowLayout {
                id: row
                required property int index
                required property string text
                Layout.fillWidth: true
                AppTextField {
                    Layout.fillWidth: true
                    placeholder: "Opsi " + String(index + 1)
                    text: row.text
                    onEdited: root.updateOption(index, text)
                }
                IconButton {
                    visible: pollOptions.count > 2
                    icon: "x"
                    tooltip: "Remove option"
                    onClicked: root.removeOption(index)
                }
            }
        }
        Rectangle {
            width: parent.width
            height: 26
            radius: Theme.rem * 0.5
            color: "transparent"
            border.width: 1
            border.color: Theme.border
            AppText {
                anchors.left: parent.left
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: "+ Tambah opsi"
                size: 0.75
                color: pollOptions.count >= 12 ? Theme.mutedForeground : Theme.primary
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: if (pollOptions.count < 12) pollOptions.append({ text: "" })
            }
        }
        RowLayout {
            id: multi
            property bool checked: false
            Layout.fillWidth: true
            spacing: 8
            Rectangle {
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                radius: Theme.rem * 0.25
                color: multi.checked ? Theme.primary : "transparent"
                border.width: 1
                border.color: multi.checked ? Theme.primary : Theme.border
                Icon { visible: multi.checked; anchors.centerIn: parent; name: "check"; size: 11; color: "#ffffff" }
            }
            AppText { Layout.fillWidth: true; text: "Izinkan jawaban ganda"; size: 0.875 }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: multi.checked = !multi.checked
            }
        }
        footer: [
            Btn { text: "Batal"; variant: "outline"; size: 0.75; onClicked: pollDialog.close() },
            Btn { text: Composer.busy ? "Mengirim…" : "Kirim"; size: 0.75; enabled: !Composer.busy; onClicked: Composer.sendPoll(question.text, root.pollValues(), multi.checked) }
        ]
    }

    AppDialog {
        id: locationDialog
        title: ""
        cardWidth: 420
        cardPadding: 20
        bodySpacing: 12
        cardColor: Theme.card
        cardRadius: Theme.rem
        overlayColor: Qt.rgba(0, 0, 0, 0.5333)
        closePolicy: Popup.CloseOnEscape
        onOpened: latitude.forceActiveFocus()
        RowLayout {
            width: parent.width
            AppText { Layout.fillWidth: true; text: "Bagikan lokasi"; size: 1.125; semibold: true; font.weight: Font.Bold }
            IconButton {
                icon: "x"; iconSize: 15; pad: 4; circular: true
                iconColor: Theme.mutedForeground; hoverColor: Theme.alpha(Theme.muted, 0.5)
                tooltip: "Tutup"
                onClicked: locationDialog.close()
            }
        }
        RowLayout {
            width: parent.width
            AppTextField { id: latitude; Layout.fillWidth: true; placeholder: "Latitude" }
            AppTextField { id: longitude; Layout.fillWidth: true; placeholder: "Longitude" }
        }
        AppTextField { id: placeName; width: parent.width; placeholder: "Nama tempat (opsional)" }
        AppTextField { id: address; width: parent.width; placeholder: "Alamat (opsional)" }
        RowLayout {
            id: live
            property bool checked: false
            width: parent.width
            spacing: 8
            Rectangle {
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                radius: Theme.rem * 0.25
                color: live.checked ? Theme.primary : "transparent"
                border.width: 1
                border.color: live.checked ? Theme.primary : Theme.border
                Icon { visible: live.checked; anchors.centerIn: parent; name: "check"; size: 11; color: "#ffffff" }
            }
            AppText { Layout.fillWidth: true; text: "Bagikan lokasi live"; size: 0.875 }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: live.checked = !live.checked
            }
        }
        footer: [
            Btn { text: "Batal"; variant: "outline"; size: 0.75; onClicked: locationDialog.close() },
            Btn { text: Composer.busy ? "Mengirim…" : "Bagikan"; size: 0.75; enabled: !Composer.busy; onClicked: Composer.sendLocation(latitude.text, longitude.text, placeName.text, address.text, live.checked) }
        ]
    }

    AppDialog {
        id: contactDialog
        title: ""
        cardWidth: 420
        cardPadding: 20
        bodySpacing: 12
        cardColor: Theme.card
        cardRadius: Theme.rem
        overlayColor: Qt.rgba(0, 0, 0, 0.5333)
        closePolicy: Popup.CloseOnEscape
        onOpened: contactName.forceActiveFocus()
        RowLayout {
            width: parent.width
            AppText { Layout.fillWidth: true; text: "Bagikan kontak"; size: 1.125; semibold: true; font.weight: Font.Bold }
            IconButton {
                icon: "x"; iconSize: 15; pad: 4; circular: true
                iconColor: Theme.mutedForeground; hoverColor: Theme.alpha(Theme.muted, 0.5)
                tooltip: "Tutup"
                onClicked: contactDialog.close()
            }
        }
        AppTextField { id: contactName; width: parent.width; placeholder: "Nama kontak" }
        AppTextField { id: phone; width: parent.width; placeholder: "Nomor telepon (mis. +62812…)" }
        footer: [
            Btn { text: "Batal"; variant: "outline"; size: 0.75; onClicked: contactDialog.close() },
            Btn { text: Composer.busy ? "Mengirim…" : "Bagikan"; size: 0.75; enabled: !Composer.busy; onClicked: Composer.sendContact(contactName.text, phone.text) }
        ]
    }

    Connections {
        target: Composer
        function onDialogResult(kind, ok) {
            if (!ok)
                return
            if (kind === "poll") {
                root.resetPoll()
                pollDialog.close()
            } else if (kind === "location") {
                root.resetLocation()
                locationDialog.close()
            } else if (kind === "contact") {
                root.resetContact()
                contactDialog.close()
            }
        }
    }
}
