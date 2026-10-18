import QtQuick
import QtQuick.Controls
import "shell"

ApplicationWindow {
    id: win
    width: 1100
    height: 720
    minimumWidth: 800
    minimumHeight: 540
    visible: Platform.startVisible
    title: "WA Bot Desktop"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    readonly property real frameMargin: Platform.popoverMode || visibility === Window.Maximized || visibility === Window.FullScreen ? 0 : 16

    onClosing: close => {
        if (Platform.popoverRequested) {
            close.accepted = false
            win.hide()
            return
        }
        if (Platform.trayAvailable && !Platform.quitting) {
            close.accepted = false
            Platform.hideToTray()
        }
    }

    function toggleMaximize() { visibility === Window.Maximized ? showNormal() : showMaximized() }

    onActiveChanged: AppState.setWindowVisible(active && visible)
    onVisibilityChanged: AppState.setWindowVisible(active && visibility !== Window.Hidden && visibility !== Window.Minimized)

    Rectangle {
        id: windowSurface
        anchors.fill: parent
        anchors.margins: win.frameMargin
        radius: win.frameMargin > 0 ? 10 : 0
        // Match the transparent GPUI root. The title bar, navigation rail, and
        // route panels apply their own glass fill, so an opaque glass root
        // would composite window transparency a second time.
        color: "transparent"
        border.width: win.frameMargin > 0 ? 1 : 0
        border.color: Theme.border
        clip: true

        Shell { anchors.fill: parent; win: win }
        // Start-up gate: "Connecting to WA Bot..." / backend-unavailable screen over everything below the titlebar.
        Loader { anchors.fill: parent; anchors.topMargin: 48; source: Qt.resolvedUrl("pages/auth/StartupGate.qml") }
    }
    ResizeHandles { win: win }

    Connections {
        target: Platform
        function onShowWindowRequested() {
            win.show()
            win.raise()
            win.requestActivate()
        }
        function onToggleWindowRequested() {
            if (win.visible) win.hide()
            else {
                win.show()
                win.raise()
                win.requestActivate()
            }
        }
    }
}
