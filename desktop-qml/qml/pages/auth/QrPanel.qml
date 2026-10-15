import QtQuick
import QtQuick.Layouts
import "../../ui"

// GPUI `QrCodeView`: 288x288 box (muted, border, 16px radius) holding the loading / error / QR / empty
// state, a "Refresh QR" button and the three numbered steps.
ColumnLayout {
    id: root
    spacing: Theme.rem * 1.5

    Rectangle {
        Layout.alignment: Qt.AlignHCenter
        Layout.preferredWidth: 288; Layout.preferredHeight: 288
        radius: 16
        color: Theme.muted
        border.width: 1; border.color: Theme.border

        // loading (also the "nothing yet" state, with its own copy)
        Column {
            anchors.centerIn: parent
            spacing: Theme.rem * 0.75
            visible: Auth.qrLoading || (!Auth.qrError && Auth.qr === "")
            Spinner { size: 32; anchors.horizontalCenter: parent.horizontalCenter }
            AuthText {
                anchors.horizontalCenter: parent.horizontalCenter
                color: Theme.mutedForeground
                text: Auth.qrLoading ? "Loading QR code..." : "No QR code available yet"
            }
        }
        Column {
            anchors.centerIn: parent
            spacing: Theme.rem
            visible: !Auth.qrLoading && Auth.qrError !== ""
            AuthText { anchors.horizontalCenter: parent.horizontalCenter; text: Auth.qrError; color: Theme.mutedForeground }
            AuthButton { anchors.horizontalCenter: parent.horizontalCenter; text: "Retry"; onClicked: Auth.refreshQr() }
        }
        Image {
            id: qrImg
            anchors.centerIn: parent
            width: 256; height: 256
            sourceSize: Qt.size(256 * Screen.devicePixelRatio, 256 * Screen.devicePixelRatio)
            source: Auth.qrSource
            cache: false
            smooth: true
            visible: !Auth.qrLoading && Auth.qrError === "" && Auth.qrSource !== ""
        }
        AuthText {
            anchors.centerIn: parent
            visible: !Auth.qrLoading && Auth.qrError === "" && Auth.qrInvalid
            text: "Invalid QR code"
        }
    }

    AuthButton {
        Layout.alignment: Qt.AlignHCenter
        text: "Refresh QR"
        onClicked: Auth.refreshQr()
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.maximumWidth: 320
        Layout.alignment: Qt.AlignHCenter
        spacing: Theme.rem * 0.75
        QrStep { Layout.fillWidth: true; num: 1; text: "Open WhatsApp on your phone" }
        QrStep { Layout.fillWidth: true; num: 2; text: "Tap Menu or Settings and select Linked Devices" }
        QrStep { Layout.fillWidth: true; num: 3; text: "Point your phone to scan the QR code" }
    }
}
