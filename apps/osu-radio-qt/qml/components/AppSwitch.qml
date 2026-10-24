import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Switch {
    id: control
    implicitHeight: 32
    implicitWidth: implicitContentWidth + 49
    padding: 0
    spacing: 12
    hoverEnabled: true
    font.family: Theme.fontFamily
    font.pixelSize: 14
    opacity: !enabled ? 0.4 : down ? 0.6 : hovered ? 0.8 : 1
    indicator: Rectangle {
        x: 0
        y: (control.height - height) / 2
        width: 37
        height: 20
        radius: 10
        color: control.checked ? Theme.accent : Theme.surface
        Behavior on color { ColorAnimation { duration: Theme.ms(120); easing.type: Theme.easing } }
        Rectangle {
            x: control.checked ? 19 : 2
            Behavior on x { NumberAnimation { duration: Theme.ms(140); easing.type: Theme.easing } }
            y: 2
            width: 16
            height: 16
            radius: 8
            color: Theme.text
        }
    }
    contentItem: Text {
        leftPadding: control.text ? 49 : 0
        text: control.text
        font: control.font
        color: Theme.text
        verticalAlignment: Text.AlignVCenter
    }
}
