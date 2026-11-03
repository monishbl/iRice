import QtQuick
import Quickshell

// The dock, in each look's own drawing, each made only in its look: Glass's
// (DockGlass: macOS's) and Material's (MaterialDock: a Pixel tablet's
// taskbar). The same items and behaviour (DockLogic).
// Settings → Dock → Show the dock off: no dock windows at all.
Scope {
    DockGlass {}
    MaterialDock {}
}
