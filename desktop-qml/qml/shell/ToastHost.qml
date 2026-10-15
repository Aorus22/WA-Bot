import QtQuick
import "../ui"

// Renders Toast.show(...) top-centre; each dismisses itself after 4s.
Column {
    id: root
    spacing: 8
    anchors.horizontalCenter: parent.horizontalCenter
    y: 16
    z: 1000

    ListModel { id: items }
    Connections {
        target: Toast
        function onShown(id, message, kind) { items.append({ tid: id, message: message, kind: kind }) }
        function onDismissed(id) {
            for (let i = 0; i < items.count; i++) if (items.get(i).tid === id) { items.remove(i); return }
        }
    }

    Repeater {
        model: items
        delegate: Rectangle {
            id: card
            required property int tid
            required property string message
            required property string kind
            width: Math.min(380, label.implicitWidth + 56); height: Math.max(44, label.implicitHeight + 24)
            radius: Theme.rem * 0.5
            color: Theme.card; border.color: Theme.border
            Row {
                anchors.fill: parent; anchors.margins: 12; spacing: 10
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: card.kind === "error" ? "triangle-alert" : card.kind === "success" ? "circle-check" : "info"
                    color: card.kind === "error" ? Theme.destructive : card.kind === "success" ? Theme.primary : Theme.mutedForeground
                }
                AppText { id: label; anchors.verticalCenter: parent.verticalCenter; text: card.message; width: Math.min(implicitWidth, 300); wrapMode: Text.Wrap }
            }
            Timer { running: true; interval: 4000; onTriggered: Toast.dismiss(card.tid) }
            MouseArea { anchors.fill: parent; onClicked: Toast.dismiss(card.tid) }
        }
    }
}
