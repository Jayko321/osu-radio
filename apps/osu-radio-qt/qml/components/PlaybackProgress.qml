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
    background: Rectangle {
        y: 6
        width: progress.width
        height: 4
        radius: 2
        color: Theme.muted
        scale: progress.mirrored ? -1 : 1
        Rectangle {
            width: progress.position * parent.width
            radius: parent.radius
            color: Theme.accent
        }
    }
    handle: Rectangle { x: progress.visualPosition * (progress.width - width); width: 16; height: 16; radius: 8; color: Theme.accent }
}
