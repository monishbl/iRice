import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// Bottom rounded screen corners: one thin strip window per screen (the top
// corners are drawn by the menu bar). Drawn on top, never takes input.
Variants {
    model: Config.options.appearance.screenCorners ? Quickshell.screens : []

    PanelWindow {
        id: win
        required property var modelData
        readonly property int r: Config.options.appearance.screenCornerRadius

        screen: modelData
        WlrLayershell.namespace: "irice:corners"
        WlrLayershell.layer: WlrLayer.Overlay
        exclusionMode: ExclusionMode.Ignore
        anchors { bottom: true; left: true; right: true }
        implicitHeight: r
        color: "transparent"
        mask: Region {}

        Corner { x: 0; atBottom: true; radius: win.r }
        Corner { x: win.width - win.r; atBottom: true; atRight: true; radius: win.r }
    }
}
