import QtQuick
import QtQuick.Controls
import QtQuick.Effects

// Modal card centred over the window with a dimmed backdrop (web Dialog look).
//   AppDialog { id: d; title: "Delete?"; description: "..."; width: 420
//     Column { ... default content ... }
//     footer: [ Btn{...}, Btn{...} ] }
Popup {
    id: root
    property string title
    property string description
    property real cardWidth: 448
    property real cardPadding: Theme.rem * 1.25
    property real bodySpacing: Theme.rem * 0.75
    property color cardColor: Theme.card
    property real cardRadius: Theme.rem
    property color overlayColor: Qt.rgba(0, 0, 0, 0.5333)
    default property alias content: body.data
    property alias footer: foot.data
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    dim: true
    padding: 0
    closePolicy: Popup.CloseOnEscape
    width: Math.min(cardWidth, (parent ? parent.width : cardWidth) - 32)
    Overlay.modal: Rectangle { color: root.overlayColor }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.baseMs; easing.type: Easing.OutQuint } }
    exit: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 100 } }
    background: Item {
        Rectangle {
            id: surface
            anchors.fill: parent
            color: root.cardColor
            radius: root.cardRadius
            border.color: Theme.border
            layer.enabled: true
        }
        MultiEffect {
            anchors.fill: surface
            source: surface
            shadowEnabled: true
            shadowColor: "#66000000"
            shadowBlur: 0.5
            shadowVerticalOffset: 6
            z: -1
        }
    }
    contentItem: Column {
        spacing: root.bodySpacing
        topPadding: root.cardPadding; bottomPadding: root.cardPadding
        leftPadding: root.cardPadding; rightPadding: root.cardPadding
        AppText { visible: root.title !== ""; text: root.title; size: 1.125; semibold: true; width: parent.width - root.cardPadding * 2; wrapMode: Text.Wrap }
        AppText { visible: root.description !== ""; text: root.description; color: Theme.mutedForeground; width: parent.width - root.cardPadding * 2; wrapMode: Text.Wrap }
        Column { id: body; width: parent.width - root.cardPadding * 2; spacing: root.bodySpacing }
        Row { id: foot; spacing: Theme.rem * 0.5; layoutDirection: Qt.LeftToRight; width: parent.width - root.cardPadding * 2 }
    }
}
