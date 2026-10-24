import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.MenuItem {
    id: control
    required property string iconName
    property bool destructive: false
    readonly property color foreground: destructive ? Theme.red : Theme.text
    implicitHeight: 36
    leftPadding: 16
    rightPadding: 16
    font.family: Theme.fontFamily
    font.pixelSize: 16
    Accessible.name: text
    contentItem: Item {
        opacity: control.enabled ? 1 : 0.4
        Text {
            anchors.left: parent.left
            anchors.right: icon.left
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: control.text
            font: control.font
            color: control.foreground
            elide: Text.ElideRight
        }
        AppIcon {
            id: icon
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            name: control.iconName
            color: control.foreground
            size: 16
        }
    }
    background: Rectangle { radius: 6; color: control.highlighted ? Theme.hover : "transparent" }
}
