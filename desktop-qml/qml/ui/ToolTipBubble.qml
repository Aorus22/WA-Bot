import QtQuick
import QtQuick.Controls

ToolTip {
    delay: 500
    padding: 6
    contentItem: AppText { text: ToolTip.toolTip.text; size: 0.75; color: Theme.foreground }
    background: Rectangle { color: Theme.card; border.color: Theme.border; radius: 6 }
}
