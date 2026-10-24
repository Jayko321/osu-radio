pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts
import QtQuick.Effects

Basic.Popup {
    id: root
    parent: Basic.Overlay.overlay
    property Item scene: null
    property string title: ""
    property bool dismissible: true
    property Item initialFocus: closeButton
    property Item previousFocus: null
    property real panelWidth: 740
    property real panelHeight: 620
    default property alias body: bodyContainer.data
    property alias footer: footerContainer.data
    // Vertical offset animated by the enter transition; anchors.centerIn would fight an animated y.
    property real slide: 0
    width: Math.min(panelWidth, parent ? parent.width - 48 : panelWidth)
    height: Math.min(panelHeight, parent ? parent.height - 48 : panelHeight)
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.round((parent.height - height) / 2 + slide) : 0
    enter: Transition {
        ParallelAnimation {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
            NumberAnimation { property: "slide"; from: 8; to: 0; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
        }
    }
    exit: PopupExit {}
    padding: 28
    modal: true
    dim: true
    focus: true
    closePolicy: dismissible ? Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside : Basic.Popup.NoAutoClose
    onAboutToShow: previousFocus = scene && scene.Window.window ? scene.Window.window.activeFocusItem : null
    onOpened: if (initialFocus) initialFocus.forceActiveFocus(Qt.PopupFocusReason)
    onClosed: if (previousFocus) previousFocus.forceActiveFocus(Qt.PopupFocusReason)
    background: Rectangle { color: Theme.modal; radius: 12; border.color: Theme.border; border.width: 1 }
    Basic.Overlay.modal: Item {
        // Qt owns the overlay item's own opacity, so the backdrop fades through this child, following the panel.
        Item {
            objectName: "modalDimmerFade"
            anchors.fill: parent
            opacity: root.opacity
            ShaderEffectSource {
                id: capture
                sourceItem: root.visible ? root.scene : null
                visible: false
                live: root.visible
            }
            MultiEffect {
                anchors.fill: parent
                source: root.visible ? capture : null
                visible: root.visible && root.scene !== null
                blurEnabled: root.visible
                blurMax: 40
                blur: 1
                autoPaddingEnabled: false
            }
            Rectangle { anchors.fill: parent; color: "#66000000" }
        }
    }
    contentItem: ColumnLayout {
        spacing: 24
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: root.title
                Layout.fillWidth: true
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 24
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
            }
            IconButton {
                id: closeButton
                objectName: "modalClose"
                iconName: "x"
                accessibleName: "Close dialog"
                enabled: root.dismissible
                onClicked: root.close()
            }
        }
        Item { id: bodyContainer; Layout.fillWidth: true; Layout.fillHeight: true }
        Item { id: footerContainer; Layout.fillWidth: true; implicitHeight: childrenRect.height }
    }
}
