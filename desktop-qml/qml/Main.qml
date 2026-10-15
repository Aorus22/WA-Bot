import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 1100
    height: 720
    minimumWidth: 800
    minimumHeight: 540
    visible: true
    title: "WA Bot Desktop"

    property string authState: "unknown"   // unknown | out | in
    property string qr: ""
    property string phone: ""

    function refreshStatus() {
        api.request("GET", "/status", "", "status")
    }

    Connections {
        target: backend
        function onApiUrlChanged() { win.refreshStatus() }
    }
    Connections {
        target: api
        function onFinished(token, status, body) {
            if (token === "status" && status === 200) {
                const s = JSON.parse(body)
                win.phone = s.phone || ""
                win.authState = s.isLoggedIn ? "in" : "out"
                if (!s.isLoggedIn) api.request("GET", "/qr-code", "", "qr")
            } else if (token === "qr" && status === 200) {
                win.qr = JSON.parse(body).code || ""
            }
        }
    }
    Connections {
        target: events
        function onEvent(type, payload) {
            const p = JSON.parse(payload)
            if (type === "qr_code") { win.qr = p.code || ""; win.authState = "out" }
            else if (type === "auth_success") { win.authState = "in"; win.phone = p.phone || "" }
            else if (type === "logged_out") { win.authState = "out"; win.refreshStatus() }
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 8
        Label { text: "WA Bot"; font.pixelSize: 28; Layout.alignment: Qt.AlignHCenter }
        Label { text: "backend: " + backend.state + (backend.apiUrl ? "  " + backend.apiUrl : ""); Layout.alignment: Qt.AlignHCenter }
        Label { visible: backend.error !== ""; text: backend.error; color: "tomato"; Layout.alignment: Qt.AlignHCenter }
        Label { text: "websocket: " + (events.connected ? "connected" : "disconnected"); Layout.alignment: Qt.AlignHCenter }
        Label { text: "account: " + win.authState + (win.phone ? "  " + win.phone : ""); Layout.alignment: Qt.AlignHCenter }
        TextArea {
            visible: win.qr !== ""
            readOnly: true
            text: win.qr
            Layout.preferredWidth: 420
            wrapMode: TextEdit.WrapAnywhere
            Layout.alignment: Qt.AlignHCenter
        }
    }
}
