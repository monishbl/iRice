import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Glass dock, macOS layout: Apps, pinned and running apps | recent apps |
// minimized windows, Downloads, Trash. Uniform icons with a hover lift;
// magnification is optional. The window has a fixed size (no per-frame
// resizing, which made the old dock flicker); input is masked to what's visible.
// Auto-hide: off, always, or only when a window overlaps the dock.
// Settings → Dock → Show the dock off: no dock windows at all.
// The Material look has its own dock (MaterialDock: a Pixel tablet's taskbar)
// with the same items and behaviour.
Scope {
    MaterialDock {}
}
