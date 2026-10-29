import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// While a window is dragged to a screen edge or corner, a glass outline of
// where it will land (like macOS Sequoia). Takes no input; the drag goes on
// underneath.
Variants {
    model: Quickshell.screens

    GlassWindow {
        id: win
        required property var modelData
        screen: modelData
        name: "snappreview"

        // (modelData, not `screen`: the window re-announces its screen when
        // it shows or hides, and visible depends on this: a binding loop)
        readonly property bool active: Tiling.dragZone !== "none" && Tiling.dragMonitor === modelData.name
        // The target, in this screen's coordinates (kept while fading out)
        property rect target: Qt.rect(0, 0, 0, 0)
        onActiveChanged: if (active) place()
        Connections {
            target: Tiling
            function onDragZoneChanged() { if (win.active) win.place(); }
        }
        function place() {
            const a = Tiling.areaOf(modelData.name);
            const r = Tiling.rect(Tiling.dragZone, a);
            const mx = Hyprland.monitors.values.find(m => m.name === modelData.name)?.x ?? 0;
            const my = Hyprland.monitors.values.find(m => m.name === modelData.name)?.y ?? 0;
            target = Qt.rect(r.x - mx, r.y - my, r.w, r.h);
        }

        visible: active || shape.opacity > 0.01
        anchors { top: true; bottom: true; left: true; right: true }
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        mask: Region {}

        Item {
            id: shape
            x: win.target.x
            y: win.target.y
            width: win.target.width
            height: win.target.height
            opacity: win.active ? 1 : 0
            scale: win.active ? 1 : 0.96
            Behavior on x { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
            Behavior on y { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
            Behavior on width { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
            Behavior on height { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
            Behavior on opacity { NumberAnimation { duration: win.active ? 150 : 100 } }
            Behavior on scale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

            Glass {
                anchors.fill: parent
                anchors.margins: -shadowMargin
                shadowMargin: 16
                radius: Config.options.appearance.windowRadius
            }
            Rectangle {
                anchors.fill: parent
                radius: Config.options.appearance.windowRadius
                color: Theme.accentAlpha(0.12)
                border.width: 1
                border.color: Qt.alpha("white", 0.35)
            }
        }
    }
}
