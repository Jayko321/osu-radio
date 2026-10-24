import QtQuick

// Remembers the replaced image source so two stacked layers can cross-fade. A change to or from an
// empty source is not a replacement: it swaps at once, because the empty state is meaningful.
QtObject {
    id: fade
    property url source
    property int duration: Theme.ms(Theme.motionSlow)
    // Source of the layer fading out; empty unless a replacement is in progress.
    property url previous
    // Opacity of the layer showing `source`.
    property real progress: 1
    property url last
    readonly property NumberAnimation animation: NumberAnimation {
        target: fade; property: "progress"; from: 0; to: 1
        duration: fade.duration; easing.type: Theme.easing
        onFinished: fade.previous = ""
    }
    onSourceChanged: {
        const replaced = last.toString().length > 0 && source.toString().length > 0;
        previous = replaced ? last : "";
        last = source;
        if (replaced) animation.restart();
        else { animation.stop(); progress = 1; }
    }
}
