import QtQuick

// shadcn-style button. variant: primary | secondary | outline | ghost | destructive | link
Rectangle {
    id: root
    property string text
    property string icon
    property string variant: "primary"
    property bool loading: false
    property real size: 0.875
    property alias hovered: mouse.containsMouse
    signal clicked()
    readonly property color fg: variant === "primary" ? Theme.primaryForeground
        : variant === "destructive" ? Theme.destructiveForeground
        : variant === "secondary" ? Theme.secondaryForeground
        : variant === "link" ? Theme.primary : Theme.foreground
    readonly property color base: variant === "primary" ? Theme.primary
        : variant === "destructive" ? Theme.destructive
        : variant === "secondary" ? Theme.secondary
        : variant === "outline" ? "transparent" : "transparent"
    implicitHeight: Math.round(Theme.rem * 2.25)
    implicitWidth: row.implicitWidth + Theme.rem * 1.5
    radius: Theme.rem * 0.375
    color: !enabled ? base
        : mouse.containsMouse ? (variant === "primary" || variant === "destructive" ? Theme.alpha(base, 0.9)
            : variant === "link" ? "transparent" : Theme.accent)
        : base
    border.width: variant === "outline" ? 1 : 0
    border.color: Theme.input
    opacity: enabled ? 1 : 0.5
    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.rem * 0.5
        Spinner { visible: root.loading; color: root.fg; size: 16; anchors.verticalCenter: parent.verticalCenter }
        Icon { visible: root.icon !== "" && !root.loading; name: root.icon; color: root.fg; size: 16; anchors.verticalCenter: parent.verticalCenter }
        AppText {
            visible: root.text !== ""
            text: root.text; color: root.fg; size: root.size; medium: true
            font.underline: root.variant === "link" && mouse.containsMouse
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.enabled && !root.loading
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
