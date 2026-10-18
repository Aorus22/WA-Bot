import QtQuick
import QtQuick.Controls
import org.kde.layershell 1.0 as LayerShell
import "shell"

ApplicationWindow {
    id: win
    objectName: "popoverCardWindow"
    width: 880
    height: 620
    minimumWidth: 800
    minimumHeight: 540
    visible: Platform.popoverCardVisible
    property real entryProgress: 1
    title: "WA Bot Desktop"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"

    LayerShell.Window.scope: "wabot-popover"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityOnDemand
    LayerShell.Window.anchors: {
        if (Platform.popoverEdge === "bottom")
            return LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorLeft
        if (Platform.popoverEdge === "left")
            return LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
        if (Platform.popoverEdge === "right")
            return LayerShell.Window.AnchorTop | LayerShell.Window.AnchorRight
        return LayerShell.Window.AnchorTop | LayerShell.Window.AnchorLeft
    }
    LayerShell.Window.margins.top: Platform.popoverTopMargin
    LayerShell.Window.margins.right: Platform.popoverRightMargin
    LayerShell.Window.margins.bottom: Platform.popoverBottomMargin
    LayerShell.Window.margins.left: Platform.popoverLeftMargin

    onClosing: close => {
        close.accepted = false
        Platform.dismissPopover()
    }
    onVisibleChanged: {
        if (!visible) return
        entryProgress = 0
        popoverEnter.start()
    }
    onActiveChanged: AppState.setWindowVisible(active && visible)
    onVisibilityChanged: AppState.setWindowVisible(active && visibility !== Window.Hidden && visibility !== Window.Minimized)

    ParallelAnimation {
        id: popoverEnter
        NumberAnimation {
            target: win
            property: "entryProgress"
            from: 0
            to: 1
            duration: Theme.baseMs
            easing.type: Easing.OutQuint
        }
    }

    Rectangle {
        id: windowSurface
        anchors.fill: parent
        anchors.margins: 5
        radius: 14
        opacity: win.entryProgress
        color: Theme.glass(Theme.background)
        border.width: 1
        border.color: Theme.border
        clip: true
        transform: Translate {
            x: Platform.popoverEdge === "left" ? -14 * (1 - win.entryProgress)
                : Platform.popoverEdge === "right" ? 14 * (1 - win.entryProgress) : 0
            y: Platform.popoverEdge === "top" ? -14 * (1 - win.entryProgress)
                : Platform.popoverEdge === "bottom" ? 14 * (1 - win.entryProgress) : 0
        }

        Shell { anchors.fill: parent; win: win }
        Loader { anchors.fill: parent; source: Qt.resolvedUrl("pages/auth/StartupGate.qml") }
    }

    Connections {
        target: Platform
        function onShowWindowRequested() { Platform.showPopover() }
        function onToggleWindowRequested() { Platform.togglePopover() }
    }

    Window {
        id: backdrop
        objectName: "popoverBackdropWindow"
        transientParent: null
        width: Platform.popoverScreenWidth
        height: Platform.popoverScreenHeight
        visible: Platform.popoverBackdropVisible
        flags: Qt.Window | Qt.FramelessWindowHint
        color: "transparent"

        LayerShell.Window.scope: "wabot-popover-backdrop"
        LayerShell.Window.layer: LayerShell.Window.LayerTop
        LayerShell.Window.exclusionZone: -1
        LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
        LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
            | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onEntered: Platform.capturePopoverPointer(mouseX, mouseY)
            onPositionChanged: mouse => Platform.capturePopoverPointer(mouse.x, mouse.y)
            onPressed: Platform.dismissPopover()
        }
    }
}
