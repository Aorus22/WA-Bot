import QtQuick
import QtQuick.Layouts
import "../../ui"
import "sidebar"
import "conversation"
import "viewer"
import "dialogs"

// Auto is side-by-side in a normal window and resolves to pages in popover mode, like GPUI.
Rectangle {
    id: root
    objectName: "chatPage"
    color: "transparent"
    clip: true
    readonly property bool pagesLayout: System.chatLayout === "pages"
        || (System.chatLayout === "auto" && Platform.popoverMode)
    readonly property bool sidebarIsHidden: !root.pagesLayout && root.sidebarCollapsed
    readonly property bool starredFullPage: root.activeSheet === "starred"
        && root.pagesLayout && AppState.activeChatId === ""
    readonly property bool infoFullPage: root.activeSheet === "info"
        && root.pagesLayout && AppState.activeChatId !== ""
    readonly property real rightDrawerWidth: root.starredFullPage || root.infoFullPage ? 0
        : ((root.activeSheet === "info" && AppState.activeChatId !== "")
           || root.activeSheet === "search" || root.activeSheet === "starred" || Messages.infoOpen)
            ? Math.min(320, Math.max(0, root.width - 20)) : 0
    property real animatedDrawerWidth: 0
    property string activeSheet: ""
    property string pendingJumpChat: ""
    property string pendingJumpMessage: ""
    property bool sidebarCollapsed: false
    property real sidebarWidth: 360
    property real resizeStartX: 0
    property real resizeStartWidth: 360

    onRightDrawerWidthChanged: animatedDrawerWidth = rightDrawerWidth
    Component.onCompleted: animatedDrawerWidth = rightDrawerWidth
    Behavior on animatedDrawerWidth {
        NumberAnimation { duration: 240; easing.type: Easing.OutQuint }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0
        ChatSidebar {
            Layout.preferredWidth: root.sidebarIsHidden ? 0
                : root.pagesLayout && AppState.activeChatId === "" ? root.width : root.sidebarWidth
            Layout.minimumWidth: root.sidebarIsHidden ? 0 : 280
            Layout.fillWidth: !root.sidebarIsHidden && root.pagesLayout && AppState.activeChatId === ""
            Layout.fillHeight: true
            visible: !root.sidebarIsHidden && (!root.pagesLayout || AppState.activeChatId === "")
        }
        ConversationPane {
            id: conversationPane
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !root.pagesLayout || AppState.activeChatId !== ""
            sidebarCollapsed: root.sidebarCollapsed
            onToggleSidebarRequested: root.sidebarCollapsed = !root.sidebarCollapsed
        }
        Item {
            objectName: "chatRightDrawerSpace"
            Layout.preferredWidth: root.animatedDrawerWidth
            Layout.minimumWidth: 0
            Layout.maximumWidth: 320
            Layout.fillHeight: true
        }
    }

    MouseArea {
        id: sidebarResizeHandle
        objectName: "chatSidebarResizeHandle"
        x: root.sidebarWidth - width / 2
        y: 0
        width: 6
        height: root.height
        z: 2
        visible: !root.pagesLayout && !root.sidebarCollapsed
        hoverEnabled: true
        preventStealing: true
        cursorShape: Qt.SizeHorCursor
        Accessible.name: "Resize chat list"
        Accessible.description: "Drag to change the width of the chat list"
        onPressed: function(mouse) {
            root.resizeStartX = mapToItem(root, mouse.x, mouse.y).x
            root.resizeStartWidth = root.sidebarWidth
        }
        onPositionChanged: function(mouse) {
            if (!pressed) return
            const currentX = mapToItem(root, mouse.x, mouse.y).x
            root.sidebarWidth = Math.max(280, Math.min(600, root.resizeStartWidth + currentX - root.resizeStartX))
        }

        Rectangle {
            anchors.fill: parent
            color: parent.containsMouse || parent.pressed ? Theme.alpha(Theme.primary, 0.18) : "transparent"
        }
    }

    MediaViewer { anchors.fill: parent }
    MessageInfoSheet { anchors.fill: parent }
    ForwardDialog { id: forwardDialog }
    ChatInfoSheet {
        id: chatInfoSheet
        anchors.fill: parent
        open: root.activeSheet === "info"
        fullWidth: root.infoFullPage
        onClosed: root.activeSheet = ""
    }
    ChatSearchSheet {
        anchors.fill: parent
        open: root.activeSheet === "search"
        chatId: AppState.activeChatId
        onClosed: root.activeSheet = ""
    }
    StarredSheet {
        anchors.fill: parent
        open: root.activeSheet === "starred"
        fullWidth: root.starredFullPage
        onClosed: root.activeSheet = ""
    }

    Connections {
        target: Chats
        function onForwardRequested(chatId, messageIds) {
            const ids = String(messageIds || "").split(",").filter(id => id !== "")
            forwardDialog.openFor(chatId, ids)
        }
        function onForwardManyRequested(chatId, messageIds) {
            forwardDialog.openFor(chatId, messageIds)
        }
        function onSheetRequested(name, chatId) {
            if (name !== "" && Messages.infoOpen) Messages.closeInfo()
            if (root.activeSheet === name) {
                root.activeSheet = ""
                return
            }
            root.activeSheet = name
            if (name === "info") Chats.loadInfo(chatId)
            else if (name === "starred") Chats.loadStarred(false)
        }
        function onJumpRequested(chatId, messageId) {
            root.pendingJumpChat = chatId
            root.pendingJumpMessage = messageId
            if (AppState.activeChatId !== chatId) AppState.openChat(chatId)
            jumpTimer.start()
        }
        function onAddMembersRequested(groupId) {
            if (String(groupId) === AppState.activeChatId)
                chatInfoSheet.openAddMembers()
        }
    }

    Connections {
        target: AppState
        function onActiveChatChanged() {
            root.activeSheet = ""
            Chats.clearSearch()
        }
    }

    Connections {
        target: Messages
        function onInfoChanged() {
            if (Messages.infoOpen)
                root.activeSheet = ""
        }
    }

    Timer {
        id: jumpTimer
        interval: 80
        repeat: true
        onTriggered: {
            if (Messages.chatId !== root.pendingJumpChat || Messages.loading) return
            const id = root.pendingJumpMessage
            root.pendingJumpChat = ""
            root.pendingJumpMessage = ""
            stop()
            Messages.jumpTo(id)
        }
    }
}
