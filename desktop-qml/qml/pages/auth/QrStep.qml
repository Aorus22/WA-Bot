import QtQuick
import QtQuick.Layouts
import "../../ui"

// One numbered instruction: 24px round badge (muted fill, border) + text_sm label, gap_3.
RowLayout {
    property int num: 1
    property string text
    spacing: Theme.rem * 0.75
    Rectangle {
        Layout.preferredWidth: 24; Layout.preferredHeight: 24
        Layout.alignment: Qt.AlignVCenter
        radius: 12
        color: Theme.muted
        border.width: 1; border.color: Theme.border
        AppText { anchors.centerIn: parent; text: String(num); size: 0.75; medium: true }
    }
    AuthText {
        Layout.fillWidth: true
        text: parent.text
        color: Theme.mutedForeground
        wrapMode: Text.Wrap
    }
}
