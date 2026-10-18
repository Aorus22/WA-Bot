import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import QtQuick.Dialogs
import "../../../ui"

Rectangle {
    id: root
    objectName: "composerBar"
    implicitHeight: content.implicitHeight + 24
    color: "transparent"
    property alias filePickerVisible: fileDialog.visible
    property alias filePickerSelectedFile: fileDialog.selectedFile
    property bool filePickerForceFallback: false
    property string fileKind: "media"
    property alias editorActiveFocus: editor.activeFocus
    signal scrollKeyRequested(int key)

    function chooseAttachment(action) {
        if (action === "media" || action === "document" || action === "audio" || action === "gif") {
            fileKind = action
            fileDialog.open()
        } else if (action === "emoji") {
            composeDialogs.openEmoji(root)
        } else if (action === "poll") {
            composeDialogs.openPoll()
        } else if (action === "location") {
            composeDialogs.openLocation()
        } else if (action === "contact") {
            composeDialogs.openContact()
        } else if (action === "markdown") {
            Composer.toggleMarkdown()
        } else if (action === "stickers") {
            composeDialogs.openStickers(root)
        }
    }

    function openDocumentDialog() {
        chooseAttachment("document")
    }

    function acceptFilePicker() {
        fileDialog.accept()
    }

    function composePillHeight() {
        const rows = Math.max(1, Math.min(6, String(Composer.text || "").split("\n").length))
        return Math.max(46, rows * 20 + 22)
    }

    ColumnLayout {
        id: content
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 8

        RowLayout {
            visible: Boolean(Composer.replyTo && Composer.replyTo.id) || Boolean(Composer.editing && Composer.editing.id)
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 42 : 0
            spacing: 8

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: 6
                color: Theme.alpha(Theme.muted, 0.48)
                Rectangle {
                    width: 3
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    color: Theme.primary
                }
                Column {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 8
                    anchors.topMargin: 5
                    anchors.bottomMargin: 5
                    spacing: 2
                    AppText {
                        text: Composer.editing && Composer.editing.id
                            ? "Editing message"
                            : String(Composer.replyTo.sender || "Reply")
                        size: 0.6875
                        semibold: true
                        color: Theme.primary
                    }
                    AppText {
                        width: parent.width
                        text: Composer.editing && Composer.editing.id
                            ? String(Composer.editing.preview || "")
                            : String(Composer.replyTo.preview || "")
                        size: 0.75
                        color: Theme.mutedForeground
                        elide: Text.ElideRight
                    }
                }
            }

            IconButton {
                icon: "x"
                tooltip: "Cancel"
                iconSize: 14
                pad: 7
                onClicked: Composer.cancel()
            }
        }

        Rectangle {
            id: mentionCard
            visible: Composer.mentionCandidates.length > 0 && Composer.voiceState === "idle"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(240, mentionList.contentHeight + 8)
            radius: Theme.rem * 0.75
            color: Theme.card
            border.width: 1
            border.color: Theme.border

            MultiEffect {
                anchors.fill: mentionCard
                source: mentionCard
                shadowEnabled: true
                shadowColor: "#55000000"
                shadowBlur: 0.4
                shadowVerticalOffset: 5
                z: -1
            }

            ListView {
                id: mentionList
                objectName: "mentionList"
                anchors.fill: parent
                anchors.margins: 4
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: Composer.mentionCandidates
                delegate: Rectangle {
                    id: mentionRow
                    required property var modelData
                    required property int index
                    width: mentionList.width
                    height: 48
                    radius: Theme.rem * 0.5
                    color: mentionMouse.containsMouse ? Theme.alpha(Theme.muted, 0.55)
                        : index === Composer.mentionIndex ? Theme.alpha(Theme.primary, 0.18) : "transparent"
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 12
                        Avatar {
                            Layout.preferredWidth: 32
                            Layout.preferredHeight: 32
                            chatId: String(mentionRow.modelData.jid || "")
                            name: String(mentionRow.modelData.name || "")
                            size: 32
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0
                            AppText {
                                Layout.fillWidth: true
                                text: String(mentionRow.modelData.name || "")
                                size: 0.875
                                color: Theme.foreground
                                elide: Text.ElideRight
                            }
                            AppText {
                                Layout.fillWidth: true
                                text: String(mentionRow.modelData.jid || "")
                                size: 0.75
                                color: Theme.mutedForeground
                                elide: Text.ElideRight
                            }
                        }
                        AppText {
                            visible: Boolean(mentionRow.modelData.isAdmin)
                            text: "admin"
                            size: 0.75
                            color: Theme.primary
                        }
                    }
                    MouseArea {
                        id: mentionMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Composer.insertMention(String(mentionRow.modelData.jid || ""))
                    }
                }
            }
        }

        Rectangle {
            id: inputPill
            objectName: "composerPill"
            Layout.fillWidth: true
            Layout.minimumHeight: 46
            Layout.preferredHeight: root.composePillHeight()
            visible: Composer.voiceState === "idle"
            radius: Theme.rem * 1.5
            color: Theme.alpha(Theme.muted, 0.5)
            border.width: 1
            border.color: Theme.alpha(Theme.border, 0.2)

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4

                IconButton {
                    id: attachButton
                    objectName: "composerAttachButton"
                    icon: "paperclip"
                    tooltip: "Attach or share"
                    iconSize: 20
                    pad: 8
                    radius: width / 2
                    active: composeDialogs.attachMenuVisible
                    activeColor: Theme.alpha(Theme.primary, 0.15)
                    iconColor: active ? Theme.primary : hovered ? Theme.foreground : Theme.mutedForeground
                    hoverColor: Theme.alpha(Theme.muted, 0.65)
                    enabled: !Composer.busy
                    onClicked: composeDialogs.toggleAttachMenu(attachButton)
                }

                IconButton {
                    icon: "sticker"
                    tooltip: "Emoji and stickers"
                    iconSize: 20
                    pad: 8
                    radius: width / 2
                    active: composeDialogs.pickerVisible
                    activeColor: Theme.alpha(Theme.primary, 0.15)
                    iconColor: active ? Theme.primary : hovered ? Theme.foreground : Theme.mutedForeground
                    hoverColor: Theme.alpha(Theme.muted, 0.65)
                    onClicked: composeDialogs.toggleEmoji(root)
                }

                TextArea {
                    id: editor
                    objectName: "composerEditor"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumWidth: 0
                    Layout.preferredWidth: 1
                    Layout.topMargin: 5
                    Layout.bottomMargin: 5
                    verticalAlignment: TextEdit.AlignVCenter
                    color: Theme.foreground
                    selectionColor: Theme.primary
                    selectedTextColor: Theme.primaryForeground
                    placeholderText: Composer.placeholder
                    placeholderTextColor: Theme.mutedForeground
                    font.pixelSize: Math.round(Theme.rem * 0.875)
                    wrapMode: TextEdit.Wrap
                    selectByMouse: true
                    background: Item {}
                    onTextChanged: Composer.setText(text)
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_V && (event.modifiers & Qt.ControlModifier) !== 0
                                && Composer.handlePaste()) {
                            event.accepted = true
                            return
                        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            const shift = (event.modifiers & Qt.ShiftModifier) !== 0
                            const ctrl = (event.modifiers & Qt.ControlModifier) !== 0
                            if (Composer.mentionCandidates.length > 0 && !shift) {
                                Composer.insertHighlightedMention()
                                event.accepted = true
                            } else if (Composer.enterShouldSend(shift, ctrl)) {
                                Composer.send()
                                event.accepted = true
                            }
                        } else if (event.key === Qt.Key_Up && Composer.mentionCandidates.length > 0) {
                            Composer.moveMention(-1)
                            event.accepted = true
                        } else if (event.key === Qt.Key_Down && Composer.mentionCandidates.length > 0) {
                            Composer.moveMention(1)
                            event.accepted = true
                        } else if (event.key === Qt.Key_Escape && Composer.mentionCandidates.length > 0) {
                            Composer.cancelMention()
                            event.accepted = true
                        } else if (event.key === Qt.Key_Escape && Composer.cancel()) {
                            event.accepted = true
                        } else if ((event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown
                                    || ((event.key === Qt.Key_Home || event.key === Qt.Key_End) && editor.text === ""))
                                   && AppState.section === "chat" && Chats.activeChatId !== ""
                                   && !Messages.viewerOpen && Messages.model.count > 0) {
                            root.scrollKeyRequested(event.key)
                            event.accepted = true
                        }
                    }
                }

                IconButton {
                    objectName: "composerVoiceButton"
                    icon: Composer.canSend ? (Composer.editing && Composer.editing.id ? "check" : "send") : "mic"
                    tooltip: Composer.canSend
                        ? (Composer.editing && Composer.editing.id ? "Save edit" : "Send message")
                        : "Record voice message"
                    iconSize: 20
                    pad: 8
                    radius: width / 2
                    color: Composer.canSend ? Theme.primary : "transparent"
                    hoverColor: Composer.canSend ? Theme.alpha(Theme.primary, 0.85) : Theme.alpha(Theme.muted, 0.65)
                    iconColor: Composer.canSend ? Theme.primaryForeground : Theme.mutedForeground
                    enabled: !Composer.busy
                    onClicked: {
                        if (Composer.canSend) Composer.send()
                        else Composer.startVoice()
                        editor.forceActiveFocus()
                    }
                }
            }
        }

        RowLayout {
            visible: Boolean(Composer.pendingAttachment && Composer.pendingAttachment.path && Composer.pendingAttachment.isImage)
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 72 : 0
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Theme.rem * 0.75
                color: Theme.alpha(Theme.muted, 0.42)
                Rectangle {
                    width: 4
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    color: Theme.primary
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12
                    Rectangle {
                        Layout.preferredWidth: 56
                        Layout.preferredHeight: 56
                        radius: Theme.rem * 0.5
                        clip: true
                        color: "transparent"
                        Image {
                            anchors.fill: parent
                            source: String(Composer.pendingAttachment.url || "")
                            asynchronous: true
                            fillMode: Image.PreserveAspectCrop
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        AppText {
                            Layout.fillWidth: true
                            text: "Image ready to send"
                            size: 0.75
                            semibold: true
                            color: Theme.primary
                        }
                        AppText {
                            Layout.fillWidth: true
                            text: String(Composer.pendingAttachment.fileName || "")
                            size: 0.75
                            color: Theme.mutedForeground
                            elide: Text.ElideRight
                        }
                    }
                    IconButton {
                        icon: "x"
                        iconSize: 14
                        pad: 4
                        circular: true
                        tooltip: "Remove attachment"
                        onClicked: Composer.clearPendingAttachment()
                    }
                }
            }
        }

        RowLayout {
            visible: String(Composer.linkPreview.url || "") !== "" && Composer.voiceState === "idle"
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 52 : 0
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: 7
                color: Theme.alpha(Theme.muted, 0.38)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 6
                    AppText { Layout.fillWidth: true; text: String(Composer.linkPreview.title || Composer.linkPreview.host || Composer.linkPreview.url || ""); size: 0.75; semibold: true; elide: Text.ElideRight }
                    IconButton { icon: "x"; tooltip: "Hide preview"; iconSize: 13; pad: 6; onClicked: Composer.dismissLinkPreview() }
                }
            }
        }

        RowLayout {
            visible: Composer.voiceState !== "idle"
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 44 : 0
            spacing: 8
            AppText {
                text: Composer.voiceState === "starting" ? "Opening microphone…" : Composer.voiceTimer
                size: 0.8125
                semibold: true
                color: Theme.primary
            }
            Row {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                spacing: 2
                Repeater {
                    model: Composer.voiceLevels
                    delegate: Rectangle {
                        required property real modelData
                        width: 3
                        height: Math.max(3, Number(modelData) * 26)
                        anchors.verticalCenter: parent.verticalCenter
                        radius: 2
                        color: Theme.primary
                    }
                }
            }
            IconButton { icon: "trash"; tooltip: "Discard recording"; enabled: Composer.voiceState !== "starting"; onClicked: Composer.cancelVoice() }
            IconButton { objectName: "composerSendVoiceButton"; icon: "send"; tooltip: "Send voice message"; enabled: Composer.voiceState === "recording"; active: enabled; onClicked: Composer.sendVoice() }
        }
    }

    Connections {
        target: Composer
        function onTextChanged() {
            if (editor.text !== Composer.text)
                editor.text = Composer.text
        }
        function onTextSetRequested(text) {
            editor.text = text
        }
        function onFocusRequested() {
            editor.forceActiveFocus()
        }
    }

    FileDialog {
        id: fileDialog
        parentWindow: root.Window.window
        options: root.filePickerForceFallback ? FileDialog.DontUseNativeDialog : 0
        title: root.fileKind === "media" ? "Pilih media (gambar, video, audio)"
            : root.fileKind === "document" ? "Pilih dokumen"
            : root.fileKind === "audio" ? "Pilih audio" : "Pilih GIF"
        fileMode: FileDialog.OpenFile
        nameFilters: ["All files (*)"]
        onAccepted: {
            Composer.sendFile(String(selectedFile), root.fileKind)
            root.fileKind = "media"
        }
        onRejected: root.fileKind = "media"
    }

    ComposeDialogs { id: composeDialogs }

    Connections {
        target: composeDialogs
        function onAttachmentRequested(action) { root.chooseAttachment(action) }
        function onEmojiChosen(emoji) {
            const position = editor.cursorPosition
            editor.insert(position, emoji)
            editor.forceActiveFocus()
        }
    }

}
