import QtQuick
import QtQuick.Effects

// Shared background for every popup except AppModal: translucent surface, hairline border, soft shadow.
Rectangle {
    color: Theme.popup
    border.color: Theme.border
    radius: 12
    layer.enabled: true
    layer.effect: MultiEffect {
        shadowEnabled: true
        shadowColor: "#000000"
        shadowOpacity: 0.5
        shadowBlur: 0.8
        shadowVerticalOffset: 6
    }
}
