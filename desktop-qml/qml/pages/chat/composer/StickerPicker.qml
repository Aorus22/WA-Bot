import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import "../../../ui"

Popup {
    id: root
    objectName: "stickerPicker"
    signal switchToEmoji()

    parent: Overlay.overlay
    width: 440
    height: 392
    padding: 8
    modal: false
    focus: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    property string tab: "recent"
    readonly property var stickers: tab === "starred" ? Composer.stickersStarred
        : tab === "custom" ? Composer.stickersCustom : Composer.stickersRecent

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

    onOpened: Composer.loadStickers()

    ColumnLayout {
        anchors.fill: parent
        spacing: 4

        Row {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredHeight: 28
            spacing: 0
            Repeater {
                model: [
                    { key: "recent", label: "Recent", icon: "clock" },
                    { key: "starred", label: "Starred", icon: "star" },
                    { key: "custom", label: "My stickers", icon: "heart" }
                ]
                delegate: Item {
                    required property var modelData
                    width: 52
                    height: 28
                    Column {
                        anchors.centerIn: parent
                        spacing: 1
                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: String(modelData.icon)
                            size: 20
                            color: root.tab === modelData.key ? Theme.foreground : Theme.mutedForeground
                        }
                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 26
                            height: 3
                            radius: 2
                            color: root.tab === modelData.key ? Theme.primary : "transparent"
                        }
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: String(modelData.label)
                    MouseArea {
                        id: tabMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.tab = modelData.key
                    }
                    ToolTipBubble { visible: tabMouse.containsMouse; text: String(modelData.label) }
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            GridView {
                id: grid
                objectName: "stickerPickerGrid"
                anchors.fill: parent
                clip: true
                cellWidth: 104
                cellHeight: 104
                model: root.stickers
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    objectName: "stickerPickerCell_" + index
                    width: 96
                    height: 96
                    radius: Theme.rem * 0.75
                    color: stickerMouse.containsMouse ? Theme.alpha(Theme.muted, 0.6) : Theme.alpha(Theme.muted, 0.3)
                    Image {
                        anchors.fill: parent
                        anchors.margins: 4
                        source: String(modelData.url || "")
                        asynchronous: true
                        fillMode: Image.PreserveAspectFit
                    }
                    MouseArea {
                        id: stickerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Composer.sendSticker(String(modelData.mediaUrl || ""), Boolean(modelData.isAnimated))
                            root.close()
                        }
                    }
                }
            }

            Column {
                width: parent.width
                anchors.top: parent.top
                anchors.topMargin: 32
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 8
                visible: !Composer.stickersLoading && root.stickers.length === 0
                AppText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - 32, 360)
                    text: root.tab === "recent" ? "Stickers you send will show up here."
                        : root.tab === "starred" ? "Star a sticker in WhatsApp on your phone and it will show up here."
                        : "Right-click a sticker in a chat and choose Add to favourites."
                    size: 0.875
                    color: Theme.mutedForeground
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                }
            }
            Spinner {
                anchors.horizontalCenter: parent.horizontalCenter
                y: 32
                visible: Composer.stickersLoading
                size: 22
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
                        color: switchEmojiMouse.containsMouse ? Theme.alpha(Theme.muted, 0.9) : "transparent"
                        Icon { anchors.centerIn: parent; name: "smile"; size: 17; color: Theme.mutedForeground }
                        MouseArea {
                            id: switchEmojiMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.switchToEmoji()
                        }
                    }
                    Rectangle {
                        width: 52
                        height: 30
                        radius: 15
                        color: Theme.alpha(Theme.muted, 0.9)
                        Icon { anchors.centerIn: parent; name: "sticker"; size: 17; color: Theme.foreground }
                    }
                }
            }
        }
    }
}
