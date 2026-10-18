import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import "../../../ui"

Item {
    id: root
    objectName: "mediaViewer"
    visible: Messages.viewerOpen
    focus: visible
    z: 80

    readonly property var media: Messages.viewerItem
    readonly property string mediaId: String(media.id || "")
    readonly property string mediaChatId: String(media.chatId || "")
    readonly property string mediaSource: String(media.src || "")
    readonly property bool forwardable: Boolean(media.forwardable)
    property real previewZoom: 1
    property real panX: 0
    property real panY: 0
    property bool dragging: false
    property real dragStartX: 0
    property real dragStartY: 0
    property real dragPanX: 0
    property real dragPanY: 0

    readonly property real areaWidth: Math.max(100, imageArea.width)
    readonly property real areaHeight: Math.max(100, imageArea.height)
    readonly property real naturalWidth: viewerImage.implicitWidth > 0 ? viewerImage.implicitWidth : 0
    readonly property real naturalHeight: viewerImage.implicitHeight > 0 ? viewerImage.implicitHeight : 0
    readonly property real fitScale: naturalWidth > 0 && naturalHeight > 0
        ? Math.min(areaWidth / naturalWidth, areaHeight / naturalHeight, 1) : 1
    readonly property real baseImageWidth: naturalWidth > 0 ? naturalWidth * fitScale : 0
    readonly property real baseImageHeight: naturalHeight > 0 ? naturalHeight * fitScale : 0
    readonly property real imageWidth: baseImageWidth * previewZoom
    readonly property real imageHeight: baseImageHeight * previewZoom
    readonly property bool imageReady: viewerImage.status === Image.Ready

    function resetPreview() {
        previewZoom = 1
        panX = 0
        panY = 0
        dragging = false
    }

    function clampPan(value, extent, zoom) {
        const limit = extent * zoom / 2
        return Math.max(-limit, Math.min(limit, value))
    }

    function zoomAt(factor, anchorX, anchorY) {
        const oldZoom = previewZoom
        const newZoom = Math.max(1, Math.min(8, oldZoom * factor))
        if (Math.abs(newZoom - oldZoom) < 0.0001)
            return
        if (newZoom <= 1.0001) {
            resetPreview()
            return
        }
        const centerX = imageArea.width / 2
        const centerY = imageArea.height / 2
        const ax = anchorX === undefined ? centerX : anchorX
        const ay = anchorY === undefined ? centerY : anchorY
        const qx = (ax - (centerX + panX)) / oldZoom
        const qy = (ay - (centerY + panY)) / oldZoom
        previewZoom = newZoom
        panX = clampPan(ax - centerX - qx * newZoom, areaWidth, newZoom)
        panY = clampPan(ay - centerY - qy * newZoom, areaHeight, newZoom)
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Escape) {
            Messages.closeViewer()
            event.accepted = true
        } else if (event.key === Qt.Key_Left) {
            Messages.moveViewer(-1)
            event.accepted = true
        } else if (event.key === Qt.Key_Right) {
            Messages.moveViewer(1)
            event.accepted = true
        } else if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal) {
            zoomAt(1.25)
            event.accepted = true
        } else if (event.key === Qt.Key_Minus) {
            zoomAt(1 / 1.25)
            event.accepted = true
        } else if (event.key === Qt.Key_0) {
            resetPreview()
            event.accepted = true
        }
    }

    Rectangle { anchors.fill: parent; color: "#ee0b141a" }
    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: Messages.closeViewer()
    }

    Rectangle {
        id: toolbar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 56
        color: "#cc0b141a"
        z: 5

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 16
            anchors.rightMargin: 16
            spacing: 4

            AppText {
                visible: Messages.viewerCount > 1
                Layout.preferredWidth: visible ? 52 : 0
                text: String(Messages.viewerIndex + 1) + " / " + String(Messages.viewerCount)
                size: 0.875
                color: "#e9edef"
            }
            Item { Layout.fillWidth: true }

            IconButton {
                objectName: "mediaViewerZoomOut"
                icon: "minus"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Zoom out"
                onClicked: root.zoomAt(1 / 1.25)
            }
            AppText {
                Layout.preferredWidth: 48
                horizontalAlignment: Text.AlignHCenter
                text: String(Math.round(root.previewZoom * 100)) + "%"
                size: 0.75
                color: "#e9edef"
            }
            IconButton {
                objectName: "mediaViewerZoomIn"
                icon: "plus"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Zoom in"
                onClicked: root.zoomAt(1.25)
            }
            IconButton {
                objectName: "mediaViewerFit"
                icon: "maximize"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Fit to window"
                onClicked: root.resetPreview()
            }
            IconButton {
                objectName: "mediaViewerSaveButton"
                icon: "download"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Save image"
                onClicked: saveDialog.open()
            }
            IconButton {
                objectName: "mediaViewerForward"
                visible: root.forwardable && root.mediaChatId !== ""
                icon: "forward"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Forward image"
                onClicked: {
                    const id = root.mediaId
                    Messages.closeViewer()
                    Messages.forward(id)
                }
            }
            IconButton {
                objectName: "mediaViewerCloseButton"
                icon: "x"
                iconSize: 18
                pad: 9
                radius: 18
                iconColor: "#e9edef"
                hoverColor: "#1fffffff"
                tooltip: "Close viewer"
                onClicked: Messages.closeViewer()
            }
        }
    }

    Item {
        id: imageArea
        objectName: "mediaViewerImageArea"
        anchors.top: toolbar.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        anchors.bottomMargin: 24
        clip: true

        Image {
            id: viewerImage
            objectName: "mediaViewerImage"
            x: (imageArea.width - width) / 2 + root.panX
            y: (imageArea.height - height) / 2 + root.panY
            width: root.imageWidth
            height: root.imageHeight
            visible: root.mediaSource !== ""
            source: root.mediaSource
            asynchronous: true
            cache: true
            fillMode: Image.PreserveAspectFit
        }

        AppText {
            anchors.centerIn: parent
            visible: root.mediaSource === "" || viewerImage.status === Image.Error
            text: "Couldn't load this image"
            size: 0.875
            color: "#e9edef"
        }
        Spinner {
            anchors.centerIn: parent
            visible: root.mediaSource !== "" && viewerImage.status === Image.Loading
            size: 32
        }

        MouseArea {
            id: imageHit
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            hoverEnabled: true
            cursorShape: root.previewZoom > 1 ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.ArrowCursor
            onPressed: mouse => {
                if (mouse.button === Qt.RightButton) {
                    if (root.imageReady && root.pointInsideImage(mouse.x, mouse.y)) {
                        const pos = imageArea.mapToItem(Overlay.overlay, mouse.x, mouse.y)
                        imageMenu.popup(pos.x + 8, pos.y + 8)
                    }
                    mouse.accepted = true
                    return
                }
                if (root.previewZoom > 1) {
                    root.dragging = true
                    root.dragStartX = mouse.x
                    root.dragStartY = mouse.y
                    root.dragPanX = root.panX
                    root.dragPanY = root.panY
                }
            }
            onPositionChanged: mouse => {
                if (!root.dragging)
                    return
                root.panX = root.clampPan(root.dragPanX + mouse.x - root.dragStartX, root.areaWidth, root.previewZoom)
                root.panY = root.clampPan(root.dragPanY + mouse.y - root.dragStartY, root.areaHeight, root.previewZoom)
            }
            onReleased: root.dragging = false
            onCanceled: root.dragging = false
            onClicked: mouse => {
                if (mouse.button === Qt.LeftButton && !root.pointInsideImage(mouse.x, mouse.y))
                    Messages.closeViewer()
            }
            onDoubleClicked: mouse => root.zoomAt(root.previewZoom > 1 ? 1 / root.previewZoom : 2.5, mouse.x, mouse.y)
            onWheel: wheel => {
                const amount = Math.pow(1.25, wheel.angleDelta.y / 120)
                root.zoomAt(amount, wheel.x, wheel.y)
                wheel.accepted = true
            }
        }

        IconButton {
            objectName: "pv-prev"
            visible: Messages.viewerIndex > 0
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            icon: "chevron-left"
            iconSize: 20
            pad: 10
            radius: 20
            iconColor: "#e9edef"
            hoverColor: "#1fffffff"
            onClicked: Messages.moveViewer(-1)
        }
        IconButton {
            objectName: "pv-next"
            visible: Messages.viewerIndex + 1 < Messages.viewerCount
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            icon: "chevron-right"
            iconSize: 20
            pad: 10
            radius: 20
            iconColor: "#e9edef"
            hoverColor: "#1fffffff"
            onClicked: Messages.moveViewer(1)
        }
    }

    AppMenu {
        id: imageMenu
        minWidth: 208
        model: [
            { label: "Copy image", icon: "copy", action: "copy" },
            { label: "Save image as…", icon: "download", action: "save" }
        ]
        onTriggered: item => {
            if (item.action === "copy") Messages.copyImage(root.mediaSource)
            else if (item.action === "save") saveDialog.open()
        }
    }

    FileDialog {
        id: saveDialog
        title: "Save image as…"
        fileMode: FileDialog.SaveFile
        selectedFile: "file:///" + String(root.media.name || "image.jpg").replace(/[^a-zA-Z0-9._ -]/g, "_")
        nameFilters: ["All files (*)"]
        onAccepted: Messages.saveMedia(root.mediaId, selectedFile.toString())
    }

    Connections {
        target: Messages
        function onViewerChanged() { root.resetPreview() }
    }

    function pointInsideImage(x, y) {
        if (!root.imageReady || root.imageWidth <= 0 || root.imageHeight <= 0)
            return false
        const left = (imageArea.width - root.imageWidth) / 2 + root.panX
        const top = (imageArea.height - root.imageHeight) / 2 + root.panY
        return x >= left && x <= left + root.imageWidth && y >= top && y <= top + root.imageHeight
    }
}
