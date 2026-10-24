import QtQuick
import QtQuick.Controls.Basic as Basic

AppButton {
    id: control
    property string accessibleName: ""
    implicitWidth: 40
    implicitHeight: 40
    leftPadding: 0
    rightPadding: 0
    display: Basic.AbstractButton.IconOnly
    Accessible.name: accessibleName
    background: Rectangle {
        radius: 8
        color: control.hovered ? Theme.hover : Qt.alpha(Theme.hover, 0)
        Behavior on color { ColorAnimation { duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing } }
    }
    AppToolTip {
        objectName: "buttonToolTip"
        visible: control.hovered && control.accessibleName.length > 0
        text: control.accessibleName
    }
}
