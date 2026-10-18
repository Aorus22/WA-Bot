import QtQuick
import QtQuick.Controls
import QtQuick.Effects

// Popover menu. Usage:
//   AppMenu { id: m; model: [{label:"Reply", icon:"reply", onTriggered: ...}, {separator:true}, {label:"Delete", danger:true}] }
//   m.popup()  /  m.popup(x, y)   (coords relative to parent)
// Item keys: label, icon, danger, disabled, separator, checked, shortcut, action (string returned via `triggered`).
Popup {
    id: root
    parent: Overlay.overlay
    property var model: []
    property int minWidth: 180
    property int rowHeight: 40
    property int smallRowHeight: 32
    property int horizontalInset: 20
    property int itemSpacing: 16
    property int outerVerticalPadding: 8
    property real hoverOpacity: 0.6
    signal triggered(var item)
    focus: true
    padding: 0
    margins: 8
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    function open2(x, y) { if (x !== undefined) { root.x = x; root.y = y } open() }
    function popup(x, y) {
        if (x === undefined || y === undefined) { open2(x, y); return }
        const overlay = Overlay.overlay
        const maxX = Math.max(8, overlay.width - root.implicitWidth - 8)
        const maxY = Math.max(8, overlay.height - root.implicitHeight - 8)
        open2(Math.max(8, Math.min(x, maxX)), Math.max(8, Math.min(y, maxY)))
    }
    function popupAt(item, alignRight, above) {
        if (!item) { open2(undefined, undefined); return }
        const overlay = Overlay.overlay
        const anchor = item.mapToItem(overlay, alignRight ? item.width : 0, above ? 0 : item.height)
        const menuWidth = root.implicitWidth
        const menuHeight = root.implicitHeight
        const maxX = Math.max(8, overlay.width - menuWidth - 8)
        const maxY = Math.max(8, overlay.height - menuHeight - 8)
        const x = Math.max(8, Math.min(alignRight ? anchor.x - menuWidth : anchor.x, maxX))
        const y = Math.max(8, Math.min(above ? anchor.y - menuHeight - 8 : anchor.y + 8, maxY))
        open2(x, y)
    }
    function activateItem(item) {
        if (!item || item.separator || item.disabled)
            return
        if (item.keepOpen !== true) close()
        if (item.onTriggered)
            item.onTriggered()
        root.triggered(item)
    }
    function focusNext(index, direction) {
        for (let i = index + direction; i >= 0 && i < entries.count; i += direction) {
            const candidate = entries.itemAt(i)
            if (candidate && candidate.actionable) {
                candidate.forceActiveFocus()
                return
            }
        }
    }
    function focusFirst() {
        for (let i = 0; i < entries.count; i++) {
            const candidate = entries.itemAt(i)
            if (candidate && candidate.actionable) {
                candidate.forceActiveFocus()
                return
            }
        }
    }
    onOpened: Qt.callLater(root.focusFirst)
    implicitWidth: Math.max(minWidth, col.implicitWidth)
    background: Item {
        Rectangle {
            id: menuSurface
            anchors.fill: parent
            color: Theme.card
            radius: Theme.rem
            border.color: Theme.alpha(Theme.border, 0.35)
            layer.enabled: true
        }
        MultiEffect {
            anchors.fill: menuSurface
            source: menuSurface
            shadowEnabled: true
            shadowColor: "#4d000000"
            shadowBlur: 0.35
            shadowVerticalOffset: 4
            z: -1
        }
    }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.fastMs } }
    exit: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 80 } }
    contentItem: Column {
        id: col
        spacing: 0
        Item { width: col.width; height: root.outerVerticalPadding }
        Repeater {
            id: entries
            model: root.model
            delegate: Item {
                required property var modelData
                required property int index
                objectName: String(modelData.objectName || ("menuItem_" + String(modelData.action || "")))
                readonly property bool actionable: modelData.separator !== true && modelData.disabled !== true
                activeFocusOnTab: actionable
                enabled: modelData.disabled !== true
                Accessible.role: Accessible.Button
                Accessible.ignored: !actionable
                Accessible.name: String(modelData.label || "")
                Accessible.onPressAction: root.activateItem(modelData)
                width: col.width
                height: modelData.separator === true ? 17 : modelData.small === true ? root.smallRowHeight : root.rowHeight
                visible: modelData.hidden !== true
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                        root.focusNext(index, event.key === Qt.Key_Up ? -1 : 1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        root.activateItem(modelData)
                        event.accepted = true
                    }
                }
                Rectangle {
                    visible: modelData.separator === true
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 1; color: Theme.alpha(Theme.border, 0.5)
                }
                Rectangle {
                    visible: !modelData.separator
                    anchors.fill: parent
                    radius: Theme.rem * 0.5
                    color: modelData.highlighted === true
                        ? Theme.alpha(Theme.primary, 0.12)
                        : !modelData.disabled && (ma.containsMouse || parent.activeFocus)
                            ? Theme.alpha(Theme.muted, modelData.hoverOpacity === undefined ? root.hoverOpacity : modelData.hoverOpacity) : "transparent"
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: modelData.indent === true ? 48 : root.horizontalInset
                        anchors.rightMargin: root.horizontalInset
                        spacing: root.itemSpacing
                        Icon { visible: !!modelData.icon; name: modelData.icon || ""; size: modelData.iconSize || 17; anchors.verticalCenter: parent.verticalCenter
                               color: modelData.iconColor || (modelData.danger ? Theme.destructive : modelData.primary ? Theme.primary : Theme.mutedForeground)
                               opacity: modelData.disabled ? 0.5 : 1 }
                        AppText {
                            width: Math.max(0, parent.width - (modelData.icon ? (modelData.iconSize || 17) + root.itemSpacing : 0)
                                - (modelData.chevron ? 13 + root.itemSpacing : 0))
                            text: modelData.label || ""
                            size: modelData.small === true ? 0.75 : 0.875
                            semibold: modelData.semibold === true
                            medium: modelData.medium === true
                            anchors.verticalCenter: parent.verticalCenter
                            opacity: modelData.disabled ? 0.5 : 1
                            color: modelData.textColor || (modelData.danger ? Theme.destructive : Theme.foreground)
                            elide: Text.ElideRight
                        }
                        Icon { visible: modelData.chevron === true; name: "chevron-right"; size: 13; anchors.verticalCenter: parent.verticalCenter; color: Theme.mutedForeground }
                    }
                    Icon { visible: modelData.checked === true; name: "check"; size: 16; anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter }
                    MouseArea {
                        id: ma
                        anchors.fill: parent; hoverEnabled: true
                        cursorShape: modelData.disabled ? Qt.ArrowCursor : Qt.PointingHandCursor
                        onClicked: root.activateItem(modelData)
                    }
                }
            }
        }
        Item { width: col.width; height: root.outerVerticalPadding }
    }
}
