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
    // This display, by name (Spaces works by names: Hyprland's monitor
    // objects compared by identity could be stale ones, and then only the
    // current space was listed)
    readonly property string name: screen?.name ?? ""
    readonly property int activeId: Spaces.active[name] ?? 0
    readonly property var visibleIds: Object.keys(Spaces.active).map(n => Spaces.active[n]).filter(id => id > 0)
    function isVisibleSpace(id) { return visibleIds.includes(id); }
    // Every space's apps, wherever it lives
    readonly property var occupied: {
        const m = {};
        for (const id of Windows.occupiedIds)
            m[id] = Windows.on(id).map(t => t.lastIpcObject?.class ?? "");
        return m;
    }
    // Which display each space lives on (by name)
    readonly property var monitorOf: {
        const m = {};
        for (const w of Spaces.workspaces)
            m[w.id] = w.monitor;
        return m;
    }
    // This display's own spaces (numbered from 1 here), then those of a
    // display that's unplugged and staying here for now, then (with
    // menubar.otherSpaces) the other displays' spaces. Groups after the
    // first start with the name of the display they belong to.
    readonly property bool showOthers: Config.options.menubar.otherSpaces
    readonly property var groups: Spaces.groupsFor(name, showOthers)
    readonly property var ids: {
        const out = [];
        for (const g of groups)
            for (const id of g.ids)
                out.push(id);
        return out;
    }
    // id -> the label shown before it (first of a group after the first)
    readonly property var groupLabel: {
        const m = {};
        for (let i = 1; i < groups.length; i++)
            if (groups[i].ids.length > 0)
                m[groups[i].ids[0]] = groups[i].label;
        return m;
    }
    readonly property int count: ids.length
    // The window of chips (below) slides over this display's own spaces;
    // the other groups always show after them
    readonly property var ownIds: groups[0]?.ids ?? []
    readonly property var restIds: ids.slice(ownIds.length)

    // More spaces than "Spaces always shown": that many chips, a window
    // that slides (the arrows, or switching spaces: the current one always
    // stays in view), and an arrow at each end with more past it, dotted
    // when something there has windows
    readonly property int cap: Math.max(1, Config.options.menubar.workspaceCount)
    readonly property bool overflow: (ownIds?.length ?? 0) > cap
    property int start: 0
    function clampStart(v) {
        return Math.max(0, Math.min(Math.max(0, (ownIds?.length ?? 0) - cap), v));
    }
    function keepActive() {
        if (!ownIds) // (still starting up)
            return;
        const i = ownIds.indexOf(activeId);
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
    // Past the ends: arrows, or (dynamic) the next chips shrink and fade
    // out like Instagram's dots and slide along as you move
    readonly property bool dynamic: Config.options.menubar.workspaceOverflow === "dynamic"
    readonly property var shown: {
        if (!overflow || dynamic)
            return ownIds;
        const page = ownIds.slice(start, start + cap);
        // Every monitor's active space remains visible even outside this page.
        return ownIds.filter(id => page.includes(id) || isVisibleSpace(id));
    }
    readonly property var hiddenLeft: overflow ? ownIds.slice(0, start).filter(id => !isVisibleSpace(id)) : []
    readonly property var hiddenRight: overflow ? ownIds.slice(start + cap).filter(id => !isVisibleSpace(id)) : []
    function busy(list) {
        return list.some(id => !!occupied[id]);
    }

    implicitWidth: row.implicitWidth + 8
    implicitHeight: 22

    function go(id) {
        if (typeof id === "string")
            Hyprland.dispatch("hl.dsp.focus({ workspace = " + JSON.stringify(id) + " })");
        else
            Spaces.go(id);
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
            visible: root.overflow && !root.dynamic
            side: -1
            more: root.hiddenLeft.length > 0
            hot: root.busy(root.hiddenLeft)
        }

        Repeater {
            model: root.shown
            delegate: chip
        }

        Arrow {
            visible: root.overflow && !root.dynamic
            side: 1
            more: root.hiddenRight.length > 0
            hot: root.busy(root.hiddenRight)
        }

        // Spaces of other displays (each group led by its display's name)
        Repeater {
            model: root.restIds
            delegate: chip
        }
    }

    Component {
        id: chip
            Item {
                id: slot
                required property int modelData
                required property int index
                // The first space of a group that isn't this display's own:
                // the name of the display it belongs to, in front
                readonly property string mark: root.groupLabel[modelData] ?? ""
                readonly property real markW: mark !== "" && ws.fall === 1 ? markText.implicitWidth + 10 : 0
                width: markW + ws.width
                height: 18
                visible: ws.fall > 0 || widthAnim.running
                onXChanged: ws.report()
                Text {
                    id: markText
                    visible: slot.markW > 0
                    x: 6
                    anchors.verticalCenter: parent.verticalCenter
                    text: slot.mark
                    color: root.fg
                    opacity: 0.6
                    font { family: Theme.font.family; pixelSize: 10; weight: Font.Medium }
                }
            Rectangle {
                id: ws
                x: slot.markW
                readonly property int wsId: slot.modelData
                readonly property int index: slot.index
                // Dynamic overflow: how far past the shown window (0 inside);
                // one past is smaller and fainter, two past smaller still,
                // beyond that it's gone
                readonly property int past: root.isVisibleSpace(wsId) || !root.overflow || !root.dynamic || !root.ownIds.includes(wsId) ? 0 : index < root.start ? root.start - index : Math.max(0, index - (root.start + root.cap - 1))
                readonly property real fall: past === 0 ? 1 : past === 1 ? 0.7 : past === 2 ? 0.45 : 0
                readonly property bool active: wsId === root.activeId
                // Lives on another display (and, if shown there, is that one's current space)
                readonly property bool elsewhere: !!root.monitorOf[wsId] && root.monitorOf[wsId] !== root.name
                readonly property bool shownThere: elsewhere && Spaces.active[root.monitorOf[wsId]] === wsId
                opacity: (elsewhere && !shownThere ? 0.55 : 1) * (past === 0 ? 1 : past === 1 ? 0.6 : 0.3)
                scale: fall
                Behavior on scale { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
                Behavior on opacity { NumberAnimation { duration: Theme.motion.popover } }
                readonly property var apps: [...new Set(root.occupied[wsId] ?? [])].slice(0, 3)
                readonly property bool showIcons: root.style === "icons" && apps.length > 0
                height: 18
                width: (showIcons ? apps.length * 14 + 10 : (active || shownThere ? 26 : 18)) * fall
                radius: 9
                color: active ? (root.hot ? Qt.tint(Qt.alpha(root.fg, 0.28), Qt.alpha(Theme.accents.red, 0.45)) : Qt.alpha(root.fg, 0.28)) : shownThere ? Qt.alpha(root.fg, 0.22) : wsMouse.containsMouse ? Qt.alpha(root.fg, 0.14) : "transparent"
                function report() {
                    // (may run late, after the chip list was rebuilt)
                    if (active && ws.parent && root)
                        root.activeCenter = ws.mapToItem(root, ws.width / 2, 0).x;
                }
                onActiveChanged: Qt.callLater(report)
                onXChanged: report()
                onWidthChanged: report()
                Behavior on width { NumberAnimation { id: widthAnim; duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
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
                    // empty: a faint dot; with windows: a short pill; current: longer
                    width: ws.active || ws.shownThere ? 14 : root.occupied[ws.wsId] ? 10 : 5
                    height: 5
                    radius: 2.5
                    color: root.fg
                    opacity: ws.active || ws.shownThere ? 1 : (root.occupied[ws.wsId] ? 0.85 : 0.35)
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

    // Right-click a space: move it to another display (it becomes that
    // display's next space, windows and all)
    function openMenu(id, x) {
        const here = monitorOf[id] ?? name;
        const items = Hyprland.monitors.values.filter(m => m.name !== here && Spaces.bases[m.name] !== undefined).map(m => ({
            text: "Move to " + Spaces.labelForName(m.name) + " (" + m.name + ")",
            action: () => Spaces.moveSpaceTo(id, m.name)
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
        onWheel: e => root.go(e.angleDelta.y < 0 ? "m+1" : "m-1") // (this display's spaces)
    }
}
