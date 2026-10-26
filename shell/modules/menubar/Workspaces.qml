import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import qs.common
import qs.services

// Spaces indicator: one capsule per workspace. Occupied spaces show their
// apps' icons (or a dot), the active one is highlighted. Click to switch,
// scroll to step through.
Item {
    id: root

    required property var screen
    property color fg: "white"
    property bool plain: false // no capsule of its own (inside the island)
    property bool hot: false // CPU running hot: the active chip turns red
    property real activeCenter: width / 2 // x of the active chip's centre (the island's heat ripples start there)
    readonly property string style: Config.options.menubar.workspaces // dots | icons
    readonly property var monitor: Hyprland.monitorFor(screen)
    readonly property int activeId: monitor?.activeWorkspace?.id ?? 1
    readonly property var occupied: {
        const m = {};
        for (const id of Windows.occupiedIds) {
            const wins = Windows.on(id);
            if (wins[0].workspace?.monitor === monitor)
                m[id] = wins.map(t => t.lastIpcObject?.class ?? "");
        }
        return m;
    }
    // This monitor's spaces: the ones living on it, and on the first screen
    // also 1..workspaceCount (ids are global in Hyprland, so a second
    // monitor mustn't show the first one's spaces)
    readonly property bool primary: screen === Quickshell.screens[0]
    readonly property var ids: {
        const mine = new Set(), elsewhere = new Set();
        for (const w of Hyprland.workspaces.values) {
            if (w.id <= 0)
                continue; // special workspaces
            (w.monitor === monitor ? mine : elsewhere).add(w.id);
        }
        mine.add(activeId);
        for (const id of Object.keys(occupied))
            mine.add(Number(id));
        if (primary)
            for (let i = 1; i <= Config.options.menubar.workspaceCount; i++)
                if (!elsewhere.has(i))
                    mine.add(i);
        return [...mine].sort((a, b) => a - b);
    }
    readonly property int count: ids.length

    implicitWidth: row.implicitWidth + 8
    implicitHeight: 22

    function go(id) {
        Hyprland.dispatch("hl.dsp.focus({ workspace = " + (typeof id === "string" ? JSON.stringify(id) : id) + " })");
    }

    Rectangle {
        anchors.fill: parent
        visible: !root.plain
        radius: height / 2
        color: Qt.alpha(root.fg, 0.1)
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: root.ids
            Rectangle {
                id: ws
                required property int modelData
                readonly property int wsId: modelData
                readonly property bool active: wsId === root.activeId
                readonly property var apps: [...new Set(root.occupied[wsId] ?? [])].slice(0, 3)
                readonly property bool showIcons: root.style === "icons" && apps.length > 0
                height: 18
                width: showIcons ? apps.length * 14 + 10 : (active ? 26 : 18)
                radius: 9
                color: active ? (root.hot ? Qt.tint(Qt.alpha(root.fg, 0.28), Qt.alpha(Theme.accents.red, 0.45)) : Qt.alpha(root.fg, 0.28)) : wsMouse.containsMouse ? Qt.alpha(root.fg, 0.14) : "transparent"
                function report() {
                    // (may run late, after the chip list was rebuilt)
                    if (active && ws.parent && root)
                        root.activeCenter = ws.mapToItem(root, ws.width / 2, 0).x;
                }
                onActiveChanged: Qt.callLater(report)
                onXChanged: report()
                onWidthChanged: report()
                Behavior on width { NumberAnimation { duration: Theme.motion.hover } }
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

                Row {
                    anchors.centerIn: parent
                    visible: ws.showIcons
                    spacing: 2
                    Repeater {
                        model: ws.apps
                        IconImage {
                            required property string modelData
                            implicitSize: 12
                            source: Apps.iconFor(modelData)
                        }
                    }
                }
                Rectangle {
                    anchors.centerIn: parent
                    visible: !ws.showIcons
                    width: ws.active ? 12 : 5
                    height: 5
                    radius: 2.5
                    color: root.fg
                    opacity: ws.active ? 1 : (root.occupied[ws.wsId] ? 0.8 : 0.35)
                    Behavior on width { NumberAnimation { duration: Theme.motion.hover } }
                }
                MouseArea {
                    id: wsMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.go(ws.wsId)
                }
            }
        }
    }

    WheelHandler {
        onWheel: e => root.go(e.angleDelta.y < 0 ? "e+1" : "e-1")
    }
}
