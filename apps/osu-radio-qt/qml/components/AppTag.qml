import QtQuick

AppButton {
    id: control
    property bool selected: false
    property int filterState: -1
    readonly property bool included: filterState === 1 || (filterState === -1 && selected)
    implicitHeight: 32
    foreground: enabled && filterState === 2 ? Theme.red : Theme.text
    Accessible.description: filterState < 0 ? (selected ? "Selected" : "Unselected")
        : filterState === 0 ? "Neutral" : filterState === 1 ? "Included" : "Excluded"
    background: Rectangle {
        radius: 16
        color: control.included ? Theme.accent : "transparent"
        border.width: 1
        border.color: control.filterState === 2 ? Theme.red : control.included ? Theme.accent : Theme.border
    }
}
