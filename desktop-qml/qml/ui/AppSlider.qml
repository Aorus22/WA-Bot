import QtQuick
import QtQuick.Controls

Slider {
    id: root
    implicitHeight: 20
    background: Rectangle {
        x: root.leftPadding; y: root.topPadding + root.availableHeight / 2 - height / 2
        width: root.availableWidth; height: 6; radius: 3; color: Theme.muted
        Rectangle { width: root.visualPosition * parent.width; height: parent.height; radius: 3; color: Theme.primary }
    }
    handle: Rectangle {
        x: root.leftPadding + root.visualPosition * (root.availableWidth - width); y: root.topPadding + root.availableHeight / 2 - height / 2
        width: 16; height: 16; radius: 8; color: Theme.background; border.color: Theme.primary; border.width: 2
    }
}
