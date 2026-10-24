import QtQuick

// Shared `enter` transition for menus and small popups; set `transformOrigin` on the popup to the opening corner.
Transition {
    ParallelAnimation {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing }
        NumberAnimation { property: "scale"; from: 0.96; to: 1; duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing }
    }
}
