import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"

Button {
    id: root
    property string label
    property string iconName
    property real diameter: 60
    property color buttonColor: Theme.alpha(Theme.foreground, 0.10)
    property color hoverColor: Theme.alpha(Theme.foreground, 0.16)
    property color iconColor: Theme.alpha(Theme.foreground, 0.90)
    property color labelColor: Theme.alpha(Theme.foreground, 0.55)
    property color ringColor: Theme.alpha(Theme.foreground, 0.15)

    implicitWidth: diameter
    implicitHeight: diameter + 25
    padding: 0
    hoverEnabled: true
    Accessible.name: label
    background: Item { }

    contentItem: Column {
        spacing: 8
        Rectangle {
            width: root.diameter
            height: root.diameter
            radius: width / 2
            color: root.hovered ? root.hoverColor : root.buttonColor
            border.width: 1
            border.color: root.ringColor
            Icon {
                anchors.centerIn: parent
                name: root.iconName
                size: 20
                color: root.iconColor
            }
        }
        AppText {
            width: root.diameter
            horizontalAlignment: Text.AlignHCenter
            text: root.label
            size: 0.75
            color: root.labelColor
        }
    }
}
