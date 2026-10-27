import QtQuick
import Quickshell

// Creates a panel's window only while it's open (plus a moment for the close
// animation), so closed panels cost no memory or render threads. Put the
// window inline as the child. `keepLoaded` keeps it after the first open.
LazyLoader {
    id: root

    required property string panel
    property var screen: null
    property bool keepLoaded: false
    property int linger: 600 // ms after closing, > exit animations
    // Settings → Appearance → Keep panels loaded: the most-used panels too
    readonly property var everyday: ["spotlight", "controlCenter", "notificationCenter"]
    readonly property bool keep: keepLoaded || (Config.ready && Config.options.performance.keepPanelsLoaded && everyday.includes(panel))

    readonly property bool wanted: Panels.isOpen(panel, screen)
    property bool alive: false
    active: alive

    onWantedChanged: {
        if (wanted) {
            unload.stop();
            alive = true;
        } else if (!keep) {
            unload.restart();
        }
    }
    // Switched off while one is loaded and closed: let it go
    onKeepChanged: if (!keep && !wanted && alive) unload.restart()

    property Timer unload: Timer {
        interval: root.linger
        onTriggered: root.alive = false
    }
}
