pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

// One short, non-interactive strip over everything, including modal panels. It sits in the overlay layer
// so the queue panel's dimmer does not cover it; there are no buttons and no Undo.
Basic.Popup {
    id: root
    // The toast is centred horizontally on this item's bottom edge.
    property Item anchorItem: null
    property string text: ""
    property real slide: 0
    property real baseY: 0
    readonly property int duration: 1000
    // A new call replaces the text and restarts the timer.
    function show(message: string): void {
        text = message;
        reposition();
        open();
        hideTimer.restart();
    }
    function reposition(): void {
        const origin = anchorItem ? anchorItem.mapToItem(parent, 0, 0) : Qt.point(0, 0);
        x = Math.round(origin.x + ((anchorItem ? anchorItem.width : parent.width) - width) / 2);
        baseY = Math.round(origin.y + (anchorItem ? anchorItem.height : parent.height) - height - 24);
    }
    parent: Basic.Overlay.overlay
    y: baseY + slide
    z: 1000
    width: label.implicitWidth + 40
    height: 40
    padding: 0
    modal: false
    dim: false
    focus: false
    closePolicy: Basic.Popup.NoAutoClose
    enter: Transition {
        ParallelAnimation {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
            NumberAnimation { property: "slide"; from: 16; to: 0; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
        }
    }
    exit: PopupExit {}
    background: PopupSurface { radius: 8 }
    contentItem: Item {
        Accessible.role: Accessible.AlertMessage
        Accessible.name: root.text
        Text {
            id: label
            objectName: "toastText"
            anchors.centerIn: parent
            text: root.text
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 14
        }
    }
    Timer { id: hideTimer; interval: root.duration; onTriggered: root.close() }
}
