import QtQuick

// Lucide icon from qml/assets/icons/<name>.svg, recoloured. size is in px.
Image {
    id: root
    property string name
    property color color: Theme.foreground
    property real size: 16
    width: size
    height: size
    sourceSize: Qt.size(size * Screen.devicePixelRatio, size * Screen.devicePixelRatio)
    source: name ? "image://icons/" + name + "?c=" + encodeURIComponent(color) : ""
    fillMode: Image.PreserveAspectFit
    asynchronous: false
    cache: true
}
