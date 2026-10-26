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

    readonly property bool wanted: Panels.isOpen(panel, screen)
    property bool alive: false
    active: alive

    onWantedChanged: {
        if (wanted) {
            unload.stop();
            alive = true;
        } else if (!keepLoaded) {
            unload.restart();
        }
    }

    property Timer unload: Timer {
        interval: root.linger
        onTriggered: root.alive = false
    }
}
