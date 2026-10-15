import QtQuick

// Square ghost button with an icon; hover fills `muted`.
Rectangle {
    id: root
    property string icon
    property real iconSize: 16
    property real pad: 8
    property color iconColor: Theme.foreground
    property color hoverColor: Theme.muted
    property string tooltip: ""
    property bool active: false
    property alias hovered: mouse.containsMouse
    signal clicked(var mouse)
    signal pressed(var mouse)
    implicitWidth: iconSize + pad * 2
    implicitHeight: iconSize + pad * 2
    radius: Theme.rem * 0.375
    color: active || mouse.containsMouse ? hoverColor : "transparent"
    opacity: enabled ? 1 : 0.5
    Icon { anchors.centerIn: parent; name: root.icon; size: root.iconSize; color: root.iconColor }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => root.clicked(m)
        onPressed: m => root.pressed(m)
    }
    ToolTipBubble { visible: root.tooltip !== "" && mouse.containsMouse; text: root.tooltip }
}
