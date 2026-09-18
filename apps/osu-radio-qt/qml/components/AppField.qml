import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.TextField {
    id: control
    implicitWidth: 240
    implicitHeight: 40
    leftPadding: 12
    rightPadding: 12
    topPadding: 6
    bottomPadding: 6
    font.family: Theme.fontFamily
    font.pixelSize: 16
    color: Theme.text
    placeholderTextColor: enabled ? Theme.muted : Theme.text
    selectionColor: "#40eff1f5"
    selectedTextColor: Theme.text
    selectByMouse: true
    opacity: enabled ? 1 : 0.4
    background: Rectangle {
        radius: 8
        color: Theme.surface
        border.color: control.hovered ? Theme.muted : Theme.border
        border.width: 1
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            radius: 11
            color: "transparent"
            border.color: Theme.text
            border.width: 2
            visible: control.enabled && control.activeFocus
        }
    }
}
