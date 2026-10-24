import QtQuick
import QtQuick.Controls.Basic as Basic

// Thin overlay handle without a track; attach with `Basic.ScrollBar.vertical: AppScrollBar {}`.
Basic.ScrollBar {
    id: control
    policy: Basic.ScrollBar.AsNeeded
    minimumSize: 0.1
    padding: 2
    contentItem: Rectangle {
        implicitWidth: control.vertical ? (control.hovered || control.pressed ? 8 : 6) : 40
        implicitHeight: control.vertical ? 40 : (control.hovered || control.pressed ? 8 : 6)
        radius: 4
        color: Qt.alpha(Theme.border, control.pressed ? 0.5 : control.hovered ? 0.4 : 0.3)
        opacity: control.size < 1 && (control.active || control.policy === Basic.ScrollBar.AlwaysOn) ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
        Behavior on implicitWidth { NumberAnimation { duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing } }
        Behavior on implicitHeight { NumberAnimation { duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing } }
    }
    background: null
}
