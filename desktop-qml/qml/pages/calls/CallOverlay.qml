import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"

Item {
    id: root
    objectName: "callOverlay"
    visible: Calls.hasActiveCall
    z: 100

    readonly property var call: Calls.activeCall
    readonly property string callId: String(call.id || "")
    readonly property string target: String(call.group_jid || call.target || "")
    readonly property string peerName: Calls.displayNameFor(target)
    readonly property bool incoming: Calls.incomingCall.id !== undefined && Calls.incomingCall.id !== ""
    readonly property bool groupCall: String(call.group_jid || "") !== ""
    readonly property bool video: Boolean(call.video_enabled || call.remote_video_enabled)
    readonly property bool videoCall: root.video || String(call.type || "").endsWith("video")
    readonly property bool connected: String(call.status || "") === "connected"
    readonly property string peerInitials: {
        const words = root.peerName.trim().split(/\s+/)
        if (words.length > 1) return (words[0][0] + words[1][0]).toUpperCase()
        return root.peerName.slice(0, 2).toUpperCase()
    }

    Rectangle {
        anchors.fill: parent
        color: "#0a0a0a"
        opacity: root.incoming ? 0.70 : 0.92
    }

    Image {
        anchors.fill: parent
        visible: !root.incoming && root.video && Calls.remoteFrameUrl !== ""
        source: Calls.remoteFrameUrl
        asynchronous: true
        cache: false
        fillMode: Image.PreserveAspectCrop
        opacity: 0.55
    }

    Rectangle {
        anchors.fill: parent
        visible: !root.incoming
        gradient: Gradient {
            GradientStop { position: 0; color: "#aa090d12" }
            GradientStop { position: 0.45; color: "#22090d12" }
            GradientStop { position: 1; color: "#ee090d12" }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        anchors.leftMargin: 23
        anchors.topMargin: 20
        spacing: 0
        visible: !root.incoming

        RowLayout {
            Layout.fillWidth: true
            Rectangle {
                implicitWidth: callKind.implicitWidth + 24
                implicitHeight: callKind.implicitHeight + 8
                radius: height / 2
                color: Theme.alpha(Theme.foreground, 0.05)
                border.width: 1
                border.color: Theme.alpha(Theme.foreground, 0.10)
                RowLayout {
                    id: callKind
                    anchors.centerIn: parent
                    spacing: 8
                    Rectangle {
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        radius: 4
                        color: root.connected ? "#10b981" : "#f59e0b"
                    }
                    AppText {
                        text: (root.videoCall ? "VIDEO" : "VOICE") + " CALL"
                        size: 0.75
                        semibold: true
                        color: Theme.alpha(Theme.foreground, 0.70)
                    }
                }
            }
            Item { Layout.fillWidth: true }
        }

        Item { Layout.fillHeight: true; Layout.minimumHeight: 10 }

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.maximumWidth: 560
            Layout.fillWidth: true
            Layout.topMargin: 18
            spacing: 20
            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: 112
                Layout.preferredHeight: 112
                radius: 56
                color: "#18181b"
                border.width: 1
                border.color: Theme.alpha(root.connected ? "#10b981" : "#f59e0b", 0.50)
                AppText {
                    anchors.centerIn: parent
                    text: root.peerInitials
                    size: 2.25
                    color: Theme.foreground
                    font.weight: Font.Bold
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                AppText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: root.peerName
                    size: 1.5
                    color: Theme.alpha(Theme.foreground, 0.95)
                    semibold: true
                    elide: Text.ElideRight
                }
                AppText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: root.target
                    size: 0.75
                    color: Theme.alpha(Theme.foreground, 0.50)
                    elide: Text.ElideRight
                }
            }
        }

        Item { Layout.fillHeight: true; Layout.minimumHeight: 10 }

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.maximumWidth: 620
            Layout.fillWidth: true
            spacing: 14

            AppText {
                Layout.fillWidth: true
                visible: Calls.mediaNotice !== ""
                horizontalAlignment: Text.AlignHCenter
                text: Calls.mediaNotice
                size: 0.75
                color: "#f4c66a"
                wrapMode: Text.WordWrap
            }

            AppText {
                Layout.fillWidth: true
                visible: root.connected
                horizontalAlignment: Text.AlignHCenter
                text: Calls.elapsedText
                size: 1.125
                color: Theme.alpha(Theme.foreground, 0.75)
            }

            AppText {
                Layout.fillWidth: true
                visible: !root.connected && !root.incoming
                horizontalAlignment: Text.AlignHCenter
                text: String(root.call.status || "Connecting").replace(/_/g, " ").replace(/^./, c => c.toUpperCase()) + "…"
                size: 0.875
                color: Theme.alpha(Theme.foreground, 0.75)
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                visible: root.connected && String(root.call.group_jid || "") !== ""
                spacing: 8
                AppTextField {
                    id: participantJid
                    objectName: "callParticipantJid"
                    Layout.preferredWidth: 300
                    placeholder: "Add participant JID"
                }
                Btn {
                    objectName: "callAddParticipant"
                    text: "Add"
                    icon: "users"
                    enabled: participantJid.text.trim() !== ""
                    onClicked: {
                        Calls.addParticipants(root.callId, [participantJid.text.trim()])
                        participantJid.text = ""
                    }
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 10
                visible: root.incoming
                Btn {
                    text: "Decline"
                    icon: "phone-off"
                    variant: "destructive"
                    onClicked: Calls.reject(root.callId)
                }
                Btn {
                    text: "Answer"
                    icon: "phone"
                    onClicked: Calls.answer(root.callId)
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 20
                visible: !root.incoming
                CallRoundButton {
                    objectName: "activeCallMic"
                    label: Calls.micMuted ? "Unmute" : "Mute"
                    iconName: Calls.micMuted ? "mic-off" : "mic"
                    buttonColor: Calls.micMuted ? Theme.alpha("#f59e0b", 0.85) : Theme.alpha(Theme.foreground, 0.10)
                    hoverColor: Calls.micMuted ? Theme.alpha("#f59e0b", 0.95) : Theme.alpha(Theme.foreground, 0.16)
                    iconColor: Theme.alpha(Theme.foreground, 0.90)
                    onClicked: Calls.toggleMic()
                }
                CallRoundButton {
                    objectName: "activeCallSpeaker"
                    label: "Speaker"
                    iconName: Calls.speakerMuted ? "volume-x" : "volume-2"
                    buttonColor: Calls.speakerMuted ? Theme.alpha("#3f3f46", 0.60) : Theme.alpha(Theme.foreground, 0.10)
                    hoverColor: Calls.speakerMuted ? Theme.alpha("#3f3f46", 0.70) : Theme.alpha(Theme.foreground, 0.16)
                    iconColor: Calls.speakerMuted ? Theme.alpha(Theme.foreground, 0.50) : Theme.alpha(Theme.foreground, 0.90)
                    onClicked: Calls.toggleSpeaker()
                }
                CallRoundButton {
                    objectName: "activeCallVideo"
                    visible: !root.videoCall || root.video
                    label: root.video ? "Stop video" : "Video"
                    iconName: root.video ? "video-off" : "video"
                    buttonColor: root.video ? Theme.alpha("#0284c7", 0.85) : Theme.alpha(Theme.foreground, 0.10)
                    hoverColor: root.video ? Theme.alpha("#0284c7", 0.95) : Theme.alpha(Theme.foreground, 0.16)
                    iconColor: Theme.alpha(Theme.foreground, 0.90)
                    onClicked: Calls.toggleVideo()
                }
                CallRoundButton {
                    objectName: "activeCallEnd"
                    label: "End"
                    iconName: "phone-off"
                    buttonColor: "#ef4444"
                    hoverColor: "#dc2626"
                    iconColor: "#ffffff"
                    ringColor: "transparent"
                    onClicked: Calls.hangup(root.callId)
                }
            }
        }

        Item { Layout.preferredHeight: 10 }
    }

    Image {
        visible: !root.incoming && root.video && Calls.localFrameUrl !== ""
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: 32
        anchors.bottomMargin: 112
        width: 150
        height: 112
        source: Calls.localFrameUrl
        asynchronous: true
        cache: false
        fillMode: Image.PreserveAspectCrop
        clip: true
        Rectangle { anchors.fill: parent; color: "transparent"; border.width: 1; border.color: "#66ffffff"; radius: 10 }
    }

    Rectangle {
        visible: !root.incoming && Calls.videoUpgradeRequested.id !== undefined && Calls.videoUpgradeRequested.id !== ""
        anchors.top: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.topMargin: 64
        width: Math.min(parent.width - 32, 420)
        height: 96
        radius: Theme.rem * 0.75
        color: Theme.card
        border.width: 1
        border.color: Theme.alpha(Theme.border, 0.8)
        RowLayout {
            anchors.fill: parent
            anchors.margins: 14
            AppText { Layout.fillWidth: true; text: "They want to switch to video"; size: 0.875; semibold: true; wrapMode: Text.WordWrap }
            Btn { text: "Decline"; variant: "outline"; size: 0.75; onClicked: Calls.rejectVideo(root.callId) }
            Btn { text: "Accept"; size: 0.75; onClicked: Calls.acceptVideo(root.callId) }
        }
    }

    Rectangle {
        id: incomingCard
        visible: root.incoming
        anchors.centerIn: parent
        width: 360
        height: incomingContent.implicitHeight + 51
        radius: 16
        color: Theme.card
        border.width: 1
        border.color: Theme.border

        ColumnLayout {
            id: incomingContent
            anchors.centerIn: parent
            width: parent.width - 48
            spacing: 11

            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                implicitWidth: incomingBadge.implicitWidth + 24
                implicitHeight: incomingBadge.implicitHeight + 8
                Layout.bottomMargin: 2
                radius: height / 2
                color: Theme.alpha(Theme.muted, 0.60)
                RowLayout {
                    id: incomingBadge
                    anchors.centerIn: parent
                    spacing: 8
                    Rectangle {
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        radius: 4
                        color: "#10b981"
                    }
                    AppText {
                        text: "INCOMING " + (root.video ? "VIDEO" : "VOICE") + " CALL"
                        size: 0.75
                        semibold: true
                        color: Theme.mutedForeground
                    }
                }
            }

            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                width: 92
                height: 92
                radius: 46
                color: "#18181b"
                border.width: 1
                border.color: Theme.border
                AppText {
                    anchors.centerIn: parent
                    text: root.peerInitials
                    size: 1.75
                    color: Theme.foreground
                    font.weight: Font.Bold
                }
            }

            Item { Layout.preferredHeight: 5 }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 10
                AppText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: root.peerName
                    size: 1.5
                    color: Theme.foreground
                    semibold: true
                    elide: Text.ElideRight
                }
                AppText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: root.target
                    size: 0.75
                    color: Theme.mutedForeground
                    elide: Text.ElideMiddle
                }
            }

            AppText {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: "Ringing…"
                size: 0.875
                color: Theme.mutedForeground
            }

            Item { Layout.preferredHeight: 8 }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 24
                CallRoundButton {
                    objectName: "incomingCallDecline"
                    label: "Decline"
                    iconName: "phone-off"
                    diameter: 64
                    buttonColor: "#ef4444"
                    hoverColor: "#dc2626"
                    iconColor: "#ffffff"
                    labelColor: Theme.alpha(Theme.foreground, 0.55)
                    ringColor: "transparent"
                    onClicked: Calls.reject(root.callId)
                }
                CallRoundButton {
                    objectName: "incomingCallAccept"
                    label: "Accept"
                    iconName: "phone"
                    diameter: 64
                    buttonColor: "#10b981"
                    hoverColor: "#059669"
                    iconColor: "#ffffff"
                    labelColor: Theme.alpha(Theme.foreground, 0.55)
                    ringColor: "transparent"
                    onClicked: Calls.answer(root.callId)
                }
            }
        }
    }
}
