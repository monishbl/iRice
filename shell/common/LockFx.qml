pragma Singleton
import QtQuick
import Quickshell

// The desktop's half of the unlock transition (Settings → Lock Screen →
// Unlock): just before the lock lets go, the desktop is set back (zoomed in,
// widgets hidden), then settles while the lock's own content has lifted away.
// Background.qml reads `desktop` (0 set back .. 1 at rest) and `zoom`.
Singleton {
    id: root

    property real desktop: 1
    property bool zoom: true // zoom (iOS parallax) or only fade

    function prepare(withZoom) {
        settle.stop();
        zoom = withZoom;
        desktop = 0;
    }
    function reveal(ms) {
        settle.duration = ms;
        settle.restart();
    }

    NumberAnimation {
        id: settle
        target: root
        property: "desktop"
        to: 1
        easing.type: Easing.OutCubic
    }
}
