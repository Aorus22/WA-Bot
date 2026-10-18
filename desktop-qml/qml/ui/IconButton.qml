import QtQuick

// Square ghost button with an icon; hover fills `muted`.
Rectangle {
    id: root
    property string icon
    property real iconSize: 16
    property real pad: 8
    property bool circular: false
    property color iconColor: Theme.foreground
    property color hoverColor: Theme.muted
    property color activeColor: hoverColor
    property real disabledOpacity: 0.5
    property string tooltip: ""
    property bool active: false
    property bool pointerEnabled: true
    property alias hovered: mouse.containsMouse
    signal clicked(var mouse)
    signal pressed(var mouse)
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.tooltip !== "" ? root.tooltip : root.icon
    Keys.onPressed: event => {
        if (!root.enabled)
            return
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.clicked(null)
            event.accepted = true
        }
    }
    implicitWidth: iconSize + pad * 2
    implicitHeight: iconSize + pad * 2
    radius: root.circular ? Math.min(width, height) / 2 : Theme.rem * 0.375
    color: mouse.containsMouse ? hoverColor : active ? activeColor : activeFocus ? hoverColor : "transparent"
    border.width: activeFocus ? 1 : 0
    border.color: Theme.ring
    opacity: enabled ? 1 : root.disabledOpacity
    Icon { anchors.centerIn: parent; name: root.icon; size: root.iconSize; color: root.iconColor }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.pointerEnabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        acceptedButtons: Qt.LeftButton
        onClicked: m => root.clicked(m)
        onPressed: m => root.pressed(m)
    }
    ToolTipBubble { visible: root.tooltip !== "" && mouse.containsMouse; text: root.tooltip }
}
