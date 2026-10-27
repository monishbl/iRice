import QtQuick
import QtQml.Models
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// Finder-style window tabs (Settings → Appearance → Window tabs: Finder).
// Hyprland groups the windows and keeps an empty bar's space above them
// (hypr/irice/tabs.lua); this draws the tabs there: app icon and title,
// the front tab raised, close on hover, + for a new window of the app.
// Which tab is in front comes from the irice-glass plugin. While windows
// move or spaces switch, the strips step out of the way and come back
// once things settle, so they never trail behind.
Variants {
    model: Config.options.appearance.windowTabs === "finder" ? Quickshell.screens : []

    Scope {
        id: win
        required property var modelData
        readonly property var screen: modelData

        readonly property var monitor: Hyprland.monitorFor(screen)
        readonly property int barHeight: 30
        readonly property int gap: 4 // group:groupbar:gaps_out


        // [{ current, windows: [toplevel], x, y, w }] on this screen's space
        property var strips: []
        property var groups: []
        function rebuild() {
            const ws = monitor?.activeWorkspace?.id;
            const byAddr = {};
            for (const t of Hyprland.toplevels.values)
                byAddr["0x" + t.address.replace(/^0x/, "")] = t;
            const out = [];
            for (const g of groups) {
                const wins = g.windows.map(a => byAddr[a]).filter(t => t);
                const cur = byAddr[g.current] ?? wins[0];
                const o = cur?.lastIpcObject;
                if (!o?.at || wins.length < 2 || cur.workspace?.id !== ws || o.fullscreen > 0)
                    continue;
                out.push({ current: g.current, locked: g.locked, windows: wins, x: o.at[0] - (monitor?.x ?? 0), y: o.at[1] - (monitor?.y ?? 0) - gap - barHeight, w: o.size[0] });
            }
            // Variants makes a window per strip and remakes them all for a new
            // list: only replace it when something about the strips changed
            // (most focus events don't), so they don't flash
            const key = JSON.stringify(out.map(o => [o.current, o.locked, o.windows.map(t => t.address), o.x, o.y, o.w]));
            if (key === stripsKey)
                return;
            stripsKey = key;
            strips = out;
        }
        property string stripsKey: ""

        // Fresh group data after anything that can change it
        Process {
            id: query
            command: ["hyprctl", "irice-glass", "groups"]
            stdout: StdioCollector {
                onStreamFinished: {
                    try {
                        win.groups = JSON.parse(text);
                    } catch (e) {
                        win.groups = [];
                    }
                    win.rebuild();
                    settle.restart();
                }
            }
        }
        property bool settled: true
        Timer {
            id: settle
            interval: 220
            onTriggered: win.settled = true
        }
        function refresh(moving) {
            if (moving)
                settled = false;
            Hyprland.refreshToplevels();
            later.restart();
        }
        Timer {
            id: later
            interval: 120 // geometry arrives with the refresh
            onTriggered: query.running = true
        }
        Component.onCompleted: refresh(false)
        Connections {
            target: Hyprland
            function onRawEvent(event) {
                const moving = ["workspace", "workspacev2", "movewindow", "movewindowv2", "changefloatingmode", "fullscreen", "openwindow", "closewindow", "irice-drag", "moveworkspace", "moveworkspacev2", "configreloaded", "monitoradded", "monitorremoved"].includes(event.name);
                const grouping = ["togglegroup", "moveintogroup", "moveoutofgroup", "activewindow", "activewindowv2", "lockgroups", "ignoregrouplock"].includes(event.name);
                if (moving || grouping)
                    win.refresh(moving && !(event.name === "irice-drag" && event.data.startsWith("drop")));
            }
        }

        // One small window per tab strip, placed over its group's bar
        Variants {
            model: win.strips
            PanelWindow {
                id: stripWin
                required property var modelData
                screen: win.screen
                WlrLayershell.namespace: "irice:tabs"
                WlrLayershell.layer: WlrLayer.Top
                exclusionMode: ExclusionMode.Ignore
                anchors { top: true; left: true }
                margins { left: Math.max(0, modelData.x); top: Math.max(0, modelData.y) }
                implicitWidth: modelData.w
                implicitHeight: win.barHeight
                color: "transparent"
                Item {
                    id: strip
                    readonly property var modelData: stripWin.modelData
                    anchors.fill: parent
                    opacity: win.settled ? 1 : 0
                    Behavior on opacity { OpacityAnimator { duration: win.settled ? 150 : 60 } }

                    // The well the tabs sit in
                    Rectangle {
                        anchors.fill: parent
                        radius: 10
                        color: Theme.dark ? Qt.rgba(0.1, 0.1, 0.11, 0.82) : Qt.rgba(0.93, 0.93, 0.94, 0.88)
                        border.width: 1
                        border.color: Theme.dark ? "#26262A" : "#E4E4E7"
                    }
                    Row {
                        id: tabs
                        x: 3
                        y: 3
                        readonly property real tabW: (strip.width - 6 - plus.width - 2) / Math.max(1, strip.modelData.windows.length)
                        Repeater {
                            model: strip.modelData.windows
                            Item {
                                id: tab
                                required property var modelData
                                required property int index
                                readonly property string addr: "0x" + modelData.address.replace(/^0x/, "")
                                readonly property bool front: addr === strip.modelData.current
                                width: tabs.tabW
                                height: win.barHeight - 6
                                Rectangle {
                                    anchors.fill: parent
                                    radius: 7
                                    color: tab.front ? (Theme.dark ? "#2C2C31" : "#FFFFFF") : tabMouse.containsMouse ? (Theme.dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(0, 0, 0, 0.05)) : "transparent"
                                    border.width: tab.front ? 1 : 0
                                    border.color: Theme.dark ? "#36363C" : "#E4E4E7"
                                }
                                // A thin divider between back tabs
                                Rectangle {
                                    visible: tab.index > 0 && !tab.front && strip.modelData.windows[tab.index - 1] && ("0x" + strip.modelData.windows[tab.index - 1].address.replace(/^0x/, "")) !== strip.modelData.current
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 1
                                    height: parent.height * 0.5
                                    color: Theme.color.separator
                                }
                                Row {
                                    anchors.centerIn: parent
                                    width: Math.min(implicitWidth, parent.width - 36)
                                    spacing: 6
                                    AppIcon {
                                        anchors.verticalCenter: parent.verticalCenter
                                        size: 14
                                        source: Apps.iconFor(Apps.entryFor(tab.modelData.lastIpcObject?.class ?? "") ?? (tab.modelData.lastIpcObject?.class ?? ""))
                                    }
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: Math.min(implicitWidth, tab.width - 60)
                                        text: tab.modelData.title || (tab.modelData.lastIpcObject?.class ?? "")
                                        elide: Text.ElideRight
                                        color: tab.front ? Theme.color.label : Theme.color.secondary
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: tab.front ? Font.Medium : Font.Normal }
                                    }
                                }
                                MouseArea {
                                    id: tabMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                    onClicked: m => m.button === Qt.MiddleButton
                                        ? Hyprland.dispatch("hl.dsp.window.close({ window = \"address:" + tab.addr + "\" })")
                                        : Hyprland.dispatch("hl.dsp.focus({ window = \"address:" + tab.addr + "\" })")
                                }
                                // Close, on hover
                                Rectangle {
                                    visible: tabMouse.containsMouse || closeMouse.containsMouse
                                    anchors { left: parent.left; leftMargin: 6; verticalCenter: parent.verticalCenter }
                                    width: 16
                                    height: 16
                                    radius: 4
                                    color: closeMouse.containsMouse ? Theme.color.fillHover : "transparent"
                                    Icon { anchors.centerIn: parent; name: "x"; bold: true; size: 10; color: Theme.color.secondary }
                                    MouseArea {
                                        id: closeMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        onClicked: Hyprland.dispatch("hl.dsp.window.close({ window = \"address:" + tab.addr + "\" })")
                                    }
                                }
                            }
                        }
                    }
                    // New window of the same app, as another tab
                    Rectangle {
                        id: plus
                        anchors { right: parent.right; rightMargin: 3; verticalCenter: parent.verticalCenter }
                        width: 24
                        height: 24
                        radius: 7
                        color: plusMouse.containsMouse ? Theme.color.fill : "transparent"
                        Icon { anchors.centerIn: parent; name: "plus"; bold: true; size: 12; color: Theme.color.secondary }
                        MouseArea {
                            id: plusMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: Launch.app(Apps.entryFor(strip.modelData.windows[0]?.lastIpcObject?.class ?? ""))
                        }
                    }
                }
            }
        }
    }
}
