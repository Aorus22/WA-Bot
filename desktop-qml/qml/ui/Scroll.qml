import QtQuick
import QtQuick.Controls

// ScrollView/Flickable with a slim themed scrollbar. Put content in as children.
Flickable {
    id: root
    default property alias content: inner.data
    contentWidth: width
    contentHeight: inner.childrenRect.height
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ThinScrollBar {}
    Item { id: inner; width: root.width; height: childrenRect.height }
}
