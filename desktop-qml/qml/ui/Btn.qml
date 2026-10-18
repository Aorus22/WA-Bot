import QtQuick

// shadcn-style button. variant: primary | softPrimary | secondary | outline | ghost | destructive | link
Rectangle {
    id: root
    property string text
    property string icon
    property string variant: "primary"
    property bool loading: false
    property string accessibleName: text !== "" ? text : icon
    property real size: 0.875
    property real horizontalPadding: Theme.rem * 0.75
    property bool boldText: false
    property alias hovered: mouse.containsMouse
    signal clicked()
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Keys.onPressed: event => {
        if (!root.enabled || root.loading)
            return
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.clicked()
            event.accepted = true
        }
    }
    readonly property color fg: variant === "primary" ? Theme.primaryForeground
        : variant === "destructive" ? Theme.destructiveForeground
        : variant === "secondary" ? Theme.secondaryForeground
        : (variant === "link" || variant === "softPrimary") ? Theme.primary : Theme.foreground
    readonly property color base: variant === "primary" ? Theme.primary
        : variant === "destructive" ? Theme.destructive
        : variant === "secondary" ? Theme.secondary
        : variant === "softPrimary" ? Theme.alpha(Theme.primary, 0.12)
        : variant === "outline" ? "transparent" : "transparent"
    implicitHeight: Math.round(Theme.rem * 2.25)
    implicitWidth: row.implicitWidth + horizontalPadding * 2
    radius: Theme.rem * 0.375
    color: !enabled ? base
        : mouse.containsMouse || activeFocus ? (variant === "primary" || variant === "destructive" ? Theme.alpha(base, 0.9)
            : variant === "link" ? "transparent" : Theme.accent)
        : base
    border.width: activeFocus ? 2 : variant === "outline" ? 1 : 0
    border.color: activeFocus ? Theme.ring : Theme.input
    opacity: enabled ? 1 : 0.5
    Row {
        id: row
        anchors.centerIn: parent
        spacing: root.variant === "softPrimary" ? 6 : Theme.rem * 0.5
        Spinner { visible: root.loading; color: root.fg; size: 16; anchors.verticalCenter: parent.verticalCenter }
        Icon { visible: root.icon !== "" && !root.loading; name: root.icon; color: root.fg; size: root.variant === "softPrimary" ? 14 : 16; anchors.verticalCenter: parent.verticalCenter }
        AppText {
            visible: root.text !== ""
            text: root.text; color: root.fg; size: root.size; medium: true
            font.weight: root.boldText || root.variant === "softPrimary" ? Font.Bold : Font.Medium
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
