import QtQuick
import QtQuick.Layouts
import "../../ui"

// GPUI tab switcher: max 320px, p_1, 12px radius, muted fill + border; active tab = card fill + border.
Rectangle {
    id: root
    implicitHeight: row.implicitHeight + 10
    radius: 12
    color: Theme.muted
    border.width: 1; border.color: Theme.border

    RowLayout {
        id: row
        anchors { fill: parent; margins: 5 }
        spacing: 4
        Repeater {
            model: [{ id: "qr", label: "QR Code" }, { id: "phone", label: "Phone" }]
            delegate: Rectangle {
                required property var modelData
                readonly property bool active: Auth.activeTab === modelData.id
                Layout.fillWidth: true
                Layout.fillHeight: true
                implicitHeight: label.implicitHeight + 16 + 2
                radius: 8
                color: active ? Theme.card : "transparent"
                border.width: active ? 1 : 0
                border.color: Theme.border
                AuthText {
                    id: label
                    anchors.centerIn: parent
                    text: modelData.label
                    medium: true
                    color: parent.active ? Theme.cardForeground : Theme.mutedForeground
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Auth.setTab(modelData.id)
                }
            }
        }
    }
}
