import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import "../../ui"

Item {
    id: root
    objectName: "channelPost_" + String(post.id || "")
    property var post: ({})
    property string channelName: ""
    property var quickReactions: []
    signal reacted(string emoji)

    implicitHeight: card.implicitHeight + 16

    MultiEffect {
        anchors.fill: card
        source: card
        shadowEnabled: true
        shadowColor: "#33000000"
        shadowBlur: 0.22
        shadowVerticalOffset: 1
        z: -1
    }

    Rectangle {
        id: card
        x: (root.width - width) / 2
        width: Math.max(0, Math.min(root.width - 48, 600))
        implicitHeight: content.implicitHeight + 34
        height: implicitHeight
        radius: Theme.rem
        color: Theme.card
        border.width: 1
        border.color: Theme.border

        ColumnLayout {
            id: content
            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                AppText {
                    Layout.fillWidth: true
                    text: root.channelName
                    size: 0.75
                    color: Theme.primary
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                }
                AppText {
                    text: String(root.post.timeLabel || "")
                    size: 0.75
                    color: Theme.mutedForeground
                }
            }

            AppText {
                Layout.fillWidth: true
                text: String(root.post.content || "")
                size: 0.875
                color: Theme.foreground
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
                lineHeight: 1.4
                lineHeightMode: Text.ProportionalHeight
            }

            Item {
                Layout.fillWidth: true
                implicitHeight: footerRow.implicitHeight + 5

                Rectangle {
                    anchors.top: parent.top
                    width: parent.width
                    height: 1
                    color: Theme.alpha(Theme.border, 0.5)
                }

                RowLayout {
                    id: footerRow
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.topMargin: 5
                    spacing: 0

                    RowLayout {
                        spacing: 4
                        Icon { name: "eye"; size: 14; color: Theme.mutedForeground }
                        AppText { text: String(Number(root.post.viewsCount || 0)); size: 0.75; color: Theme.mutedForeground }
                    }

                    Item { Layout.fillWidth: true }

                    Row {
                        spacing: 6
                        Repeater {
                            model: root.post.reactions || []
                            delegate: Rectangle {
                                required property var modelData
                                height: 20
                                width: reactionLabel.implicitWidth + 16
                                radius: height / 2
                                color: Theme.alpha(Theme.muted, 0.4)
                                Row {
                                    id: reactionLabel
                                    anchors.centerIn: parent
                                    spacing: 4
                                    AppText { text: String(modelData.emoji || ""); size: 0.75 }
                                    AppText { text: String(modelData.count || 0); size: 0.75; color: Theme.mutedForeground }
                                }
                            }
                        }
                    }

                    Row {
                        Layout.leftMargin: 8
                        spacing: 4
                        Repeater {
                            model: root.quickReactions
                            delegate: Rectangle {
                                required property var modelData
                                property bool hovered: false
                                width: quickReactionLabel.implicitWidth + 12
                                height: 20
                                radius: 4
                                color: hovered ? Theme.alpha(Theme.muted, 0.5) : "transparent"
                                AppText {
                                    id: quickReactionLabel
                                    anchors.centerIn: parent
                                    text: String(modelData)
                                    size: 0.75
                                }
                                MouseArea {
                                    objectName: "channelReaction_" + String(root.post.id || "") + "_" + String(modelData)
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onEntered: parent.hovered = true
                                    onExited: parent.hovered = false
                                    onClicked: root.reacted(String(modelData))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
