import QtQuick
import QtQuick.Layouts
import "../../ui"

AppDialog {
    id: root
    objectName: "channelDiscoverDialog"
    title: "Find channel"
    cardWidth: 448

    AppTextField {
        id: linkField
        objectName: "channelDiscoverLink"
        width: parent.width
        leadingIcon: "link"
        placeholder: "https://whatsapp.com/channel/…"
        onTextChanged: Channels.clearPreview()
        onAccepted: if (text.trim() !== "" && !Channels.discovering) Channels.previewChannel(text)
    }

    Row {
        spacing: 8
        Btn {
            objectName: "channelDiscoverPreview"
            text: "Preview"
            variant: "outline"
            loading: Channels.discovering
            enabled: linkField.text.trim() !== "" && !Channels.discovering
            onClicked: Channels.previewChannel(linkField.text)
        }
        AppText {
            anchors.verticalCenter: parent.verticalCenter
            visible: Channels.preview.jid !== ""
            text: String(Channels.preview.subscribersLabel || "") + " subscribers"
            size: 0.75
            color: Theme.mutedForeground
        }
    }

    Rectangle {
        visible: Channels.preview.jid !== ""
        width: parent.width
        implicitHeight: previewColumn.implicitHeight + 20
        radius: Theme.rem * 0.5
        color: Theme.alpha(Theme.muted, 0.55)
        ColumnLayout {
            id: previewColumn
            anchors.fill: parent
            anchors.margins: 10
            spacing: 4
            AppText { Layout.fillWidth: true; text: String(Channels.preview.name || ""); size: 0.875; semibold: true }
            AppText { Layout.fillWidth: true; text: String(Channels.preview.description || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
        }
    }

    footer: [
        Btn { text: "Cancel"; variant: "outline"; onClicked: root.close() },
        Btn {
            objectName: "channelDiscoverFollow"
            text: "Follow"
            variant: "primary"
            loading: Channels.discovering
            enabled: linkField.text.trim() !== "" && !Channels.discovering
            onClicked: Channels.follow(linkField.text)
        }
    ]

    Connections {
        target: Channels
        function onFollowed() {
            linkField.text = ""
            root.close()
        }
    }

    onOpened: linkField.forceActiveFocus()
    onClosed: {
        linkField.text = ""
        Channels.clearPreview()
    }
}
