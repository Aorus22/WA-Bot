import QtQuick
import QtQuick.Controls

// Modal card centred over the window with a dimmed backdrop (web Dialog look).
//   AppDialog { id: d; title: "Delete?"; description: "..."; width: 420
//     Column { ... default content ... }
//     footer: [ Btn{...}, Btn{...} ] }
Popup {
    id: root
    property string title
    property string description
    property real cardWidth: 448
    default property alias content: body.data
    property alias footer: foot.data
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    dim: true
    padding: 0
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    width: Math.min(cardWidth, (parent ? parent.width : cardWidth) - 32)
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.6) }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.baseMs; easing.type: Easing.OutQuint } }
    exit: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 100 } }
    background: Rectangle { color: Theme.background; radius: Theme.rem * 0.5; border.color: Theme.border }
    contentItem: Column {
        spacing: Theme.rem
        topPadding: Theme.rem * 1.5; bottomPadding: Theme.rem * 1.5
        leftPadding: Theme.rem * 1.5; rightPadding: Theme.rem * 1.5
        AppText { visible: root.title !== ""; text: root.title; size: 1.125; semibold: true; width: parent.width - Theme.rem * 3; wrapMode: Text.Wrap }
        AppText { visible: root.description !== ""; text: root.description; color: Theme.mutedForeground; width: parent.width - Theme.rem * 3; wrapMode: Text.Wrap }
        Column { id: body; width: parent.width - Theme.rem * 3; spacing: Theme.rem * 0.75 }
        Row { id: foot; spacing: Theme.rem * 0.5; layoutDirection: Qt.RightToLeft; width: parent.width - Theme.rem * 3 }
    }
}
