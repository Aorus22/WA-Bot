import QtQuick
import QtQuick.Controls

// Popover menu. Usage:
//   AppMenu { id: m; model: [{label:"Reply", icon:"reply", onTriggered: ...}, {separator:true}, {label:"Delete", danger:true}] }
//   m.popup()  /  m.popup(x, y)   (coords relative to parent)
// Item keys: label, icon, danger, disabled, separator, checked, shortcut, action (string returned via `triggered`).
Popup {
    id: root
    property var model: []
    property int minWidth: 180
    signal triggered(var item)
    padding: 4
    margins: 8
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    function open2(x, y) { if (x !== undefined) { root.x = x; root.y = y } open() }
    implicitWidth: Math.max(minWidth, col.implicitWidth + 8)
    background: Rectangle {
        color: Theme.card; radius: Theme.rem * 0.5
        border.color: Theme.border
    }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.fastMs } }
    exit: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 80 } }
    contentItem: Column {
        id: col
        spacing: 0
        Repeater {
            model: root.model
            delegate: Item {
                required property var modelData
                width: col.width
                height: modelData.separator ? 9 : Math.round(Theme.rem * 2)
                visible: modelData.hidden !== true
                Rectangle {
                    visible: modelData.separator === true
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 1; color: Theme.border
                }
                Rectangle {
                    visible: !modelData.separator
                    anchors.fill: parent
                    radius: Theme.rem * 0.375
                    color: ma.containsMouse && !modelData.disabled ? (modelData.danger ? Theme.alpha(Theme.destructive, 0.15) : Theme.accent) : "transparent"
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 8; anchors.rightMargin: 8
                        spacing: 8
                        Icon { visible: !!modelData.icon; name: modelData.icon || ""; size: 16; anchors.verticalCenter: parent.verticalCenter
                               color: modelData.danger ? Theme.destructive : Theme.foreground; opacity: modelData.disabled ? 0.5 : 1 }
                        AppText { text: modelData.label || ""; anchors.verticalCenter: parent.verticalCenter; opacity: modelData.disabled ? 0.5 : 1
                                  color: modelData.danger ? Theme.destructive : Theme.foreground }
                    }
                    Icon { visible: modelData.checked === true; name: "check"; size: 16; anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter }
                    MouseArea {
                        id: ma
                        anchors.fill: parent; hoverEnabled: true
                        cursorShape: modelData.disabled ? Qt.ArrowCursor : Qt.PointingHandCursor
                        onClicked: {
                            if (modelData.disabled) return
                            root.close()
                            if (modelData.onTriggered) modelData.onTriggered()
                            root.triggered(modelData)
                        }
                    }
                }
            }
        }
    }
}
