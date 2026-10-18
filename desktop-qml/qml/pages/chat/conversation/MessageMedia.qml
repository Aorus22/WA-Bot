import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import "../../../ui"

Item {
    id: root
    objectName: "messageMedia_" + messageId
    property var message: ({})
    readonly property var saveFileDialog: saveDialog
    property color textColor: Theme.foreground
    readonly property string kind: String(message.kind || "")
    readonly property string messageId: String(message.id || "")
    readonly property string sourceUrl: String(message.mediaSrc || "")
    readonly property var poll: message.poll || ({})
    readonly property var location: message.location || ({})
    readonly property var contact: message.contact || ({})
    readonly property var preview: message.linkPreview || ({})
    readonly property bool hasCard: ["poll", "location", "contact"].indexOf(kind) >= 0
    readonly property real maxCardWidth: kind === "document" ? 300
        : ["poll", "location", "contact"].indexOf(kind) >= 0 || Boolean(preview.url) ? 260 : 340
    property alias imageFrameItem: imageFrame
    readonly property real imageFrameWidth: {
        if (kind === "sticker") return 160
        const nw = image.implicitWidth
        const nh = image.implicitHeight
        if (nw <= 0 || nh <= 0) return 240
        return Math.max(240, Math.min(340, nw * Math.min(340 / nw, 360 / nh)))
    }
    readonly property real imageFrameHeight: {
        if (kind === "sticker") return 160
        const nw = image.implicitWidth
        const nh = image.implicitHeight
        if (nw <= 0 || nh <= 0) return 180
        return Math.max(180, Math.min(360, nh * Math.min(340 / nw, 360 / nh)))
    }
    implicitWidth: content.implicitWidth
    implicitHeight: content.implicitHeight

    function openImageMenuAt(x, y) { imageMenu.popup(x + 8, y + 8) }

    ColumnLayout {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 7

        Rectangle {
            id: imageFrame
            objectName: "messageImageFrame_" + root.messageId
            visible: (root.kind === "image" || root.kind === "sticker") && root.sourceUrl !== ""
            Layout.preferredWidth: root.imageFrameWidth
            Layout.preferredHeight: root.imageFrameHeight
            Layout.maximumWidth: root.kind === "sticker" ? 160 : 340
            radius: Theme.rem * 0.75
            color: "transparent"
            clip: true

            Image {
                id: image
                anchors.fill: parent
                source: root.sourceUrl
                asynchronous: true
                cache: true
                fillMode: root.kind === "sticker" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                sourceSize: root.kind === "sticker"
                    ? Qt.size(Math.ceil(160 * Screen.devicePixelRatio), Math.ceil(160 * Screen.devicePixelRatio))
                    : Qt.size(Math.ceil(340 * Screen.devicePixelRatio), Math.ceil(360 * Screen.devicePixelRatio))
            }

            Btn {
                objectName: "messageMediaImageOpen_" + root.messageId
                anchors.fill: parent
                enabled: root.sourceUrl !== ""
                accessibleName: "Open " + (root.kind === "sticker" ? "sticker" : "image")
                variant: "link"
                onClicked: Messages.openViewer(root.messageId)
            }

            Rectangle {
                visible: root.kind === "image" && !Boolean(root.message.hasText)
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.rightMargin: 6
                anchors.bottomMargin: 6
                width: imageFooter.implicitWidth + 12
                height: imageFooter.implicitHeight + 4
                radius: 6
                color: "#99000000"
                z: 2
                Row {
                    id: imageFooter
                    anchors.centerIn: parent
                    spacing: 4
                    Icon { visible: Boolean(root.message.disappearing); name: "clock"; size: 11; color: "#bfffffff" }
                    Icon { visible: Boolean(root.message.pinned); name: "pin"; size: 11; color: "#bfffffff" }
                    Icon { visible: Boolean(root.message.starred); name: "star-filled"; size: 11; color: "#bfffffff" }
                    AppText { text: String(root.message.timeText || ""); size: 0.75; color: "#ffffff" }
                    Icon {
                        visible: root.message.isMine
                        name: root.message.failed ? "x"
                            : root.message.ticks === "read" || root.message.ticks === "delivered" ? "check-check" : "check"
                        size: root.message.failed ? 12 : 14
                        color: root.message.failed ? "#ef4444"
                            : root.message.ticks === "read" ? "#53bdeb" : "#ccffffff"
                    }
                }
            }

        }

        Rectangle {
            id: documentCard
            Layout.fillWidth: true
            Layout.minimumWidth: 210
            Layout.maximumWidth: 300
            visible: root.kind === "document"
            implicitHeight: documentContent.implicitHeight + 16
            radius: Theme.rem * 0.75
            color: Theme.isDark ? Theme.alpha("#ffffff", 0.08) : Theme.alpha("#000000", 0.06)

            ColumnLayout {
                id: documentContent
                anchors.fill: parent
                anchors.margins: 8
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Rectangle {
                        Layout.preferredWidth: 40
                        Layout.preferredHeight: 40
                        radius: Theme.rem * 0.5
                        color: Theme.alpha(Theme.primary, 0.15)
                        Icon { anchors.centerIn: parent; name: "file-text"; size: 22; color: Theme.primary }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText {
                            Layout.fillWidth: true
                            text: String(root.message.fileName || "Document")
                            size: 0.75
                            semibold: true
                            color: root.textColor
                            elide: Text.ElideRight
                        }
                        AppText {
                            Layout.fillWidth: true
                            text: String(root.message.fileExt || "FILE")
                            size: 0.625
                            color: Theme.alpha(root.textColor, 0.6)
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    visible: root.sourceUrl !== ""
                    color: Theme.alpha(root.textColor, 0.12)
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    visible: root.sourceUrl !== ""

                    Rectangle {
                        id: openDocumentAction
                        objectName: "messageDocumentOpen_" + root.messageId
                        Layout.fillWidth: true
                        Layout.preferredHeight: 30
                        radius: Theme.rem * 0.5
                        color: openDocumentMouse.containsMouse ? Theme.alpha(Theme.primary, 0.22) : Theme.alpha(Theme.primary, 0.12)
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "Open"
                        Keys.onPressed: event => {
                            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                                Messages.openUrl(root.sourceUrl)
                                event.accepted = true
                            }
                        }
                        Row {
                            anchors.centerIn: parent
                            spacing: 6
                            Icon { name: "external-link"; size: 14; color: Theme.primary; anchors.verticalCenter: parent.verticalCenter }
                            AppText { text: "Open"; size: 0.6875; semibold: true; color: Theme.primary; anchors.verticalCenter: parent.verticalCenter }
                        }
                        MouseArea {
                            id: openDocumentMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Messages.openUrl(root.sourceUrl)
                        }
                    }

                    Rectangle {
                        id: saveDocumentAction
                        objectName: "messageDocumentSave_" + root.messageId
                        Layout.fillWidth: true
                        Layout.preferredHeight: 30
                        radius: Theme.rem * 0.5
                        color: saveDocumentMouse.containsMouse ? Theme.alpha(root.textColor, 0.16) : Theme.alpha(root.textColor, 0.08)
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "Save As"
                        Keys.onPressed: event => {
                            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                                saveDialog.open()
                                event.accepted = true
                            }
                        }
                        Row {
                            anchors.centerIn: parent
                            spacing: 6
                            Icon { name: "download"; size: 14; color: root.textColor; anchors.verticalCenter: parent.verticalCenter }
                            AppText { text: "Save As"; size: 0.6875; semibold: true; color: root.textColor; anchors.verticalCenter: parent.verticalCenter }
                        }
                        MouseArea {
                            id: saveDocumentMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: saveDialog.open()
                        }
                    }
                }
            }
        }

        Rectangle {
            id: pollCard
            Layout.fillWidth: true
            Layout.maximumWidth: root.maxCardWidth
            visible: root.kind === "poll" && Boolean(root.poll.question)
            implicitHeight: pollBody.implicitHeight
            radius: Theme.rem * 0.5
            color: "transparent"
            ColumnLayout {
                id: pollBody
                anchors.fill: parent
                anchors.margins: 0
                spacing: 8
                AppText { Layout.fillWidth: true; text: String(root.poll.question || ""); size: 0.875; semibold: true; color: root.textColor; wrapMode: Text.WordWrap }
                Repeater {
                    model: root.poll.options || []
                    delegate: Rectangle {
                        id: pollOptionRow
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredHeight: 34
                        radius: 7
                        color: "transparent"
                        border.width: pollOptionFocus.activeFocus ? 2 : 1
                        border.color: pollOptionFocus.activeFocus ? Theme.ring
                            : modelData.voted ? Theme.primary : Theme.alpha(Theme.border, 0.65)
                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: parent.width * Math.max(0, Math.min(1, Number(modelData.percent || 0) / 100))
                            radius: 6
                            color: Theme.alpha(Theme.primary, 0.15)
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 9
                            anchors.rightMargin: 8
                            AppText { Layout.fillWidth: true; text: (modelData.voted ? "● " : "○ ") + String(modelData.name || ""); size: 0.8125; color: modelData.voted ? Theme.primary : root.textColor; font.weight: modelData.voted ? Font.Bold : Font.Normal; elide: Text.ElideRight }
                            AppText { text: String(modelData.count || 0); size: 0.6875; color: Theme.alpha(root.textColor, 0.6); font.weight: Font.Bold }
                        }
                        Btn {
                            id: pollOptionFocus
                            objectName: "messagePollOption_" + root.messageId + "_" + String(modelData.name || "")
                            anchors.fill: parent
                            accessibleName: "Vote " + String(modelData.name || "")
                            variant: "link"
                            onClicked: Messages.vote(root.messageId, String(modelData.name || ""))
                        }
                    }
                }
                AppText { Layout.fillWidth: true; text: String(root.poll.footer || ""); size: 0.6875; color: Theme.alpha(root.textColor, 0.6) }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.maximumWidth: root.maxCardWidth
            visible: root.kind === "location" && Boolean(root.location.title)
            implicitHeight: locationBody.implicitHeight
            radius: Theme.rem * 0.5
            color: "transparent"
            ColumnLayout {
                id: locationBody
                anchors.fill: parent
                anchors.margins: 0
                spacing: 7
                RowLayout {
                    Layout.fillWidth: true
                    Rectangle {
                        Layout.preferredWidth: 36
                        Layout.preferredHeight: 36
                        radius: 18
                        color: Theme.alpha(Theme.primary, 0.12)
                        Icon { anchors.centerIn: parent; name: "map-pin"; size: 18; color: Theme.primary }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText { Layout.fillWidth: true; text: String(root.location.title || "Location"); size: 0.8125; semibold: true; color: root.textColor; elide: Text.ElideRight }
                        AppText { Layout.fillWidth: true; text: String(root.location.coords || ""); size: 0.6875; color: Theme.alpha(root.textColor, 0.6) }
                    }
                }
                Btn {
                    objectName: "messageLocationOpenMaps_" + root.messageId
                    Layout.fillWidth: true
                    Layout.leftMargin: 2
                    Layout.rightMargin: 2
                    Layout.preferredHeight: 34
                    text: "Open in Maps"
                    icon: "external-link"
                    variant: "softPrimary"
                    size: 0.6875
                    onClicked: Messages.openUrl(String(root.location.mapsUrl || ""))
                }
            }
        }

        Repeater {
            model: root.kind === "contact" ? (root.contact.entries || []) : []
            delegate: Rectangle {
                required property var modelData
                Layout.fillWidth: true
                Layout.maximumWidth: root.maxCardWidth
                implicitHeight: contactBody.implicitHeight
                radius: Theme.rem * 0.5
                color: "transparent"
                ColumnLayout {
                    id: contactBody
                    anchors.fill: parent
                    anchors.margins: 0
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        Rectangle {
                            Layout.preferredWidth: 40
                            Layout.preferredHeight: 40
                            radius: 20
                            color: Theme.alpha(Theme.primary, 0.12)
                            Icon { anchors.centerIn: parent; name: "user"; size: 20; color: Theme.primary }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            AppText { Layout.fillWidth: true; text: String(modelData.name || "Contact"); size: 0.8125; semibold: true; color: root.textColor; elide: Text.ElideRight }
                            AppText { Layout.fillWidth: true; text: String(modelData.phone || ""); size: 0.6875; color: Theme.alpha(root.textColor, 0.6); elide: Text.ElideRight }
                        }
                    }
                    Btn {
                        objectName: "messageContactOpen_" + root.messageId + "_" + String(modelData.jid || "")
                        Layout.fillWidth: true
                        Layout.leftMargin: 2
                        Layout.rightMargin: 2
                        Layout.preferredHeight: 34
                        visible: String(modelData.jid || "") !== ""
                        text: "Message"
                        icon: "message-square"
                        variant: "softPrimary"
                        size: 0.6875
                        onClicked: Chats.openDirect(String(modelData.jid), String(modelData.name || ""))
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.maximumWidth: root.maxCardWidth
            visible: Boolean(root.preview.url)
            implicitHeight: previewRow.implicitHeight + 16
            radius: Theme.rem * 0.5
            color: Theme.alpha(Theme.background, 0.32)
            RowLayout {
                id: previewRow
                anchors.fill: parent
                anchors.margins: 8
                spacing: 8
                Image {
                    Layout.preferredWidth: 54
                    Layout.preferredHeight: 54
                    visible: String(root.preview.thumb || "") !== ""
                    source: String(root.preview.thumb || "")
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    AppText { Layout.fillWidth: true; text: String(root.preview.title || root.preview.host || "Link"); size: 0.75; semibold: true; maximumLineCount: 2; wrapMode: Text.WordWrap; elide: Text.ElideRight }
                    AppText { Layout.fillWidth: true; text: String(root.preview.description || root.preview.url || ""); size: 0.625; color: Theme.mutedForeground; maximumLineCount: 2; wrapMode: Text.WordWrap; elide: Text.ElideRight }
                }
            }
            Btn {
                id: previewOpen
                objectName: "messageLinkPreviewOpen_" + root.messageId
                anchors.fill: parent
                accessibleName: "Open link preview " + String(root.preview.host || root.preview.url || "")
                variant: "link"
                onClicked: Messages.openUrl(String(root.preview.url || ""))
            }
            Rectangle {
                anchors.fill: parent
                z: 2
                visible: previewOpen.activeFocus
                enabled: false
                color: "transparent"
                border.width: 2
                border.color: Theme.ring
                radius: Theme.rem * 0.5
            }
        }
    }

    AppMenu {
        id: imageMenu
        minWidth: 208
        model: [
            { label: "Copy image", icon: "copy", action: "copy" },
            { label: "Save image as…", icon: "download", action: "save" }
        ]
        onTriggered: item => {
            if (item.action === "copy") Messages.copyImage(root.sourceUrl)
            else if (item.action === "save") saveDialog.open()
        }
    }

    FileDialog {
        id: saveDialog
        objectName: "messageDocumentSaveDialog"
        title: root.kind === "image" || root.kind === "sticker" ? "Save image as…"
            : root.kind === "document" ? "Save document" : "Save as…"
        fileMode: FileDialog.SaveFile
        selectedFile: "file:///" + String(Messages.suggestedName(root.messageId) || "file").replace(/[^a-zA-Z0-9._ -]/g, "_")
        nameFilters: ["All files (*)"]
        onAccepted: Messages.saveMedia(root.messageId, selectedFile.toString())
    }
}
