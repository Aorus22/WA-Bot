import QtQuick
import "../../ui"

// AppText with GPUI's default line height (gpui TextStyle default = phi x font size) so block heights match.
AppText {
    lineHeightMode: Text.FixedHeight
    lineHeight: Math.round(font.pixelSize * 1.618)
}
