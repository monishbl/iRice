import QtQuick

// Caps how long a closing panel stays on screen. Exit animations run on the
// render loop; if frames stall, `progress > 0` alone could leave an invisible
// full-screen layer catching every click. Use: visible: progress > 0 && guard.open
Timer {
    property bool shown
    readonly property bool open: shown || running
    interval: 700
    onShownChanged: shown ? stop() : restart()
}
