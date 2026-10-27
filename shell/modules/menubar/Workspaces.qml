import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import qs.common
import qs.glass
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
    // Every space's apps, wherever it lives
    readonly property var occupied: {
        const m = {};
        for (const id of Windows.occupiedIds)
            m[id] = Windows.on(id).map(t => t.lastIpcObject?.class ?? "");
        return m;
    }
    // Which display each space lives on
    readonly property var monitorOf: {
        const m = {};
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0)
                m[w.id] = w.monitor;
        return m;
    }
    // One list for every display: 1..workspaceCount and every space in use,
    // wherever it lives (spaces on another display are marked and dimmed);
    // with menubar.otherSpaces off, only this display's own
    readonly property bool primary: screen === Quickshell.screens[0]
    readonly property bool showOthers: Config.options.menubar.otherSpaces
    readonly property var ids: {
        const all = new Set([activeId]);
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0 && (showOthers || w.monitor === monitor))
                all.add(w.id);
        for (const id of Object.keys(occupied))
            if (showOthers || monitorOf[id] === monitor)
                all.add(Number(id));
        for (let i = 1; i <= Config.options.menubar.workspaceCount; i++)
            if (showOthers ? true : (primary && !monitorOf[i]) || monitorOf[i] === monitor)
                all.add(i);
        return [...all].sort((a, b) => a - b);
    }
    readonly property int count: ids.length

    // More spaces than "Spaces always shown": that many chips, a window
    // that slides (the arrows, or switching spaces: the current one always
    // stays in view), and an arrow at each end with more past it, dotted
    // when something there has windows
    readonly property int cap: Math.max(1, Config.options.menubar.workspaceCount)
    readonly property bool overflow: (ids?.length ?? 0) > cap
    property int start: 0
    function clampStart(v) {
        return Math.max(0, Math.min(Math.max(0, (ids?.length ?? 0) - cap), v));
    }
    function keepActive() {
        if (!ids) // (still starting up)
            return;
        const i = ids.indexOf(activeId);
        let st = clampStart(start);
        if (i >= 0 && i < st)
            st = i;
        else if (i >= st + cap)
            st = i - cap + 1;
        start = clampStart(st);
    }
    onActiveIdChanged: keepActive()
    onIdsChanged: keepActive()
    onCapChanged: keepActive()
    readonly property var shown: overflow ? ids.slice(start, start + cap) : ids
    readonly property var hiddenLeft: overflow ? ids.slice(0, start) : []
    readonly property var hiddenRight: overflow ? ids.slice(start + cap) : []
    function busy(list) {
        return list.some(id => !!occupied[id]);
    }

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

        Arrow {
            visible: root.overflow
            side: -1
            more: root.hiddenLeft.length > 0
            hot: root.busy(root.hiddenLeft)
        }

        Repeater {
            model: root.shown
            Rectangle {
                id: ws
                required property int modelData
                readonly property int wsId: modelData
                readonly property bool active: wsId === root.activeId
                // Lives on another display (and, if shown there, is that one's current space)
                readonly property bool elsewhere: !!root.monitorOf[wsId] && root.monitorOf[wsId] !== root.monitor
                readonly property bool shownThere: elsewhere && root.monitorOf[wsId]?.activeWorkspace?.id === wsId
                opacity: elsewhere ? 0.55 : 1
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
                // On another display: a small bar under it (brighter while
                // it's that display's current space)
                Rectangle {
                    visible: ws.elsewhere
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 1 }
                    width: 8
                    height: 1.5
                    radius: 1
                    color: root.fg
                    opacity: ws.shownThere ? 0.9 : 0.5
                }
                MouseArea {
                    id: wsMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: m => {
                        if (m.button === Qt.RightButton)
                            root.openMenu(ws.wsId, ws.mapToItem(null, ws.width / 2, ws.height).x);
                        else
                            root.go(ws.wsId);
                    }
                }
            }
        }

        Arrow {
            visible: root.overflow
            side: 1
            more: root.hiddenRight.length > 0
            hot: root.busy(root.hiddenRight)
        }
    }

    component Arrow: Item {
        id: arrow
        property int side: 1 // -1 left, 1 right
        property bool more: false
        property bool hot: false
        width: 12
        height: 18
        opacity: more ? 1 : 0.25
        Icon {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: arrow.hot ? -2 : 0
            name: arrow.side < 0 ? "caret-left" : "caret-right"
            bold: true
            size: 10
            color: root.fg
            opacity: arrowMouse.containsMouse && arrow.more ? 1 : 0.7
        }
        // Something on that side has windows
        Rectangle {
            visible: arrow.hot
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 1 }
            width: 3
            height: 3
            radius: 1.5
            color: root.fg
        }
        MouseArea {
            id: arrowMouse
            anchors { fill: parent; margins: -4 }
            hoverEnabled: true
            enabled: arrow.more
            onClicked: root.start = root.clampStart(root.start + arrow.side)
        }
    }

    // Right-click a space: move it to another display (it stays there, and
    // comes back to it after that display was unplugged; WorkspaceHomes)
    function openMenu(id, x) {
        const here = monitorOf[id] ?? monitor;
        const items = Hyprland.monitors.values.filter(m => m !== here).map(m => ({
            text: "Move to " + (m.description?.split(" ").slice(0, 3).join(" ") || m.name) + " (" + m.name + ")",
            action: () => Hyprland.dispatch("hl.dsp.workspace.move({ workspace = " + id + ", monitor = \"" + m.name + "\" })")
        }));
        if (items.length === 0)
            items.push({ text: "Only one display is connected", enabled: false, action: () => {} });
        menuX = x;
        menuItems = items;
        menuLoader.active = true;
    }
    property real menuX: 0
    property var menuItems: []
    Loader {
        id: menuLoader
        active: false
        sourceComponent: ContextMenu {
            screen: root.screen
            pointX: root.menuX
            pointY: 30
            upward: false
            items: root.menuItems
            onDismissed: menuLoader.active = false
        }
    }

    WheelHandler {
        onWheel: e => root.go(e.angleDelta.y < 0 ? "e+1" : "e-1")
    }
}
