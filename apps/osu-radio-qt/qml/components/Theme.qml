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
    readonly property string fontFamily: regularFont.name || "Poppins"
}
