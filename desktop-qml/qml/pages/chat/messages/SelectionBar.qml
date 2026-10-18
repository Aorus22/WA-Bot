import QtQuick
import QtQuick.Layouts
import "../../../ui"

Rectangle {
    color: Theme.alpha(Theme.muted, 0.2)
    border.width: 0

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.alpha(Theme.border, 0.4)
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        spacing: 8
        IconButton { icon: "x"; iconSize: 18; circular: true; iconColor: Theme.mutedForeground; hoverColor: Theme.alpha(Theme.muted, 0.65); tooltip: "Cancel"; onClicked: Messages.exitSelect() }
        AppText {
            Layout.fillWidth: true
            text: Messages.selectConfirmDelete
                ? "Delete " + String(Messages.selectedCount) + " for everyone? Click the bin again"
                : String(Messages.selectedCount) + " selected"
            size: 1.0
            medium: true
        }
        IconButton {
            icon: "star"
            iconSize: 18
            circular: true
            iconColor: Theme.mutedForeground
            hoverColor: Theme.alpha(Theme.muted, 0.65)
            disabledOpacity: 0.35
            tooltip: "Star"
            enabled: Messages.selectedCount > 0
            onClicked: Messages.selectStar()
        }
        IconButton {
            objectName: "selectionCopyButton"
            icon: "copy"
            iconSize: 18
            circular: true
            iconColor: Theme.mutedForeground
            hoverColor: Theme.alpha(Theme.muted, 0.65)
            disabledOpacity: 0.35
            tooltip: "Copy"
            enabled: Messages.selectedCount > 0
            onClicked: Messages.selectCopy()
        }
        IconButton {
            objectName: "selectionForwardButton"
            icon: "forward"
            iconSize: 18
            circular: true
            iconColor: Theme.mutedForeground
            hoverColor: Theme.alpha(Theme.muted, 0.65)
            disabledOpacity: 0.35
            tooltip: "Forward"
            enabled: Messages.selectedCount > 0
            onClicked: Messages.selectForward()
        }
        IconButton {
            objectName: "selectionDeleteButton"
            icon: "trash"
            iconSize: 18
            circular: true
            iconColor: Messages.selectConfirmDelete ? Theme.destructive : Theme.mutedForeground
            hoverColor: Theme.alpha(Theme.muted, 0.65)
            disabledOpacity: 0.35
            tooltip: "Delete for everyone (own messages only)"
            enabled: Messages.selectionDeletable
            onClicked: Messages.selectDelete()
        }
    }
}
