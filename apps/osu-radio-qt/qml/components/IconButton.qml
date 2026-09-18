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
        color: control.hovered ? "#14f2f4fc" : "transparent"
    }
    Basic.ToolTip.visible: hovered && accessibleName.length > 0
    Basic.ToolTip.text: accessibleName
    Basic.ToolTip.delay: 700
}
