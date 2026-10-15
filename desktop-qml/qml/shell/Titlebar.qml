import QtQuick
import "../ui"

// GPUI titlebar: 48px, "WA Bot" label (14px semibold @70%), drag area, 44/44/46px window controls.
Rectangle {
    id: root
    required property var win
    implicitHeight: 48
    color: Theme.glass(Theme.background)
    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.alpha(Theme.border, 0.4) }

    AppText { x: Theme.rem * 0.75; anchors.verticalCenter: parent.verticalCenter; text: "WA Bot"; semibold: true; opacity: 0.7 }

    MouseArea {
        anchors { left: parent.left; right: controls.left; top: parent.top; bottom: parent.bottom }
        onPressed: root.win.startSystemMove()
        onDoubleClicked: root.win.toggleMaximize()
    }

    Row {
        id: controls
        anchors.right: parent.right
        height: parent.height
        Repeater {
            model: [
                { icon: "minus", w: 44, sz: 12, act: "min" },
                { icon: "square", w: 44, sz: 12, act: "max" },
                { icon: "x", w: 46, sz: 13, act: "close" }
            ]
            delegate: Rectangle {
                required property var modelData
                width: modelData.w; height: parent.height
                color: ma.containsMouse ? (modelData.act === "close" ? "#e81123" : Theme.muted) : "transparent"
                Icon {
                    anchors.centerIn: parent
                    size: modelData.sz
                    name: modelData.act === "max" && root.win.visibility === Window.Maximized ? "copy" : modelData.icon
                    color: modelData.act === "close" && ma.containsMouse ? "#ffffff" : Theme.foreground
                }
                MouseArea {
                    id: ma; anchors.fill: parent; hoverEnabled: true
                    onClicked: {
                        if (modelData.act === "min") root.win.showMinimized()
                        else if (modelData.act === "max") root.win.toggleMaximize()
                        else root.win.close()
                    }
                }
            }
        }
    }
}
