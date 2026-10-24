pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Slider {
    id: progress
    // Presentation seconds: source time divided by speed. The owner never converts them.
    property real playbackPosition: 0
    property real playbackDuration: 0
    // Identifies the launch, selection and speed that give the scale its meaning.
    property string scaleKey: ""
    // A drag that outlives its scale must not seek: the released value belongs to the old one.
    property bool stale: false
    signal seekRequested(real seconds)

    from: 0
    to: Math.max(playbackDuration, 1)
    padding: 0
    Accessible.name: "Playback position"
    Component.onCompleted: value = playbackPosition
    onPlaybackPositionChanged: if (!pressed) value = playbackPosition
    onPlaybackDurationChanged: if (!pressed) value = playbackPosition
    onScaleKeyChanged: {
        if (pressed) stale = true;
        value = playbackPosition;
    }
    onPressedChanged: {
        if (pressed) {
            stale = false;
            return;
        }
        if (enabled && !stale) seekRequested(value);
        stale = false;
        value = playbackPosition;
    }
    onMoved: if (!pressed) seekRequested(value)
    // Pointer fraction with the same handle-travel mapping the slider applies to a click.
    readonly property real hoverFraction: Math.max(0, Math.min(1, (hover.point.position.x - handle.width / 2) / Math.max(width - handle.width, 1)))
    readonly property real tipFraction: pressed ? position : hoverFraction
    function timeLabel(seconds: real): string {
        const whole = Math.max(0, Math.floor(seconds));
        return Math.floor(whole / 60) + ":" + String(whole % 60).padStart(2, "0");
    }
    HoverHandler { id: hover }
    background: Rectangle {
        y: (progress.height - height) / 2
        width: progress.width
        height: progress.hovered || progress.pressed ? 6 : 4
        radius: height / 2
        color: Theme.muted
        scale: progress.mirrored ? -1 : 1
        Behavior on height { NumberAnimation { duration: Theme.ms(Theme.motionQuick); easing.type: Theme.easing } }
        Rectangle {
            width: progress.position * parent.width
            height: parent.height
            radius: parent.radius
            color: Theme.accent
        }
    }
    handle: Rectangle {
        x: progress.visualPosition * (progress.width - width)
        y: (progress.height - height) / 2
        width: 16; height: 16; radius: 8
        color: Theme.accent
    }
    Rectangle {
        id: tip
        objectName: "playbackTip"
        visible: progress.enabled && (hover.hovered || progress.pressed)
        x: Math.max(0, Math.min(progress.width - width, progress.handle.width / 2 + progress.tipFraction * (progress.width - progress.handle.width) - width / 2))
        y: -30
        width: tipText.implicitWidth + 16
        height: tipText.implicitHeight + 4
        radius: 6
        color: Theme.popup
        border.color: Theme.border
        Text {
            id: tipText
            objectName: "playbackTipText"
            anchors.centerIn: parent
            text: progress.timeLabel(progress.tipFraction * progress.playbackDuration)
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
        }
    }
}
