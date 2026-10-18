import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"
import "."

Rectangle {
    id: root
    objectName: "statusPage"
    color: Theme.glass(Theme.background)

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.preferredWidth: 340
            Layout.fillHeight: true
            color: Theme.glass(Theme.background)
            border.width: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 64
                    Layout.leftMargin: 20
                    Layout.rightMargin: 20
                    AppText { Layout.fillWidth: true; text: "Status"; size: 1.5; font.weight: Font.Bold }
                    IconButton {
                        objectName: "statusNewButton"
                        icon: "plus"
                        tooltip: "New status"
                        iconSize: 20
                        pad: 8
                        onClicked: composer.open()
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }

                ListView {
                    id: statusList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 0
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ThinScrollBar {}

                    header: Column {
                        width: statusList.width
                        spacing: 0
                        Item { width: parent.width; height: 12 }
                        Item {
                            width: parent.width
                            height: Statuses.loading && Statuses.model.count === 0 ? 46 : 0
                            AppText {
                                anchors.centerIn: parent
                                text: "Memuat…"
                                size: 0.75
                                color: Theme.mutedForeground
                            }
                        }
                        Item { width: parent.width; height: Statuses.loading && Statuses.model.count === 0 ? 4 : 0 }
                        Item {
                            width: parent.width
                            height: 72
                            StatusStoryRow {
                                width: parent.width
                                height: 72
                                own: true
                                group: Statuses.own
                                onOpened: Statuses.openOwn()
                            }
                        }
                        Item { width: parent.width; height: Statuses.model.count > 0 ? 20 : 0 }
                        RowLayout {
                            visible: Statuses.model.count > 0
                            width: parent.width
                            height: visible ? 30 : 0
                            spacing: 0
                            AppText {
                                Layout.leftMargin: 24
                                text: "RECENT"
                                size: 0.6875
                                font.weight: Font.Bold
                                color: Theme.mutedForeground
                            }
                        }
                    }

                    model: Statuses.model
                    delegate: StatusStoryRow {
                        required property var model
                        width: statusList.width
                        group: model
                        onOpened: Statuses.openGroup(group.sender)
                    }

                    footer: Item { width: statusList.width; height: 12 }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Theme.glass(Theme.background)
            Rectangle { anchors.left: parent.left; width: 1; height: parent.height; color: Theme.alpha(Theme.border, 0.4) }

            Item {
                anchors.fill: parent
                StatusViewer {
                    id: statusViewer
                    anchors.fill: parent
                    visible: Statuses.viewerOpen
                    opacity: Statuses.viewerOpen ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: 200; easing.type: Easing.OutQuint } }
                }

                Column {
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 48, 500)
                    spacing: 16
                    visible: !Statuses.viewerOpen
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 80
                        height: 80
                        radius: 40
                        color: Theme.alpha(Theme.muted, 0.3)
                        Icon { anchors.centerIn: parent; name: "circle-dashed"; size: 48; color: Theme.mutedForeground }
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Bagikan status"
                        size: 1.875
                        font.weight: Font.Bold
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        text: "Bagikan foto, video, dan teks yang hilang setelah 24 jam."
                        color: Theme.mutedForeground
                        size: 0.875
                    }
                }
            }
        }
    }

    StatusComposer { id: composer }

    Connections {
        target: Statuses
        function onComposeRequested() { composer.open() }
    }
}
