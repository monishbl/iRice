import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services
import qs.modules.spotlight

// Overview (tap Super): this monitor's spaces as miniatures of the screen,
// each window a preview of its content at its real place and size (floating
// windows above tiled ones, the most recently focused on top; fullscreen
// fills the miniature), plus a search field. Click a space to go there, a
// window to focus it, drag a window onto another space to move it. Typing
// searches apps and actions like Spotlight; Enter opens the first result.
// Arrows pick a space.
// Previews (Config overview.previews): "snapshot" captures each window once
// per open, "live" keeps them updating, "hybrid" is live for the current
// space only. Nothing is captured while the Overview is closed.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "overview"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "overview"

            readonly property bool shown: Panels.isOpen("overview", screen)
            readonly property var monitor: Hyprland.monitorFor(screen)
            readonly property int activeWs: monitor?.activeWorkspace?.id ?? 1
            readonly property var ids: Windows.spaceIds(monitor, screen === Quickshell.screens[0])
            readonly property int count: Math.max(1, ids.length)
            readonly property int columns: Math.min(5, count)
            readonly property real cellW: Math.min(300, (width - 160 - (columns - 1) * 24) / columns)
            readonly property real cellH: cellW * (screen?.height ?? 1080) / (screen?.width ?? 1920)
            readonly property real scaleK: cellW / (screen?.width ?? 1920)
            property int picked: activeWs
            property bool dragging: false // a window is being dragged to another space
            property int dragFrom: 0 // the space it's dragged from (drawn above the others)
            property var results: []
            property int selected: 0

            // Window previews: snapshot | live | hybrid (live on the current space)
            readonly property string previewMode: {
                const m = Config.options.overview?.previews;
                return m === "live" || m === "hybrid" ? m : "snapshot";
            }
            function liveFor(wsId) {
                return shown && (previewMode === "live" || (previewMode === "hybrid" && wsId === activeWs));
            }
            // True from open until the close animation ends: previews hold
            // their source (and last frame) while it fades out, and drop it after
            property bool capturing: false
            signal recapture() // reopened mid-close: snapshots take a new picture
            onVisibleChanged: if (!visible) capturing = false

            property real progress: 0
            Component.onCompleted: if (shown) open()
            onShownChanged: shown ? open() : closeAnim.start()
            function open() {
                Hyprland.refreshToplevels();
                if (capturing)
                    recapture();
                capturing = true;
                picked = activeWs;
                field.text = "";
                results = [];
                field.forceActiveFocus();
                closeAnim.stop();
                openAnim.start();
            }
            SpringAnimation { id: openAnim; target: win; property: "progress"; to: 1; spring: Theme.motion.spring + 0.6; damping: Theme.motion.damping + 0.08; epsilon: 0.004 }
            NumberAnimation { id: closeAnim; target: win; property: "progress"; to: 0; duration: Theme.motion.popover * 0.7; easing.type: Easing.InCubic }

            visible: progress > 0 && closeGuard.open
            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // Move the picked space by `n` places in the grid
            function step(n) {
                const i = Math.max(0, ids.indexOf(picked));
                picked = ids[Math.max(0, Math.min(ids.length - 1, i + n))] ?? picked;
            }
            // Search answers that arrive later (files, conversions)
            Connections {
                target: Search
                function onRevisionChanged() {
                    if (!win.shown || field.text === "")
                        return;
                    // Keep the row you moved to (rows shift as answers arrive)
                    const keep = win.selected > 0 ? win.results[win.selected]?.title : undefined;
                    win.results = Search.query(field.text).filter(r => !r.ai);
                    const i = win.results.findIndex(r => r.title === keep);
                    win.selected = i >= 0 ? i : 0;
                }
            }
            function go(id) {
                Panels.close();
                Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
            }
            function focusWindow(t) {
                Panels.close();
                const id = t.workspace?.id;
                if (id && id !== activeWs)
                    Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
                Qt.callLater(() => t.wayland?.activate());
            }
            function moveWindow(t, id) {
                const a = t.address.startsWith("0x") ? t.address : "0x" + t.address;
                Hyprland.dispatch("hl.dsp.window.move({ workspace = " + id + ", follow = false, window = \"address:" + a + "\" })");
                Qt.callLater(Hyprland.refreshToplevels);
            }

            // Dim the screen behind
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.35 * Math.min(1, win.progress))
                MouseArea {
                    anchors.fill: parent
                    onClicked: Panels.close()
                }
            }

            Item {
                id: content
                anchors.fill: parent
                opacity: Math.min(1, win.progress)
                scale: 0.97 + 0.03 * win.progress

                // Search
                Item {
                    id: searchBox
                    width: 560 + 2 * fieldGlass.shadowMargin
                    height: 52 + 2 * fieldGlass.shadowMargin
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 64
                    Glass {
                        id: fieldGlass
                        anchors.fill: parent
                        radius: 26
                        shadowMargin: 20
                    }
                    Icon {
                        x: fieldGlass.shadowMargin + 18
                        anchors.verticalCenter: parent.verticalCenter
                        name: "magnifying-glass"
                        bold: true
                        size: 20
                        color: Theme.color.secondary
                    }
                    TextInput {
                        id: field
                        x: fieldGlass.shadowMargin + 50
                        width: 560 - 70
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.color.label
                        selectionColor: Theme.accentAlpha(0.5)
                        clip: true
                        font { family: Theme.font.family; pixelSize: 20 }
                        onTextChanged: {
                            win.results = Search.query(text).filter(r => !r.ai);
                            win.selected = 0;
                        }
                        Keys.onEscapePressed: text !== "" ? text = "" : Panels.close()
                        Keys.onReturnPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onEnterPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onLeftPressed: e => { if (text === "") win.step(-1); else e.accepted = false; }
                        Keys.onRightPressed: e => { if (text === "") win.step(1); else e.accepted = false; }
                        Keys.onUpPressed: text !== "" ? win.selected = Math.max(0, win.selected - 1) : win.step(-win.columns)
                        Keys.onDownPressed: text !== "" ? win.selected = Math.min(win.results.length - 1, win.selected + 1) : win.step(win.columns)
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: field.text === ""
                            text: "Search apps, or pick a space"
                            color: Theme.color.tertiary
                            font: field.font
                        }
                    }
                }

                // Results while typing
                Item {
                    visible: win.results.length > 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: searchBox.y + searchBox.height - 30
                    width: 560 + 40
                    height: Math.min(resultList.contentHeight, 420) + 16 + 40
                    Glass {
                        anchors.fill: parent
                        radius: 22
                        shadowMargin: 20
                    }
                    MouseArea { anchors.fill: parent; anchors.margins: 20 }
                    ListView {
                        id: resultList
                        x: 28
                        y: 28
                        width: 560 - 16
                        height: parent.height - 56
                        clip: true
                        model: win.results
                        currentIndex: win.selected
                        boundsBehavior: Flickable.StopAtBounds
                        delegate: ResultRow {
                            required property var modelData
                            required property int index
                            width: resultList.width
                            result: modelData
                            selected: index === win.selected
                            onActivated: {
                                Panels.close();
                                modelData.run();
                            }
                        }
                    }
                }

                // Spaces
                Grid {
                    id: spaces
                    // Drag a space by its empty area to put it elsewhere (it
                    // takes the place of the one it's dropped on; Spaces.move).
                    // Windows inside still drag to other spaces as before.
                    property int dragSpace: 0
                    property point dragD: Qt.point(0, 0)
                    property int dropOn: 0
                    function spaceAt(px, py) {
                        let best = 0, dist = 1e9;
                        for (const c of spaces.children) {
                            if (c.wsId === undefined)
                                continue;
                            const d = Math.hypot(c.x + c.width / 2 - px, c.y + c.height / 2 - py);
                            if (d < dist) {
                                dist = d;
                                best = c.wsId;
                            }
                        }
                        return best;
                    }
                    visible: win.results.length === 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: Math.max(searchBox.y + searchBox.height + 40, (parent.height - height) / 2)
                    columns: win.columns
                    spacing: 24

                    Repeater {
                        // ScriptModel keeps delegates (and their captures) when the list is rebuilt
                        model: ScriptModel { values: win.ids }
                        Column {
                            id: space
                            required property int modelData
                            readonly property int wsId: modelData
                            // Grouped windows behind their tab are hidden
                            readonly property var wins: Windows.on(space.wsId).filter(t => t.lastIpcObject?.at && t.lastIpcObject?.size && !t.lastIpcObject.hidden)
                            readonly property bool live: win.liveFor(space.wsId)
                            readonly property bool dragged: spaces.dragSpace === space.wsId
                            // Dragging the space (its empty area, or its title)
                            function dragMove(dx, dy) {
                                if (spaces.dragSpace === 0 && Math.hypot(dx, dy) > 10)
                                    spaces.dragSpace = space.wsId;
                                if (spaces.dragSpace === space.wsId) {
                                    spaces.dragD = Qt.point(dx, dy);
                                    spaces.dropOn = spaces.spaceAt(space.x + space.width / 2 + dx, space.y + space.height / 2 + dy);
                                }
                            }
                            function dragEnd() { // true if it was a drag, not a click
                                if (spaces.dragSpace !== space.wsId)
                                    return false;
                                if (spaces.dropOn > 0 && spaces.dropOn !== space.wsId)
                                    Spaces.move(win.ids, space.wsId, spaces.dropOn);
                                spaces.dragSpace = 0;
                                spaces.dragD = Qt.point(0, 0);
                                spaces.dropOn = 0;
                                return true;
                            }
                            z: win.dragFrom === space.wsId || dragged ? 1 : 0 // a dragged window (or space) passes over the others
                            spacing: 8
                            opacity: dragged ? 0.85 : 1
                            transform: Translate { x: space.dragged ? spaces.dragD.x : 0; y: space.dragged ? spaces.dragD.y : 0 }

                            // The space: a plain neutral container
                            Rectangle {
                                id: screenRect
                                width: win.cellW
                                height: win.cellH
                                radius: 14
                                clip: !win.dragging // let a dragged tile leave its space
                                z: win.dragging ? 1 : 0
                                color: Theme.dark ? "#1A1A1D" : "#F4F4F5"
                                readonly property bool target: spaces.dragSpace > 0 && spaces.dropOn === space.wsId && !space.dragged
                                border.width: space.wsId === win.picked || dropArea.containsDrag || target ? 2 : 1
                                border.color: dropArea.containsDrag || target || space.wsId === win.picked ? Theme.accent : space.wsId === win.activeWs ? (Theme.dark ? "#36363C" : "#D4D4D8") : (Theme.dark ? "#26262A" : "#E4E4E7")
                                scale: spaceMouse.pressed ? 0.97 : spaceMouse.containsMouse ? 1.02 : 1
                                Behavior on scale { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.04; epsilon: 0.002 } }

                                Text {
                                    anchors.centerIn: parent
                                    visible: space.wins.length === 0
                                    text: "No windows"
                                    color: Theme.dark ? "#94949D" : "#6B6B74"
                                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                                }
                                MouseArea {
                                    id: spaceMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: spaces.dragSpace > 0 ? Qt.ClosedHandCursor : Qt.ArrowCursor
                                    property point pressAt: Qt.point(0, 0)
                                    onEntered: win.picked = space.wsId
                                    onPressed: m => pressAt = Qt.point(m.x, m.y)
                                    onPositionChanged: m => {
                                        if (pressed)
                                            space.dragMove(m.x - pressAt.x, m.y - pressAt.y);
                                    }
                                    onReleased: if (!space.dragEnd()) win.go(space.wsId)
                                }
                                DropArea {
                                    id: dropArea
                                    anchors.fill: parent
                                    onDropped: drop => {
                                        const t = drop.source?.toplevel;
                                        if (t && (t.workspace?.id ?? 0) !== space.wsId) {
                                            drop.accept(Qt.MoveAction);
                                            win.moveWindow(t, space.wsId);
                                        } else {
                                            drop.accepted = false; // its own space: it springs back
                                        }
                                    }
                                }

                                // A preview per window, at its real place and size
                                Repeater {
                                    model: ScriptModel { values: space.wins }
                                    Item {
                                        id: thumb
                                        required property var modelData
                                        readonly property var toplevel: modelData
                                        readonly property var o: modelData.lastIpcObject ?? ({})
                                        // fullscreen: 0 none, 1 maximized, 2 fullscreen (bit flags; a bool on old Hyprland)
                                        readonly property int fsMode: o.fullscreen === true ? 2 : (o.fullscreen | 0)
                                        readonly property bool full: (fsMode & 2) !== 0
                                        // Stacking like Hyprland: tiled, then floating by focus (recent on
                                        // top), then fullscreen and maximized
                                        readonly property int stack: fsMode > 0 ? 3000 : o.floating ? 2000 - Math.max(0, Math.min(999, o.focusHistoryID ?? 999)) : 1
                                        // Fullscreen fills the miniature, inside its 1px border
                                        readonly property real homeX: full ? 1 : ((o.at?.[0] ?? 0) - (win.monitor?.x ?? 0)) * win.scaleK
                                        readonly property real homeY: full ? 1 : ((o.at?.[1] ?? 0) - (win.monitor?.y ?? 0)) * win.scaleK
                                        property bool gone: false // dropped on another space, until the move lands
                                        x: homeX
                                        y: homeY
                                        width: full ? screenRect.width - 2 : Math.max(16, (o.size?.[0] ?? 0) * win.scaleK)
                                        height: full ? screenRect.height - 2 : Math.max(12, (o.size?.[1] ?? 0) * win.scaleK)
                                        opacity: gone ? 0 : 1
                                        Drag.active: thumbDrag.active
                                        Drag.source: thumb
                                        Drag.hotSpot.x: width / 2
                                        Drag.hotSpot.y: height / 2
                                        z: thumbDrag.active ? 5000 : stack
                                        scale: thumbDrag.active ? 1.04 : 1
                                        Behavior on scale { ScaleAnimator { duration: 150; easing.type: Easing.OutCubic } }

                                        WindowPreview {
                                            id: preview
                                            anchors.fill: parent
                                            toplevel: thumb.modelData
                                            radius: thumb.full ? 13 : 4 // the space's 14 minus its border
                                            capture: win.capturing
                                            live: space.live
                                            hovered: tileMouse.containsMouse || thumbDrag.active
                                        }
                                        Connections {
                                            target: win
                                            function onRecapture() {
                                                preview.recapture();
                                            }
                                        }
                                        Timer {
                                            running: thumb.gone
                                            interval: 1500 // the move didn't happen: show it again
                                            onTriggered: thumb.gone = false
                                        }
                                        MouseArea {
                                            id: tileMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            onEntered: win.picked = space.wsId
                                            onClicked: win.focusWindow(thumb.modelData)
                                        }
                                        DragHandler {
                                            id: thumbDrag
                                            onActiveChanged: {
                                                win.dragFrom = active ? space.wsId : 0;
                                                win.dragging = active;
                                                if (!active) {
                                                    thumb.gone = thumb.Drag.drop() === Qt.MoveAction;
                                                    // Back home (the drag broke the bindings); if it moved,
                                                    // it stays hidden until it shows up on the new space
                                                    thumb.x = Qt.binding(() => thumb.homeX);
                                                    thumb.y = Qt.binding(() => thumb.homeY);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: "Space " + space.wsId + (space.wins.length > 0 ? " · " + space.wins.length + (space.wins.length === 1 ? " window" : " windows") : "")
                                color: Qt.rgba(1, 1, 1, space.wsId === win.picked ? 0.95 : 0.7)
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                // The title is a handle too (a space full of windows has no empty area)
                                MouseArea {
                                    anchors { fill: parent; margins: -6 }
                                    hoverEnabled: true
                                    cursorShape: spaces.dragSpace > 0 ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                                    property point pressAt: Qt.point(0, 0)
                                    onEntered: win.picked = space.wsId
                                    onPressed: m => pressAt = Qt.point(m.x, m.y)
                                    onPositionChanged: m => {
                                        if (pressed)
                                            space.dragMove(m.x - pressAt.x, m.y - pressAt.y);
                                    }
                                    onReleased: if (!space.dragEnd()) win.go(space.wsId)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
