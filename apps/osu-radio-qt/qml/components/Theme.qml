pragma Singleton
import QtQuick

QtObject {
    // Loading the singleton registers every bundled face for both application roots.
    readonly property FontLoader regularFont: FontLoader { source: "qrc:/assets/fonts/Poppins-Regular.ttf" }
    readonly property FontLoader mediumFont: FontLoader { source: "qrc:/assets/fonts/Poppins-Medium.ttf" }
    readonly property FontLoader semiboldFont: FontLoader { source: "qrc:/assets/fonts/Poppins-SemiBold.ttf" }
    readonly property FontLoader boldFont: FontLoader { source: "qrc:/assets/fonts/Poppins-Bold.ttf" }
    readonly property FontLoader fallbackFont: FontLoader { source: "qrc:/assets/fonts/Nunito-Variable.ttf" }
    readonly property color background: "#0e0e0e"
    readonly property color text: "#eff1f5"
    readonly property color muted: "#767982"
    readonly property color accent: "#9366ed"
    readonly property color surface: "#3384898f"
    readonly property color border: "#1af2f4fc"
    readonly property color red: "#ed6666"
    readonly property color green: "#78cc32"
    readonly property color hover: "#14f2f4fc"
    readonly property color popup: "#f20d0d0d"
    readonly property color modal: "#f218181b"
    readonly property color accentSoft: "#409366ed"
    readonly property int motionQuick: 80
    readonly property int motionFast: 120
    readonly property int motionBase: 160
    readonly property int motionPanel: 200
    readonly property int motionSlow: 250
    readonly property int easing: Easing.OutCubic
    // Internal switch with no UI setting; durations pass through ms() so it can disable all motion.
    // Writable only so probes can turn motion off; application code never assigns it.
    property bool motion: true
    readonly property string fontFamily: regularFont.name || "Poppins"

    function ms(value: int): int { return motion ? value : 0 }
}
