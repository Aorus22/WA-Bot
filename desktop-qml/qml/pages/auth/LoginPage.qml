import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"

// GPUI `LoginView` (web LoginPage.tsx): header, QR/Phone tabs, card, footer, centred in a column of at most
// 512px; scrolls when the window is shorter than the column. Shown by PageHost in place of the WhatsApp pages
// while AppState.unlinked.
Rectangle {
    id: root
    color: Theme.glass(Theme.background)

    Flickable {
        id: fl
        anchors.fill: parent
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: width
        contentHeight: Math.max(height, col.implicitHeight + 32)
        ScrollBar.vertical: ThinScrollBar {}

        ColumnLayout {
            id: col
            width: Math.min(512, fl.width - 32)
            x: (fl.width - width) / 2
            y: Math.max(16, (fl.contentHeight - implicitHeight) / 2)
            spacing: Theme.rem * 1.5

            // Header
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Theme.rem * 0.25
                Rectangle {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: 56; Layout.preferredHeight: 56
                    Layout.bottomMargin: Theme.rem * 0.5
                    radius: 16
                    color: Theme.accent
                    Icon { anchors.centerIn: parent; name: "inbox"; size: 28; color: Theme.primary }
                }
                AuthText { Layout.alignment: Qt.AlignHCenter; text: "Link your device"; size: 1.5; font.weight: Font.Bold }
                AuthText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: "Scan the QR code with your WhatsApp to connect"
                    color: Theme.mutedForeground
                }
            }

            LoginTabs {
                Layout.fillWidth: true
                Layout.maximumWidth: 320
                Layout.alignment: Qt.AlignHCenter
            }

            // Main card
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: (Auth.activeTab === "qr" ? qrPanel.implicitHeight : phoneTab.implicitHeight) + Theme.rem * 3 + 2
                radius: 16
                color: Theme.card
                border.width: 1; border.color: Theme.border
                QrPanel {
                    id: qrPanel
                    visible: Auth.activeTab === "qr"
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: Theme.rem * 1.5 + 1 }
                }
                PhoneTab {
                    id: phoneTab
                    visible: Auth.activeTab === "phone"
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: Theme.rem * 1.5 + 1 }
                }
            }

            // Footer
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: Theme.rem * 0.5
                Icon { name: "circle-check"; size: 14; color: Theme.mutedForeground }
                AuthText { text: "End-to-end encrypted"; size: 0.75; color: Theme.mutedForeground }
            }
        }
    }
}
