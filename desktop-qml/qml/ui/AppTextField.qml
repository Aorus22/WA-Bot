import QtQuick
import QtQuick.Controls

// Single-line themed input with placeholder. Full IME/selection/clipboard come from TextInput.
Rectangle {
    id: root
    property alias text: input.text
    property alias placeholder: ph.text
    property alias echoMode: input.echoMode
    property alias inputMethodHints: input.inputMethodHints
    property alias readOnly: input.readOnly
    property alias maximumLength: input.maximumLength
    property alias validator: input.validator
    property alias cursorPosition: input.cursorPosition
    property string leadingIcon
    property bool clearable: false
    property real size: 0.875
    readonly property alias input: input
    readonly property bool focused: input.activeFocus
    signal accepted()
    signal edited()
    function forceActiveFocus() { input.forceActiveFocus() }
    function selectAll() { input.selectAll() }
    implicitHeight: Math.round(Theme.rem * 2.25)
    implicitWidth: 240
    radius: Theme.rem * 0.375
    color: "transparent"
    border.width: 1
    border.color: input.activeFocus ? Theme.ring : Theme.input
    Row {
        anchors.fill: parent
        anchors.leftMargin: Theme.rem * 0.75
        anchors.rightMargin: Theme.rem * 0.75
        spacing: Theme.rem * 0.5
        Icon { visible: root.leadingIcon !== ""; name: root.leadingIcon; color: Theme.mutedForeground; anchors.verticalCenter: parent.verticalCenter }
        Item {
            width: parent.width - x - (clear.visible ? clear.width + parent.spacing : 0)
            height: parent.height
            TextInput {
                id: input
                anchors.fill: parent
                verticalAlignment: TextInput.AlignVCenter
                color: Theme.foreground
                selectionColor: Theme.primary
                selectedTextColor: Theme.primaryForeground
                font.pixelSize: Math.round(Theme.rem * root.size)
                clip: true
                selectByMouse: true
                onAccepted: root.accepted()
                onTextEdited: root.edited()
            }
            AppText { id: ph; anchors.verticalCenter: parent.verticalCenter; size: root.size; color: Theme.mutedForeground; visible: input.text === "" && !input.preeditText; width: parent.width; elide: Text.ElideRight }
        }
        Icon {
            id: clear
            visible: root.clearable && input.text !== ""
            name: "x"; size: 14; color: Theme.mutedForeground; anchors.verticalCenter: parent.verticalCenter
            MouseArea { anchors.fill: parent; anchors.margins: -4; cursorShape: Qt.PointingHandCursor; onClicked: { input.text = ""; root.edited() } }
        }
    }
}
