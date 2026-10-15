import QtQuick
import QtQuick.Layouts
import "../ui"

// Titlebar over [nav rail | banner + route outlet]; call overlay and toasts float above.
Item {
    id: root
    required property var win
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Titlebar { win: root.win; Layout.fillWidth: true }
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true
            spacing: 0
            NavSidebar { Layout.fillHeight: true }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true
                spacing: 0
                ConnectionBanner { Layout.fillWidth: true; Layout.preferredHeight: height }
                PageHost { Layout.fillWidth: true; Layout.fillHeight: true }
            }
        }
    }
    // An active call paints above every route (web `fixed inset-0 z-[90]`).
    Loader { anchors.fill: parent; z: 90; source: Qt.resolvedUrl("../pages/calls/CallOverlay.qml") }
    ToastHost {}
}
