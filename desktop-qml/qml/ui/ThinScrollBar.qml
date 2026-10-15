import QtQuick
import QtQuick.Controls

ScrollBar {
    id: bar
    policy: ScrollBar.AsNeeded
    implicitWidth: 8
    minimumSize: 0.08
    contentItem: Rectangle {
        implicitWidth: 6
        radius: 3
        color: Theme.alpha(Theme.mutedForeground, bar.pressed ? 0.6 : bar.hovered ? 0.45 : 0.3)
        opacity: bar.active || bar.hovered ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 200 } }
    }
    background: Item {}
}
