import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"

Rectangle {
    id: root
    color: Theme.glass(Theme.background)

    function selectedCallMetadata() {
        const log = Calls.selectedLog
        const rows = [
            { label: "Status", value: String(log.statusLabel || "Unknown") },
            { label: "Direction", value: String(log.directionLabel || "Unknown") },
            { label: "Type", value: String(log.typeLabel || "Unknown") },
            { label: "Duration", value: String(log.durationLabel || "—") },
            { label: "Started", value: String(log.startedFullLabel || log.timeLabel || "—") }
        ]
        if (log.ended_at) rows.push({ label: "Ended", value: String(log.endedFullLabel || "—") })
        rows.push({ label: "Target", value: String(log.target || "—") })
        if (log.group_jid) rows.push({ label: "Group", value: String(log.group_jid) })
        if (log.error_message) rows.push({ label: "Error", value: String(log.error_message) })
        rows.push({ label: "ID", value: String(log.id || "—") })
        return rows
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.preferredWidth: 360
            Layout.minimumWidth: 300
            Layout.fillHeight: true
            color: Theme.glass(Theme.background)

            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.margins: 20
                    Layout.bottomMargin: 19
                    spacing: 16

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 33
                        Layout.bottomMargin: 4
                        AppText { Layout.topMargin: 3; text: "Calls"; size: 1.5; semibold: true }
                        AppText { Layout.leftMargin: 10; text: String(Calls.total); size: 0.875; color: Theme.mutedForeground }
                        Item { Layout.fillWidth: true }
                        IconButton {
                            icon: "refresh-cw"
                            iconSize: 16
                            pad: 6
                            tooltip: "Try again"
                            onClicked: Calls.refresh()
                        }
                    }

                    AppTextField {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 34
                        leadingIcon: "search"
                        placeholder: "Search calls"
                        clearable: true
                        onTextChanged: Calls.setQuery(text)
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: [
                                { key: "all", label: "All" },
                                { key: "incoming", label: "Incoming" },
                                { key: "outgoing", label: "Outgoing" },
                                { key: "missed", label: "Missed" }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool active: Calls.filter === modelData.key
                                objectName: "callsFilter_" + modelData.key
                                Layout.preferredWidth: label.implicitWidth + 28
                                Layout.preferredHeight: 30
                                radius: height / 2
                                color: active ? Theme.primary : Theme.alpha(Theme.muted, chipMouse.containsMouse ? 0.8 : 0.5)

                                AppText {
                                    id: label
                                    anchors.centerIn: parent
                                    text: modelData.label
                                    size: 0.75
                                    semibold: true
                                    color: parent.active ? Theme.primaryForeground
                                        : chipMouse.containsMouse ? Theme.foreground : Theme.mutedForeground
                                }

                                MouseArea {
                                    id: chipMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Calls.setFilter(modelData.key)
                                }
                            }
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.alpha(Theme.border, 0.4) }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        id: history
                        anchors.fill: parent
                        clip: true
                        model: Calls.history
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ThinScrollBar {}
                        header: Item { width: history.width; height: history.count > 0 ? 12 : 0 }
                        delegate: Item {
                            id: rowRoot
                            required property var model
                            required property int index
                            readonly property bool showDate: {
                                if (index <= 0) return true
                                const previous = Calls.history.get(index - 1)
                                return !previous || previous.dateLabel !== model.dateLabel
                            }
                            width: history.width
                            height: (showDate ? 36 : 0) + 72

                            AppText {
                                visible: rowRoot.showDate
                                x: 16
                                y: 8
                                text: String(rowRoot.model.dateLabel || "Earlier")
                                size: 0.75
                                semibold: true
                                color: Theme.mutedForeground
                            }

                            Rectangle {
                                id: row
                                x: 8
                                y: rowRoot.showDate ? 36 : 0
                                width: rowRoot.width - 16
                                height: 66
                                radius: Theme.rem * 0.75
                                color: Calls.selectedLog.id === rowRoot.model.id
                                    ? Theme.alpha(Theme.primary, 0.14)
                                    : rowMouse.containsMouse ? Theme.alpha(Theme.muted, 0.45) : "transparent"

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 10
                                    spacing: 12
                                    Rectangle {
                                        Layout.preferredWidth: 42
                                        Layout.preferredHeight: 42
                                        radius: 21
                                        color: rowRoot.model.missed ? Theme.alpha(Theme.destructive, 0.12) : Theme.alpha(Theme.primary, 0.12)
                                        Icon {
                                            anchors.centerIn: parent
                                            name: rowRoot.model.missed ? "phone-missed"
                                                : rowRoot.model.direction === "incoming" ? "arrow-down-left" : "arrow-up-right"
                                            size: 18
                                            color: rowRoot.model.missed ? Theme.destructive : Theme.primary
                                        }
                                        Icon {
                                            anchors.right: parent.right
                                            anchors.bottom: parent.bottom
                                            anchors.rightMargin: -2
                                            anchors.bottomMargin: -2
                                            visible: String(rowRoot.model.call_type || "").indexOf("video") >= 0
                                            name: "video"
                                            size: 12
                                            color: Theme.mutedForeground
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 3
                                        AppText {
                                            Layout.fillWidth: true
                                            text: String(rowRoot.model.displayName || "Unknown")
                                            size: 0.875
                                            semibold: true
                                            elide: Text.ElideRight
                                        }
                                        AppText {
                                            Layout.fillWidth: true
                                            text: rowRoot.model.missed ? "Missed · " + String(rowRoot.model.durationLabel || "—")
                                                : String(rowRoot.model.directionLabel || "") + " · "
                                                    + (String(rowRoot.model.call_type || "").indexOf("video") >= 0 ? "Video" : "Voice")
                                                    + " · " + String(rowRoot.model.durationLabel || "—")
                                            size: 0.6875
                                            color: rowRoot.model.missed ? Theme.destructive : Theme.mutedForeground
                                            elide: Text.ElideRight
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                                        spacing: 4
                                        AppText { Layout.alignment: Qt.AlignRight; text: String(rowRoot.model.timeLabel || ""); size: 0.6875; color: Theme.mutedForeground }
                                        Rectangle {
                                            visible: Boolean(rowRoot.model.missed)
                                            Layout.preferredWidth: 8
                                            Layout.preferredHeight: 8
                                            Layout.alignment: Qt.AlignRight
                                            radius: 4
                                            color: Theme.destructive
                                        }
                                    }
                                }
                                MouseArea {
                                    id: rowMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Calls.selectLog(String(rowRoot.model.id || ""))
                                }
                            }
                        }
                        footer: Item {
                            width: history.width
                            height: Calls.hasMore ? 56 : 12
                            Btn {
                                anchors.centerIn: parent
                                visible: Calls.hasMore
                                text: Calls.loadingMore ? "Loading…" : "Load more"
                                icon: Calls.loadingMore ? "" : "chevron-down"
                                variant: "ghost"
                                size: 0.75
                                enabled: !Calls.loadingMore
                                onClicked: Calls.loadMore()
                            }
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        width: parent.width - 40
                        spacing: 8
                        visible: Calls.history.count === 0
                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: Calls.loading ? "loader" : Calls.historyError ? "phone-off" : "phone"
                            size: 30
                            color: Calls.historyError ? Theme.destructive : Theme.mutedForeground
                        }
                        AppText {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: Calls.loading ? "Loading calls…" : Calls.historyError ? "Couldn't load calls" : "No calls yet"
                            size: 0.875
                            semibold: true
                        }
                        AppText {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            text: Calls.historyError || (Calls.query.trim() ? "Nothing matches your search." : "Your call history will appear here.")
                            size: 0.75
                            color: Theme.mutedForeground
                            wrapMode: Text.WordWrap
                        }
                        Btn {
                            anchors.horizontalCenter: parent.horizontalCenter
                            visible: Calls.historyError !== ""
                            text: "Try again"
                            variant: "outline"
                            size: 0.75
                            onClicked: Calls.refresh()
                        }
                    }
                }
            }
        }

        Rectangle { Layout.fillHeight: true; Layout.preferredWidth: 1; color: Theme.alpha(Theme.border, 0.4) }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                visible: Calls.selectedLog.id !== undefined && Calls.selectedLog.id !== ""
                anchors.fill: parent
                anchors.leftMargin: 26
                anchors.rightMargin: 23
                anchors.topMargin: 21
                anchors.bottomMargin: 24
                spacing: 24

                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "CALL DETAILS"; size: 0.75; semibold: true; color: Theme.mutedForeground }
                    IconButton { icon: "x"; tooltip: "Close details"; onClicked: Calls.clearSelection() }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 258
                    radius: Theme.rem * 0.875
                    color: Theme.card
                    border.width: 1
                    border.color: Theme.alpha(Theme.border, 0.65)
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 24
                        spacing: 12
                        Rectangle {
                            Layout.alignment: Qt.AlignHCenter
                            Layout.preferredWidth: 68
                            Layout.preferredHeight: 68
                            radius: 34
                            color: Calls.selectedLog.missed ? Theme.alpha(Theme.destructive, 0.12) : Theme.alpha(Theme.primary, 0.12)
                            Icon {
                                anchors.centerIn: parent
                                visible: Boolean(Calls.selectedLog.missed)
                                name: "phone-missed"
                                size: 32
                                color: Calls.selectedLog.missed ? Theme.destructive : Theme.primary
                            }
                            AppText {
                                anchors.centerIn: parent
                                visible: !Calls.selectedLog.missed
                                text: String(Calls.selectedLog.initial || "WA")
                                size: 1.25
                                semibold: true
                                color: Theme.primary
                            }
                        }
                        AppText {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            text: String(Calls.selectedLog.displayName || "Unknown")
                            size: 1.25
                            font.weight: Font.Bold
                            elide: Text.ElideRight
                        }
                        AppText {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            text: String(Calls.selectedLog.missed ? "Missed call" : Calls.selectedLog.directionLabel || "")
                                + " · " + String(Calls.selectedLog.typeLabel || "Voice")
                            size: 0.875
                            color: Calls.selectedLog.missed ? Theme.destructive : Theme.mutedForeground
                        }
                        RowLayout {
                            Layout.alignment: Qt.AlignHCenter
                            Layout.topMargin: 8
                            spacing: 12
                            Btn {
                                text: "View chat"
                                icon: "message-square"
                                variant: "outline"
                                Layout.preferredHeight: 40
                                horizontalPadding: Theme.rem
                                radius: Theme.rem * 0.75
                                onClicked: Calls.openChat(String(Calls.selectedLog.target || ""))
                            }
                            Btn {
                                text: "Call back"
                                icon: "phone"
                                Layout.preferredHeight: 40
                                horizontalPadding: Theme.rem
                                radius: Theme.rem * 0.75
                                onClicked: Calls.callBack(String(Calls.selectedLog.id || ""))
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: Theme.rem * 0.75
                    color: Theme.card
                    border.width: 1
                    border.color: Theme.alpha(Theme.border, 0.65)
                    Flickable {
                        anchors.fill: parent
                        anchors.margins: 16
                        contentWidth: width
                        contentHeight: metadata.implicitHeight
                        clip: true
                        ColumnLayout {
                            id: metadata
                            width: parent.width
                            spacing: 0
                            Repeater {
                                model: root.selectedCallMetadata()
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.minimumHeight: 36
                                    AppText { Layout.fillWidth: true; text: modelData.label; size: 0.75; color: Theme.mutedForeground }
                                    AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignRight; text: modelData.value; size: 0.75; medium: true; elide: Text.ElideMiddle }
                                }
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: parent.width - 48
                visible: Calls.selectedLog.id === undefined || Calls.selectedLog.id === ""
                spacing: 16
                Rectangle {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: 80
                    Layout.preferredHeight: 80
                    radius: 40
                    color: Theme.alpha(Theme.muted, 0.3)
                    Icon { anchors.centerIn: parent; name: "phone"; size: 40; color: Theme.alpha(Theme.mutedForeground, 0.5) }
                }
                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "Select a call"; size: 1.5; font.weight: Font.Bold }
                AppText { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "Select a call from history to view details."; size: 0.8125; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
            }
        }
    }
}
