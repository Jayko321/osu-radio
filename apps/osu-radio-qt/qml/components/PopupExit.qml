import QtQuick

// Shared `exit` transition for menus and small popups.
Transition {
    NumberAnimation { property: "opacity"; to: 0; duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing }
}
