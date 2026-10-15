import QtQuick

// Themed text. `size` is in rem-fractions like Tailwind (0.875 = text-sm). Colour defaults to foreground.
Text {
    property real size: 0.875
    property bool semibold: false
    property bool medium: false
    color: Theme.foreground
    font.pixelSize: Math.round(Theme.rem * size)
    font.weight: semibold ? Font.DemiBold : (medium ? Font.Medium : Font.Normal)
    elide: Text.ElideNone
    renderType: Text.NativeRendering
}
