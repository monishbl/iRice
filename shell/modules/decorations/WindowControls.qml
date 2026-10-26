import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Window controls (Settings → Decorations → Window controls): a small pill at
// the top centre of the focused window. Hover it to reveal close, minimize and
// zoom; rest on the green button for the tiling grid. Hidden for fullscreen.
// With the irice-glass plugin the compositor draws the pill (it can't lag
// behind a moving window); this then only shows the tile grid, when the
// plugin reports a rest on the green button.
Variants {
    model: Config.options.appearance.decorations === "controls" ? Quickshell.screens : []

    GlassWindow {
        id: win
        required property var modelData
        screen: modelData
        name: "windowcontrols"

        readonly property var monitor: Hyprland.monitorFor(screen)
        readonly property bool pluginMode: Decorations.pluginControls
        property var tileTarget: null // plugin mode: the window whose green button was rested on
        property point tileAt // plugin mode: bottom centre of that button, global
        readonly property var target: pluginMode ? tileTarget : Apps.activeHyprland
        readonly property var ipc: {
            Desktop.revision; // follow window moves
            return target?.lastIpcObject ?? null;
        }
        readonly property bool onThisScreen: !!ipc?.at && (target?.monitor?.name ?? "") === (monitor?.name ?? "-") && !(ipc.fullscreen > 0)
        readonly property bool expanded: hover.hovered || tiling
        property bool tiling: false // the tile grid under the green button
        onExpandedChanged: if (!hover.hovered && !pluginMode) tiling = false
        readonly property int head: pluginMode ? 8 : 24 + 8 // traffic lights above the grid (or just padding)
        readonly property int m: 12
        readonly property var tileRows: [["left", "right", "top", "bottom"], ["topLeft", "topRight", "bottomLeft", "bottomRight"], ["leftThird", "centerThird", "rightThird", "fill"]]

        visible: onThisScreen && (!pluginMode || tiling)
        WlrLayershell.layer: WlrLayer.Top
        anchors { top: true; left: true }
        implicitWidth: (tiling ? 196 : expanded ? 76 : 44) + 2 * m
        implicitHeight: (tiling ? head + 3 * 32 + 2 * 4 + 8 : expanded ? 24 : 10) + 2 * m
        margins.left: pluginMode ? tileAt.x - (monitor?.x ?? 0) - implicitWidth / 2 : (ipc?.size ? ipc.at[0] - (monitor?.x ?? 0) + ipc.size[0] / 2 : 0) - implicitWidth / 2
        margins.top: pluginMode ? tileAt.y - (monitor?.y ?? 0) + 6 - m : (ipc?.at ? ipc.at[1] - (monitor?.y ?? 0) + 4 : 0) - m

        // Plugin mode: the plugin says the pointer rests on a green button
        Connections {
            target: Hyprland
            enabled: win.pluginMode
            function onRawEvent(event) {
                if (event.name !== "irice-controls")
                    return;
                const [kind, addr, x, y] = event.data.split(",");
                const mx = win.monitor?.x ?? 0, my = win.monitor?.y ?? 0;
                if (kind !== "tile" || x < mx || y < my || x >= mx + (win.screen?.width ?? 0) || y >= my + (win.screen?.height ?? 0))
                    return;
                win.tileTarget = Hyprland.toplevels.values.find(t => (t.address.startsWith("0x") ? t.address : "0x" + t.address) === addr) ?? null;
                win.tileAt = Qt.point(Number(x), Number(y));
                win.tiling = !!win.tileTarget;
                tileLinger.restart();
            }
        }
        // Close the grid if the pointer never comes over, or once it leaves
        Timer {
            id: tileLinger
            interval: 1500
            onTriggered: if (!hover.hovered) win.tiling = false
        }
        Behavior on implicitWidth { NumberAnimation { duration: Theme.motion.hover } }
        Behavior on implicitHeight { NumberAnimation { duration: Theme.motion.hover } }

        HoverHandler {
            id: hover
            onHoveredChanged: if (!hovered) win.tiling = false
        }
        Timer {
            id: tileDelay
            interval: 500
            onTriggered: win.tiling = true
        }

        Glass {
            anchors.fill: parent
            radius: win.tiling ? 18 : height / 2
            bevel: 6
            shadowMargin: win.m
            forceFrosted: true
        }

        // Collapsed: a grab-handle style hint
        Rectangle {
            anchors.centerIn: parent
            visible: !win.expanded && !win.pluginMode
            width: 24
            height: 4
            radius: 2
            color: Theme.color.secondary
        }

        // Expanded: traffic lights
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            y: win.m + (24 - 14) / 2
            visible: win.expanded && !win.pluginMode
            spacing: 8
            Repeater {
                model: [
                    { color: "#FF5F57", icon: "x", act: () => Apps.windowActionFor(win.target, "close") },
                    { color: "#FEBC2E", icon: "minus", act: () => Apps.minimize(win.target) },
                    { color: "#28C840", icon: "arrows-out-simple", act: () => Apps.windowActionFor(win.target, "fullscreen", { mode: "maximized", action: "toggle" }) }
                ]
                Rectangle {
                    required property var modelData
                    width: 14
                    height: 14
                    radius: 7
                    color: modelData.color
                    Icon {
                        anchors.centerIn: parent
                        name: parent.modelData.icon
                        bold: true
                        size: 9
                        color: Qt.rgba(0, 0, 0, 0.55)
                    }
                    MouseArea {
                        anchors { fill: parent; margins: -3 }
                        hoverEnabled: true
                        onClicked: parent.modelData.act()
                        // Resting on the green button opens the tile grid, as in Tahoe
                        onContainsMouseChanged: {
                            if (parent.modelData.icon !== "arrows-out-simple")
                                return;
                            if (containsMouse)
                                tileDelay.restart();
                            else
                                tileDelay.stop();
                        }
                    }
                }
            }
        }
    
        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            y: win.m + win.head
            visible: win.tiling
            spacing: 4
            Repeater {
                model: win.tileRows
                Row {
                    id: tileRow
                    required property var modelData
                    spacing: 4
                    Repeater {
                        model: tileRow.modelData
                        Rectangle {
                            id: tile
                            required property string modelData
                            width: 44
                            height: 32
                            radius: Theme.radius.item + 2
                            color: tileMouse.containsMouse ? Theme.accent : "transparent"
                            TileGlyph {
                                anchors.centerIn: parent
                                frac: Tiling.layouts[tile.modelData]
                                hot: tileMouse.containsMouse
                            }
                            MouseArea {
                                id: tileMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    win.tiling = false;
                                    Tiling.snap(tile.modelData, win.target);
                                }
                            }
                        }
                    }
                }
            }
        }
}
}
