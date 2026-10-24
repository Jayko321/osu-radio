import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Menu {
    id: control
    property Item previousFocus: null
    width: 252
    padding: 6
    transformOrigin: Basic.Popup.TopLeft
    enter: PopupEnter {}
    exit: PopupExit {}
    onAboutToShow: previousFocus = parent && parent.Window.window ? parent.Window.window.activeFocusItem : parent
    onOpened: {
        for (let i = 0; i < count; ++i) {
            if (itemAt(i).visible && itemAt(i).enabled) {
                currentIndex = i;
                itemAt(i).forceActiveFocus();
                break;
            }
        }
    }
    onClosed: if (previousFocus) previousFocus.forceActiveFocus()
    background: PopupSurface {}
}
