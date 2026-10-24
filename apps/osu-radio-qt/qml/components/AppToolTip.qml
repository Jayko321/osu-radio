import QtQuick
import QtQuick.Effects
import QtQuick.Controls.Basic as Basic

// Declared as a child of the hovered item: `AppToolTip { visible: hovered; text: "..." }`.
Basic.ToolTip {
    id: control
    delay: 700
    topPadding: 4
    bottomPadding: 4
    leftPadding: 8
    rightPadding: 8
    font.family: Theme.fontFamily
    font.pixelSize: 12
    // A hidden owner (closed popup, hidden tab) can keep `hovered` stale, so close with it.
    Connections {
        target: control.parent
        function onVisibleChanged() { if (!control.parent.visible) control.close(); }
    }
    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing }
    }
    exit: Transition {
        NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing }
    }
    contentItem: Text {
        text: control.text
        font: control.font
        color: Theme.text
        wrapMode: Text.Wrap
    }
    background: Rectangle {
        radius: 6
        color: Theme.popup
        border.color: Theme.border
        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowColor: "#000000"
            shadowOpacity: 0.35
            shadowBlur: 0.5
            shadowVerticalOffset: 2
        }
    }
}
