import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"
import "."

Rectangle {
    id: root
    color: Theme.glass(Theme.background)

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.preferredWidth: 340
            Layout.fillHeight: true
            color: Theme.glass(Theme.background)

            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 64
                    Layout.leftMargin: 20
                    Layout.rightMargin: 16
                    AppText { Layout.fillWidth: true; text: "Channels"; size: 1.5; semibold: true }
                    IconButton { objectName: "channelFindButton"; icon: "plus"; tooltip: "Follow a channel"; iconSize: 20; pad: 8; onClicked: discover.open() }
                }
                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    ListView {
                        id: channelList
                        anchors.fill: parent
                        clip: true
                        spacing: 4
                        model: Channels.model
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ThinScrollBar {}
                        header: Item { width: channelList.width; height: 12 }
                        delegate: ChannelRow {
                            required property var model
                            width: channelList.width
                            channel: model
                            selected: Channels.selectedJid === String(model.jid || "")
                            onOpened: Channels.select(String(channel.jid || ""))
                        }
                        footer: Column {
                            width: channelList.width
                            spacing: 0

                            Item {
                                width: parent.width
                                height: visible ? 84 : 0
                                visible: Channels.loading && Channels.model.count === 0
                                Column {
                                    anchors.centerIn: parent
                                    spacing: 8
                                    Spinner { anchors.horizontalCenter: parent.horizontalCenter; size: 20 }
                                    AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "Loading channels…"; size: 0.75; color: Theme.mutedForeground }
                                }
                            }

                            Item { width: parent.width; height: 16 }

                            Item {
                                width: parent.width
                                height: 44
                                AppText {
                                    x: 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "FIND CHANNELS"
                                    size: 0.6875
                                    semibold: true
                                    color: Theme.mutedForeground
                                }
                            }

                            Item {
                                x: 12
                                width: parent.width - 24
                                height: 68
                                Rectangle {
                                    anchors.fill: parent
                                    radius: Theme.rem * 0.75
                                    color: findMouse.containsMouse ? Theme.alpha(Theme.muted, 0.4) : "transparent"
                                }
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 12
                                    Rectangle {
                                        Layout.preferredWidth: 44
                                        Layout.preferredHeight: 44
                                        radius: 22
                                        color: Theme.alpha(Theme.primary, 0.12)
                                        Icon { anchors.centerIn: parent; name: "search"; size: 20; color: Theme.primary }
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2
                                        AppText { Layout.fillWidth: true; text: "Follow via link"; size: 0.875; semibold: true }
                                        AppText { Layout.fillWidth: true; text: "Paste a whatsapp.com/channel/… link"; size: 0.6875; color: Theme.mutedForeground; elide: Text.ElideRight }
                                    }
                                }
                                MouseArea {
                                    id: findMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: discover.open()
                                }
                            }
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 8
                        visible: !Channels.loading && Channels.model.count === 0 && Channels.error === ""
                        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "megaphone"; size: 34; color: Theme.mutedForeground }
                        AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "No channels followed"; size: 0.8125; color: Theme.mutedForeground }
                    }
                    AppText {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.top
                        anchors.topMargin: 8
                        visible: Channels.error !== ""
                        text: Channels.error
                        size: 0.75
                        color: Theme.destructive
                    }
                }

            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Theme.glass(Theme.background)
            Rectangle { anchors.left: parent.left; height: parent.height; width: 1; color: Theme.alpha(Theme.border, 0.4) }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                visible: Channels.selectedJid !== ""

                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 64
                    Layout.leftMargin: 16
                    Layout.rightMargin: 16
                    spacing: 12
                    Avatar {
                        chatId: String(Channels.selected.jid || "")
                        name: String(Channels.selected.name || "")
                        avatar: String(Channels.selected.avatar || "")
                        size: 40
                        variant: "tinted"
                        showImage: String(Channels.selected.avatar || "") !== ""
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 5
                            AppText { Layout.fillWidth: true; text: String(Channels.selected.name || ""); size: 0.9375; semibold: true; elide: Text.ElideRight }
                            AppText { visible: Boolean(Channels.selected.verified); text: "✓"; color: Theme.primary; semibold: true }
                        }
                        AppText { Layout.fillWidth: true; text: String(Channels.selected.subscribersLabel || "") + " followers"; size: 0.6875; color: Theme.mutedForeground }
                    }
                    IconButton {
                        objectName: "channelMuteToggle"
                        icon: Channels.selected.muted ? "volume-x" : "volume-2"
                        tooltip: Channels.selected.muted ? "Unmute" : "Mute"
                        onClicked: Channels.toggleMute()
                    }
                    IconButton { icon: "user-minus"; tooltip: "Unfollow"; iconColor: Theme.destructive; onClicked: unfollowDialog.open() }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        id: feed
                        anchors.fill: parent
                        anchors.leftMargin: 24
                        anchors.rightMargin: 24
                        clip: true
                        model: Channels.posts
                        boundsBehavior: Flickable.StopAtBounds
                        spacing: 8
                        ScrollBar.vertical: ThinScrollBar {}
                        delegate: ChannelPost {
                            required property var model
                            width: feed.width
                            post: model
                            channelName: String(Channels.selected.name || "")
                            quickReactions: Channels.quickReactions
                            onReacted: emoji => Channels.react(String(post.id || ""), emoji)
                        }
                        header: Item {
                            width: feed.width
                            height: Channels.loadingMore ? 64 : Channels.hasMore ? 64 : 12
                            Row {
                                anchors.centerIn: parent
                                spacing: 8
                                Spinner { visible: Channels.loadingMore; size: 18; anchors.verticalCenter: parent.verticalCenter }
                                Btn {
                                    visible: !Channels.loadingMore && Channels.hasMore
                                    text: "Load older posts"
                                    variant: "outline"
                                    size: 0.75
                                    onClicked: Channels.loadMore()
                                }
                            }
                        }
                    }

                    Connections {
                        target: Channels
                        function onPostAppended() {
                            if (feed.atYEnd)
                                Qt.callLater(() => feed.positionViewAtEnd())
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 8
                        visible: Channels.postsLoading && Channels.posts.count === 0
                        Spinner { anchors.horizontalCenter: parent.horizontalCenter; size: 22 }
                        AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "Loading posts…"; size: 0.75; color: Theme.mutedForeground }
                    }
                    AppText {
                        anchors.centerIn: parent
                        visible: !Channels.postsLoading && Channels.posts.count === 0
                        text: Channels.postsError || "No posts yet"
                        size: 0.875
                        color: Channels.postsError ? Theme.destructive : Theme.mutedForeground
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }
                AppText {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 42
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: "Channels are one-way — you can react to posts but not reply."
                    size: 0.75
                    color: Theme.mutedForeground
                }
            }

            Column {
                anchors.centerIn: parent
                spacing: 10
                visible: Channels.selectedJid === ""
                Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "megaphone"; size: 42; color: Theme.alpha(Theme.primary, 0.45) }
                AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "Select a channel"; size: 1.125; semibold: true }
                AppText { anchors.horizontalCenter: parent.horizontalCenter; text: "Follow channels to see their posts here."; size: 0.8125; color: Theme.mutedForeground }
            }
        }
    }

    ChannelDiscoverDialog { id: discover }

    AppDialog {
        id: unfollowDialog
        title: "Unfollow " + String(Channels.selected.name || "channel") + "?"
        description: "You will stop receiving new posts from this channel."
        cardWidth: 380
        footer: [
            Btn { text: "Cancel"; variant: "outline"; onClicked: unfollowDialog.close() },
            Btn { text: "Unfollow"; variant: "destructive"; onClicked: { Channels.unfollow(); unfollowDialog.close() } }
        ]
    }
}
