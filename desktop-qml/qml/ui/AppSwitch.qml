import QtQuick

// shadcn Switch.
Rectangle {
    id: root
    property bool checked: false
    signal toggled(bool value)
    implicitWidth: 36; implicitHeight: 20; radius: 10
    color: checked ? Theme.primary : Theme.input
    opacity: enabled ? 1 : 0.5
    Behavior on color { ColorAnimation { duration: Theme.fastMs } }
    Rectangle {
        width: 16; height: 16; radius: 8; y: 2
        x: root.checked ? 18 : 2
        color: root.checked ? Theme.primaryForeground : Theme.background
        Behavior on x { NumberAnimation { duration: Theme.fastMs; easing.type: Easing.OutQuint } }
    }
    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.checked = !root.checked; root.toggled(root.checked) } }
}
