import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import "../messages/EmojiData.js" as EmojiData
import "../../../ui"

Popup {
    id: root
    objectName: "emojiPicker"
    signal chosen(string emoji)
    signal switchToStickers()

    parent: Overlay.overlay
    width: 320
    property int categoryIndex: 0
    readonly property var category: EmojiData.categories[Math.min(categoryIndex, EmojiData.categories.length - 1)]
    readonly property int gridColumns: Math.max(1, Math.floor((width - 16) / 30))
    readonly property int gridRows: Math.ceil((category ? category.emojis.length : 0) / gridColumns)
    readonly property int gridHeight: Math.min(190, gridRows * 30)
    height: Math.min(parent ? parent.height - 16 : 320, 112 + gridHeight)
    padding: 8
    modal: false
    focus: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Item {
        Rectangle {
            id: surface
            anchors.fill: parent
            color: Theme.card
            radius: Theme.rem
            border.color: Theme.border
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

    contentItem: ColumnLayout {
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 20
            spacing: 6
            Icon { name: "smile"; size: 14; color: "#eab308" }
            AppText { Layout.fillWidth: true; text: "Emoji"; size: 0.75; semibold: true }
            IconButton {
                icon: "x"
                iconSize: 13
                pad: 4
                circular: true
                iconColor: Theme.mutedForeground
                hoverColor: Theme.alpha(Theme.muted, 0.5)
                tooltip: "Close"
                onClicked: root.close()
            }
        }

        Row {
            Layout.fillWidth: true
            Layout.preferredHeight: 24
            spacing: 4
            Repeater {
                model: EmojiData.categories
                delegate: Rectangle {
                    required property int index
                    required property var modelData
                    width: categoryLabel.implicitWidth + 16
                    height: 24
                    radius: Theme.rem * 0.375
                    color: root.categoryIndex === index ? Theme.alpha(Theme.primary, 0.15) : "transparent"
                    AppText {
                        id: categoryLabel
                        anchors.centerIn: parent
                        text: String(modelData.name || "")
                        size: 0.75
                        color: root.categoryIndex === index ? Theme.primary : Theme.mutedForeground
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.categoryIndex = index
                    }
                }
            }
        }

        GridView {
            id: emojiGrid
            objectName: "emojiPickerGrid"
            Layout.fillWidth: true
            Layout.preferredHeight: root.gridHeight
            Layout.maximumHeight: 190
            clip: true
            cellWidth: 30
            cellHeight: 30
            model: root.category ? root.category.emojis : []
            delegate: Item {
                required property string modelData
                width: emojiGrid.cellWidth
                height: emojiGrid.cellHeight
                property bool hovered: false
                Rectangle {
                    anchors.fill: parent
                    radius: Theme.rem * 0.375
                    color: parent.hovered ? Theme.alpha(Theme.muted, 0.5) : "transparent"
                }
                AppText {
                    anchors.centerIn: parent
                    text: modelData
                    size: 1.0
                }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onEntered: parent.hovered = true
                    onExited: parent.hovered = false
                    onClicked: { root.chosen(modelData); root.close() }
                }
            }
        }

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 112
            Layout.preferredHeight: 40
            Rectangle {
                width: 112
                height: 36
                anchors.bottom: parent.bottom
                radius: height / 2
                color: Theme.alpha(Theme.muted, 0.45)
                Row {
                    anchors.centerIn: parent
                    spacing: 2
                    Rectangle {
                        width: 52
                        height: 30
                        radius: 15
                        color: Theme.alpha(Theme.muted, 0.9)
                        Icon { anchors.centerIn: parent; name: "smile"; size: 17; color: Theme.foreground }
                    }
                    Rectangle {
                        width: 52
                        height: 30
                        radius: 15
                        color: switchStickerMouse.containsMouse ? Theme.alpha(Theme.muted, 0.9) : "transparent"
                        Icon { anchors.centerIn: parent; name: "sticker"; size: 17; color: Theme.mutedForeground }
                        MouseArea {
                            id: switchStickerMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.switchToStickers()
                        }
                    }
                }
            }
        }
    }
}
