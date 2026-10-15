import QtQuick
import "../../ui"

// gpui-component default `Button`: 32px high, 10px side padding, theme radius, 1px `input` border,
// dark: input@30% fill (hover 50%, press 70%), light: background fill; 14px label.
Rectangle {
    id: root
    property string text
    property bool loading: false
    property alias hovered: mouse.containsMouse
    signal clicked()

    implicitHeight: 32
    implicitWidth: row.implicitWidth + 20 + 2
    radius: Theme.rem * 0.375
    border.width: 1
    border.color: Theme.input
    color: mouse.pressed ? Theme.alpha(Theme.input, 0.7)
        : mouse.containsMouse ? Theme.alpha(Theme.input, 0.5)
        : Theme.isDark ? Theme.alpha(Theme.input, 0.3) : Theme.background
    opacity: enabled ? 1 : 0.5
    activeFocusOnTab: true
    Keys.onReturnPressed: if (enabled && !loading) root.clicked()
    Keys.onSpacePressed: if (enabled && !loading) root.clicked()
    Rectangle { anchors.fill: parent; radius: parent.radius; color: "transparent"; border.width: 2; border.color: Theme.alpha(Theme.ring, 0.5); visible: root.activeFocus; anchors.margins: -1 }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Spinner { visible: root.loading; size: 16; anchors.verticalCenter: parent.verticalCenter }
        AppText { text: root.text; anchors.verticalCenter: parent.verticalCenter }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.enabled && !root.loading
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
