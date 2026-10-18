import QtQuick
import QtQuick.Controls

Slider {
    id: root
    implicitHeight: 20
    leftPadding: 0
    rightPadding: 0
    background: Rectangle {
        x: root.leftPadding; y: root.topPadding + root.availableHeight / 2 - height / 2
        width: root.availableWidth; height: 6; radius: 3; color: Theme.alpha(Theme.foreground, 0.24)
        Rectangle { width: root.visualPosition * parent.width; height: parent.height; radius: 3; color: Theme.foreground }
    }
    handle: Rectangle {
        x: root.leftPadding + root.visualPosition * root.availableWidth - width / 2; y: root.topPadding + root.availableHeight / 2 - height / 2
        width: 16; height: 16; radius: 8; color: Theme.background; border.color: Theme.alpha(Theme.foreground, 0.55); border.width: 1
    }
}
