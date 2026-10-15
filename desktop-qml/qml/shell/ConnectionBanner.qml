import QtQuick
import "../ui"

// 36px bar that slides open while the realtime socket is not connected.
Item {
    id: root
    readonly property string state: AppState.connection
    clip: true
    width: parent ? parent.width : 0
    height: state === "connected" ? 0 : 36
    Behavior on height { NumberAnimation { duration: Theme.baseMs; easing.type: Easing.OutQuint } }
    visible: height > 0.5
    Rectangle {
        width: parent.width; height: 36
        color: "#00000000"
        Rectangle { anchors.fill: parent; color: Theme.accent; visible: true }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        Row {
            anchors.centerIn: parent
            spacing: 8
            Spinner { size: 16; color: Theme.accentForeground; anchors.verticalCenter: parent.verticalCenter }
            AppText {
                anchors.verticalCenter: parent.verticalCenter
                size: 0.75; medium: true; color: Theme.accentForeground
                text: root.state === "connecting" ? "Connecting to WhatsApp backend..."
                    : "Connection lost. Attempting to reconnect (attempt " + AppState.attempt + ")..."
            }
        }
    }
}
