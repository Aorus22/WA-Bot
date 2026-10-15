import QtQuick

// Turning loader icon (one revolution per second).
Icon {
    name: "loader-circle"
    RotationAnimator on rotation { from: 0; to: 360; duration: 1000; loops: Animation.Infinite; running: visible }
}
