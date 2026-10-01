pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Popup {
    id: root
    required property Item host
    property var tracks: []
    property bool loading: false
    property string message: ""
    property Item previousFocus: null
    signal mediaRequested(int audioId)
    signal mediaDelegateAdded(var row)
    signal mediaDelegateRemoved(var row)
    signal visibleMediaChanged()
    signal retryRequested()
    parent: host
    x: host.width - width
    width: Math.min(430, host.width)
    height: host.height
    padding: 16
    modal: true
    dim: true
    focus: true
    closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
    onAboutToShow: previousFocus = host.Window.window.activeFocusItem
    onOpened: closeButton.forceActiveFocus(Qt.PopupFocusReason)
    onClosed: if (previousFocus) previousFocus.forceActiveFocus(Qt.PopupFocusReason)
    background: null
    Basic.Overlay.modal: Item {
        Rectangle {
            readonly property point origin: root.host.mapToItem(parent, 0, 0)
            x: origin.x
            y: origin.y
            width: root.host.width
            height: root.height
            gradient: Gradient {
                GradientStop { position: 0; color: "#f5000000" }
                GradientStop { position: 1; color: "#cc0e0e0e" }
            }
        }
    }
    contentItem: Item {
        Item {
            id: header
            width: parent.width
            height: 24
            Text {
                anchors.left: parent.left
                anchors.right: closeButton.left
                anchors.verticalCenter: parent.verticalCenter
                text: "Next songs on the queue"
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 14
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
            IconButton {
                id: closeButton
                objectName: "queueCloseButton"
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 32
                height: 32
                iconName: "x"
                icon.width: 18
                icon.height: 18
                foreground: "#b4b4b4"
                accessibleName: "Close queue"
                onClicked: root.close()
            }
        }
        Basic.ScrollView {
            id: scroll
            objectName: "queueScroll"
            anchors.top: header.bottom
            anchors.topMargin: 14
            anchors.bottom: parent.bottom
            width: parent.width
            clip: true
            Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
            Flickable {
                id: flickable
                contentWidth: width
                contentHeight: cards.height
                boundsBehavior: Flickable.StopAtBounds
                Column {
                    id: cards
                    width: flickable.width
                    spacing: 14
                    // ponytail: all pending cards stay instantiated; use ListView for very large queues.
                    Repeater {
                        model: root.tracks.length
                        Item {
                            id: card
                            required property int index
                            readonly property var modelData: root.tracks[index] || ({
                                audioId: -1, title: "", artist: "", durationLabel: "--:--", artworkUrl: ""
                            })
                            objectName: "queueTrackCard"
                            width: cards.width
                            height: 82
                            readonly property bool onScreen: y + height >= flickable.contentY && y < flickable.contentY + flickable.height
                            readonly property bool inViewport: root.visible && onScreen
                            readonly property int audioId: modelData.audioId
                            readonly property bool available: audioId >= 0
                            onInViewportChanged: root.visibleMediaChanged()
                            onAudioIdChanged: root.visibleMediaChanged()
                            Component.onCompleted: root.mediaDelegateAdded(card)
                            Component.onDestruction: root.mediaDelegateRemoved(card)
                            Accessible.role: Accessible.StaticText
                            Accessible.name: modelData.title + ", " + modelData.artist + ", " + modelData.durationLabel
                            Artwork { anchors.fill: parent; source: card.modelData.artworkUrl }
                            Rectangle {
                                anchors.fill: parent
                                radius: 8
                                gradient: Gradient {
                                    orientation: Gradient.Horizontal
                                    GradientStop { position: 0; color: "#d9000000" }
                                    GradientStop { position: 1; color: "#18000000" }
                                }
                            }
                            Column {
                                anchors.verticalCenter: parent.verticalCenter
                                x: 18
                                width: parent.width - 36
                                spacing: 2
                                Text {
                                    width: parent.width
                                    text: card.modelData.title
                                    color: Theme.text
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 20
                                    font.weight: Font.Bold
                                    elide: Text.ElideRight
                                }
                                Text {
                                    width: parent.width
                                    text: card.modelData.artist + " // " + card.modelData.durationLabel
                                    color: Theme.text
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                    Text {
                        objectName: "queueStatus"
                        width: parent.width
                        visible: root.message.length > 0 || root.tracks.length === 0
                        text: root.message || (root.loading ? "Loading queue…" : "No upcoming songs")
                        color: Theme.muted
                        font.family: Theme.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                    AppButton {
                        objectName: "queueRetryButton"
                        visible: root.message.length > 0
                        enabled: !root.loading
                        text: "Retry"
                        onClicked: root.retryRequested()
                    }
                }
            }
        }
    }
}
