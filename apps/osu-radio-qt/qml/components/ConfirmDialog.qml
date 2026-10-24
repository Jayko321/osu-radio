pragma ComponentBehavior: Bound
import QtQuick

// Small confirmation on AppModal. Cancel holds the initial focus; Escape and a click outside close
// without confirming.
AppModal {
    id: root
    property string message: ""
    property string actionText: "Confirm"
    signal confirmed()
    panelWidth: 440
    panelHeight: 240
    initialFocus: cancelButton
    Text {
        objectName: "confirmMessage"
        width: parent.width
        text: root.message
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: 14
        wrapMode: Text.Wrap
    }
    footer: Row {
        anchors.right: parent.right
        spacing: 12
        AppButton { id: cancelButton; objectName: "confirmCancel"; text: "Cancel"; onClicked: root.close() }
        AppButton {
            objectName: "confirmAccept"
            text: root.actionText
            variant: "danger"
            onClicked: { root.close(); root.confirmed(); }
        }
    }
}
