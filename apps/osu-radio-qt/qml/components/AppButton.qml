import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Button {
    id: control
    property string variant: "alternate"
    property string iconName: ""
    property color foreground: variant === "light" ? Theme.background
        : variant === "link" && enabled ? Theme.accent : Theme.text
    implicitHeight: 40
    implicitWidth: Math.max(40, implicitContentWidth + leftPadding + rightPadding)
    leftPadding: 16
    rightPadding: 16
    topPadding: 0
    bottomPadding: 0
    spacing: 8
    hoverEnabled: true
    font.family: Theme.fontFamily
    font.pixelSize: 14
    font.weight: Font.Medium
    icon.source: iconName ? "qrc:/assets/icons/" + iconName + ".svg" : ""
    icon.color: foreground
    icon.width: 24
    icon.height: 24
    palette.buttonText: foreground
    palette.disabled.buttonText: foreground
    opacity: !enabled ? 0.4 : down ? 0.6 : hovered ? 0.8 : 1
    background: Rectangle {
        radius: 8
        color: control.variant === "light" ? Theme.text
             : control.variant === "accent" ? Theme.accent
             : control.variant === "link" ? "transparent" : Theme.surface
        border.width: control.variant === "link" ? 0 : 1
        border.color: control.variant === "accent" ? Theme.accent : Theme.border
    }
}
