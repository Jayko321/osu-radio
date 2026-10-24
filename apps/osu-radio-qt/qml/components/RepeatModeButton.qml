import QtQuick

// Shared by the player panel and the offline gallery. `mode` is "off", "all" or "one".
IconButton {
    property string mode: "off"
    iconName: mode === "one" ? "repeat-1" : "repeat-2"
    foreground: mode === "off" ? Theme.text : Theme.accent
    accessibleName: "Repeat: " + mode
}
