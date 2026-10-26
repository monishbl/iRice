import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass

// Shader tuning bench: glass shapes over the busiest parts of the wallpaper.
// Loaded only when IRICE_LAB=1.
GlassWindow {
    name: "lab"
    screen: Quickshell.screens.find(s => s.name === "IRICE-TEST") ?? Quickshell.screens[0]
    anchors { top: true; bottom: true; left: true; right: true }
    WlrLayershell.layer: WlrLayer.Top
    mask: Region {}

    Glass {
        x: 760 - shadowMargin; y: 330 - shadowMargin
        width: 360 + 2 * shadowMargin; height: 220 + 2 * shadowMargin
        radius: Theme.radius.panel
    }
    Glass {
        x: 1240 - shadowMargin; y: 420 - shadowMargin
        width: 120 + 2 * shadowMargin; height: 120 + 2 * shadowMargin
        radius: 60
        bevel: 40
    }
    Glass {
        x: 1180 - shadowMargin; y: 640 - shadowMargin
        width: 300 + 2 * shadowMargin; height: 52 + 2 * shadowMargin
        radius: 26
    }
}
