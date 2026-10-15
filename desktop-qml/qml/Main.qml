import QtQuick
import QtQuick.Controls
import "shell"

ApplicationWindow {
    id: win
    width: 1100
    height: 720
    minimumWidth: 800
    minimumHeight: 540
    visible: true
    title: "WA Bot Desktop"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.transparency > 0 ? "transparent" : Theme.background

    function toggleMaximize() { visibility === Window.Maximized ? showNormal() : showMaximized() }

    onActiveChanged: AppState.setWindowVisible(active && visible)
    onVisibilityChanged: AppState.setWindowVisible(active && visibility !== Window.Hidden && visibility !== Window.Minimized)

    Rectangle { anchors.fill: parent; color: "transparent" }
    Shell { anchors.fill: parent; win: win }
    ResizeHandles { win: win }
}
