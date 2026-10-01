import QtQuick
import QtQuick.Effects

Item {
    id: art
    property url source
    property real radius: 8
    property real imageOpacity: 1
    Rectangle { anchors.fill: parent; radius: art.radius; color: Theme.surface }
    Image {
        id: image
        anchors.fill: parent
        source: art.source
        cache: false
        fillMode: Image.PreserveAspectCrop
        horizontalAlignment: Image.AlignHCenter
        verticalAlignment: Image.AlignVCenter
        smooth: true
        visible: false
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
        opacity: art.imageOpacity
        source: image
        maskEnabled: true
        maskSource: mask
        maskThresholdMin: 0.5
        maskSpreadAtMin: 1.0
    }
}
