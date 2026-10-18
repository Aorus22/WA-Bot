import QtQuick
import QtQuick.Layouts
import "../../ui"

Rectangle {
    id: root
    property string heading: ""
    property string description: ""

    color: Theme.glass(Theme.background)

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(560, Math.max(0, root.width - 32))
        spacing: 12

        AppText {
            Layout.fillWidth: true
            text: root.heading
            horizontalAlignment: Text.AlignHCenter
            size: 1.125
            semibold: true
        }

        AppText {
            Layout.fillWidth: true
            text: root.description
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            size: 0.875
            color: Theme.mutedForeground
        }
    }
}
