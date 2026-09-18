import QtQuick
import QtQuick.Effects

Item {
    id: root
    property string kind: "regular"
    property Item sourceItem: null
    property real radius: 12
    default property alias content: foreground.data
    // Layout changes must invalidate mapToItem, which has no notify signal itself.
    readonly property point sourceOrigin: {
        const geometry = Qt.rect(x, y, width, height);
        return sourceItem ? root.mapToItem(sourceItem, geometry.x - x, geometry.y - y) : Qt.point(0, 0);
    }

    data: [ShaderEffectSource {
        id: capture
        parent: root
        sourceItem: root.sourceItem
        sourceRect: Qt.rect(root.sourceOrigin.x, root.sourceOrigin.y, root.width, root.height)
        textureSize: Qt.size(Math.max(1, root.width), Math.max(1, root.height))
        visible: false
        live: true
    }, Item {
        parent: root
        anchors.fill: parent
        clip: true
        MultiEffect {
            anchors.fill: parent
            visible: root.sourceItem !== null
            source: capture
            blurEnabled: true
            blurMax: root.kind === "regular" ? 50 : 60
            blur: 1
            autoPaddingEnabled: false
            maskEnabled: true
            maskSource: roundedMask
        }
    }, Rectangle {
        id: roundedMask
        parent: root
        width: root.width
        height: root.height
        radius: root.radius
        color: "white"
        layer.enabled: true
        visible: false
    }, Rectangle {
        parent: root
        anchors.fill: parent
        radius: root.radius
        color: root.kind === "thick" ? "#f20d0d0d" : root.kind === "thin" ? "#80000000" : "#cc121212"
        border.color: Theme.border
        border.width: 1
    }, Item {
        id: foreground
        parent: root
        anchors.fill: parent
    }]
}
