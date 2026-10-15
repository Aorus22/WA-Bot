import QtQuick

// Edge/corner grips for a frameless window (hidden while maximized).
Item {
    id: root
    required property var win
    anchors.fill: parent
    visible: win.visibility === Window.Windowed
    z: 2000
    readonly property int m: 5
    component Grip: MouseArea {
        property int edges
        hoverEnabled: true
        onPressed: root.win.startSystemResize(edges)
    }
    Grip { edges: Qt.LeftEdge; x: 0; y: root.m; width: root.m; height: root.height - 2 * root.m; cursorShape: Qt.SizeHorCursor }
    Grip { edges: Qt.RightEdge; x: root.width - root.m; y: root.m; width: root.m; height: root.height - 2 * root.m; cursorShape: Qt.SizeHorCursor }
    Grip { edges: Qt.TopEdge; x: root.m; y: 0; width: root.width - 2 * root.m; height: root.m; cursorShape: Qt.SizeVerCursor }
    Grip { edges: Qt.BottomEdge; x: root.m; y: root.height - root.m; width: root.width - 2 * root.m; height: root.m; cursorShape: Qt.SizeVerCursor }
    Grip { edges: Qt.LeftEdge | Qt.TopEdge; x: 0; y: 0; width: root.m; height: root.m; cursorShape: Qt.SizeFDiagCursor }
    Grip { edges: Qt.RightEdge | Qt.BottomEdge; x: root.width - root.m; y: root.height - root.m; width: root.m; height: root.m; cursorShape: Qt.SizeFDiagCursor }
    Grip { edges: Qt.RightEdge | Qt.TopEdge; x: root.width - root.m; y: 0; width: root.m; height: root.m; cursorShape: Qt.SizeBDiagCursor }
    Grip { edges: Qt.LeftEdge | Qt.BottomEdge; x: 0; y: root.height - root.m; width: root.m; height: root.m; cursorShape: Qt.SizeBDiagCursor }
}
