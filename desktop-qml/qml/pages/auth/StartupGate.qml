import QtQuick
import QtQuick.Layouts
import "../../ui"

// Start-up screen drawn over the nav rail + pages (below the 48px titlebar), like GPUI's RootGateView while
// auth is `Checking`: a 40px primary spinner and "Connecting to WA Bot...". GPUI has no failure screen, so when
// the backend cannot be resolved (Backend.state === "error") or keeps refusing /status, an actionable one with
// the reason and a Retry button takes over.
Rectangle {
    id: root
    readonly property bool backendError: Backend.state === "error"
    readonly property bool unresponsive: AppState.auth === "checking" && Auth.checkFailures >= 3
    readonly property bool failed: backendError || unresponsive
    readonly property bool active: backendError || AppState.auth === "checking"

    visible: active
    color: Theme.background
    MouseArea { anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.AllButtons }  // nothing leaks to the shell below

    ColumnLayout {
        anchors.centerIn: parent
        visible: !root.failed
        spacing: Theme.rem * 0.75
        Spinner { size: 40; color: Theme.primary; Layout.alignment: Qt.AlignHCenter }
        AuthText { text: "Connecting to WA Bot..."; color: Theme.mutedForeground; Layout.alignment: Qt.AlignHCenter }
    }

    Flickable {
        id: fl
        anchors.fill: parent
        visible: root.failed
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: width
        contentHeight: Math.max(height, box.implicitHeight + 32)
        ColumnLayout {
            id: box
            width: Math.min(480, fl.width - 32)
            x: (fl.width - width) / 2
            y: Math.max(16, (fl.contentHeight - implicitHeight) / 2)
            spacing: Theme.rem * 1.5

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Theme.rem * 0.25
                Rectangle {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: 56; Layout.preferredHeight: 56
                    Layout.bottomMargin: Theme.rem * 0.5
                    radius: 16
                    color: Theme.accent
                    Icon { anchors.centerIn: parent; name: "triangle-alert"; size: 28; color: Theme.destructive }
                }
                AuthText {
                    Layout.alignment: Qt.AlignHCenter
                    size: 1.5; font.weight: Font.Bold
                    text: root.backendError ? "Can't reach the backend" : "The backend isn't responding"
                }
                AuthText {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    color: Theme.mutedForeground
                    text: root.backendError
                        ? "No running WA Bot backend was found and a new one could not be started."
                        : "The backend at " + Backend.apiUrl + " did not answer. Retrying automatically."
                }
            }

            Rectangle {
                Layout.fillWidth: true
                visible: reason.text !== ""
                implicitHeight: reason.implicitHeight + Theme.rem * 1.5
                radius: 12
                color: Theme.muted
                border.width: 1; border.color: Theme.border
                TextEdit {
                    id: reason
                    anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: Theme.rem * 0.75 }
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    color: Theme.foreground
                    selectionColor: Theme.alpha(Theme.primary, 0.35)
                    selectedTextColor: Theme.foreground
                    font.family: Theme.fontMono
                    font.pixelSize: Math.round(Theme.rem * 0.8125)
                    text: root.backendError ? Backend.error : Auth.checkError
                }
            }

            AuthText {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                size: 0.8125
                color: Theme.mutedForeground
                visible: root.backendError
                text: "Start the WA Bot backend, or point this app at it with --url http://127.0.0.1:PORT/api (or --port N), then retry."
            }

            Btn {
                Layout.alignment: Qt.AlignHCenter
                text: "Retry"
                icon: "refresh-cw"
                loading: root.backendError ? Backend.state === "resolving" : false
                onClicked: root.backendError ? Backend.start() : Auth.retryCheck()
            }
        }
    }
}
