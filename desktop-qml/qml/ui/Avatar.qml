import QtQuick
import QtQuick.Effects

// Round contact/group avatar, same rules as GPUI `render_avatar` / the chat-list avatar.
//   Avatar { chatId: jid; name: "Budi"; avatar: row.avatar; isGroup: false; size: 40 }
//   variant "tinted" (default, `render_avatar`): initial (or a group icon) on a 15-18% tint of the id colour.
//   variant "solid" (chat list row): white bold initial on the solid id colour.
// The picture (the chat's `avatar` field, or `<api>/avatar/<jid>` when empty) is painted over the
// fallback only once it has loaded, so a missing picture simply keeps the initial.
Item {
    id: root
    property string chatId: ""
    property string name: ""
    property string avatar: ""
    property bool isGroup: false
    property real size: 40
    property string variant: "tinted"
    property bool showImage: true

    // GPUI AVATAR_COLORS; index = (sum of the id's bytes) mod 8.
    readonly property var palette: ["#00a884", "#0284c7", "#6366f1", "#8b5cf6", "#ec4899", "#f59e0b", "#10b981", "#14b8a6"]
    readonly property color baseColor: {
        let s = unescape(encodeURIComponent(root.chatId || ""));
        let sum = 0;
        for (let i = 0; i < s.length; i++) sum += s.charCodeAt(i);
        return palette[sum % palette.length];
    }
    readonly property string initial: {
        const chars = Array.from(root.name || "");
        if (chars.length === 0) return root.variant === "solid" ? "#" : "?";
        return chars[0].toUpperCase();
    }
    readonly property string imageUrl: {
        if (!root.showImage || (root.chatId === "" && root.avatar === "")) return "";
        const a = (root.avatar || "").trim();
        if (a !== "" && a.indexOf("data:") !== 0) {
            if (a.indexOf("http://") === 0 || a.indexOf("https://") === 0) return a;
            return Api.mediaUrl(a.charAt(0) === "/" ? a : "/" + a);
        }
        return Api.mediaUrl("/avatar/" + Api.enc(root.chatId));
    }

    implicitWidth: size
    implicitHeight: size
    width: size
    height: size

    Rectangle {
        id: base
        anchors.fill: parent
        radius: width / 2
        color: root.variant === "solid" ? root.baseColor
             : root.isGroup ? Theme.alpha(Theme.primary, 0.15) : Theme.alpha(root.baseColor, 0.18)

        Icon {
            visible: root.variant !== "solid" && root.isGroup
            anchors.centerIn: parent
            name: "users"
            size: root.size * 0.46
            color: Theme.primary
        }
        Text {
            visible: !(root.variant !== "solid" && root.isGroup)
            anchors.centerIn: parent
            text: root.initial
            font.bold: true
            font.pixelSize: root.variant === "solid" ? Math.round(Theme.rem * 0.875) : Math.round(root.size * 0.38)
            color: root.variant === "solid" ? "#ffffff" : root.baseColor
            renderType: Text.NativeRendering
        }
    }

    Image {
        id: pic
        anchors.fill: parent
        visible: false
        source: root.imageUrl
        asynchronous: true
        cache: true
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(Math.ceil(root.size * 2), Math.ceil(root.size * 2))
    }
    Rectangle {
        id: mask
        anchors.fill: parent
        radius: width / 2
        visible: false
        layer.enabled: true
    }
    MultiEffect {
        anchors.fill: parent
        visible: pic.status === Image.Ready
        source: pic
        maskEnabled: true
        maskSource: mask
    }
}
