import QtQuick
import QtQuick.Layouts
import "../../ui"

// GPUI Phone tab: pairing by phone number is "coming soon" (the backend has no such endpoint).
ColumnLayout {
    spacing: Theme.rem * 1.5
    Item { Layout.preferredHeight: Theme.rem * 1.5 - parent.spacing; Layout.preferredWidth: 1 }  // py_6
    Rectangle {
        Layout.alignment: Qt.AlignHCenter
        Layout.preferredWidth: 64; Layout.preferredHeight: 64
        radius: 16
        color: Theme.accent
        Icon { anchors.centerIn: parent; name: "network"; size: 32; color: Theme.primary }
    }
    ColumnLayout {
        Layout.alignment: Qt.AlignHCenter
        Layout.fillWidth: true
        spacing: Theme.rem * 0.25
        AuthText { Layout.alignment: Qt.AlignHCenter; text: "Link with phone number"; size: 1.125; semibold: true }
        AuthText {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: "This feature is coming soon to the bot interface."
            color: Theme.mutedForeground
        }
    }
    AuthButton { Layout.alignment: Qt.AlignHCenter; text: "Back to QR code"; onClicked: Auth.setTab("qr") }
    Item { Layout.preferredHeight: Theme.rem * 1.5 - parent.spacing; Layout.preferredWidth: 1 }
}
