import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Notifications
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Glass banners in the top-right corner of the focused screen. Several at once
// collapse into a stack (newest in front, two peeking behind); the "more" pill
// fans them out into a list and back. Only a new banner slides in; the others
// glide to make room. Banners expire after a few seconds (paused while the
// pointer is on them) and stay listed in Notification Center.
Scope {
    id: root

    // The window exists only while banners are showing (plus a moment)
    property bool windowAlive: false
    readonly property bool windowWanted: Notifications.popupModel.count > 0
    onWindowWantedChanged: windowWanted ? (windowAlive = true, windowUnload.stop()) : windowUnload.restart()
    Timer { id: windowUnload; interval: 800; onTriggered: root.windowAlive = false }

    // Material: Android's heads-up notifications (MaterialPopupWindow.qml)
    LazyLoader {
        active: root.windowAlive && Theme.material

        MaterialPopupWindow {}
    }

}
