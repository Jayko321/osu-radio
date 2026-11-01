import QtQuick
import QtQuick.Effects

Item {
    id: art
    property url source
    property real radius: 8
    property real imageOpacity: 1
    readonly property int decodeEdge: Math.min(1280, Math.max(1, Math.ceil(width * Screen.devicePixelRatio / 256) * 256))
    // Off for wide card backgrounds, where a centered icon would sit under the text.
    property bool placeholder: true
    // A replacement image keeps the previous one visible, so only the first image after an empty source fades in.
    property real reveal: 0
    // Zero for a layer that is the underside of a cross-fade, so it never dips the blend.
    property int revealDuration: 150
    Rectangle { anchors.fill: parent; radius: art.radius; color: Theme.surface }
    Image {
        id: image
        anchors.fill: parent
        source: art.source
        sourceSize: Qt.size(art.decodeEdge, art.decodeEdge)
        cache: false
        fillMode: Image.PreserveAspectCrop
        horizontalAlignment: Image.AlignHCenter
        verticalAlignment: Image.AlignVCenter
        smooth: true
        visible: false
        onStatusChanged: {
            if (status === Image.Ready) fadeIn.restart();
            else if (status === Image.Null) { fadeIn.stop(); art.reveal = 0; }
        }
    }
    NumberAnimation { id: fadeIn; target: art; property: "reveal"; to: 1; duration: Theme.ms(art.revealDuration) }
    AppIcon {
        anchors.centerIn: parent
        visible: art.placeholder && art.source.toString().length === 0
        name: "music"
        color: Theme.muted
        size: Math.round(Math.min(40, art.width / 2, art.height / 2))
    }
    Rectangle {
        id: mask
        anchors.fill: parent
        radius: art.radius
        color: "white"
        layer.enabled: true
        visible: false
    }
    MultiEffect {
        anchors.fill: parent
        opacity: art.imageOpacity * art.reveal
        source: image
        maskEnabled: true
        maskSource: mask
        maskThresholdMin: 0.5
        maskSpreadAtMin: 1.0
    }
}
