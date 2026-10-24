import QtQuick

// Shared by the player panel and the offline gallery.
IconButton {
    property bool active: false
    iconName: "shuffle"
    foreground: active ? Theme.accent : Theme.text
    accessibleName: "Shuffle: " + (active ? "on" : "off")
}
