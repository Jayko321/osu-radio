import QtQuick
import QtQuick.Controls.Basic as Basic

Item {
    id: root
    property string name: ""
    property color color: Theme.text
    property int size: 24
    implicitWidth: size
    implicitHeight: size

    // Native icon rendering preserves Lucide's SVG strokes and tints their alpha.
    Basic.Button {
        anchors.fill: parent
        enabled: false
        focusPolicy: Qt.NoFocus
        padding: 0
        background: null
        icon.source: root.name ? "qrc:/assets/icons/" + root.name + ".svg" : ""
        icon.color: root.color
        icon.width: root.size
        icon.height: root.size
        display: Basic.AbstractButton.IconOnly
        Accessible.ignored: true
    }
}
