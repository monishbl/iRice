import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services
import qs.modules.spotlight

// Overview (tap Super): every space as a small screen with its windows, plus
// a search field. Click a space to go there, a window to focus it, drag a
// window onto another space to move it. Typing searches apps and actions
// like Spotlight; Enter opens the first result. Arrows pick a space.
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
            readonly property var occupiedIds: Hyprland.toplevels.values.map(t => t.workspace?.id ?? 0).filter(id => id > 0)
            readonly property int count: Math.max(Config.options.menubar.workspaceCount, activeWs, ...occupiedIds, 1)
            readonly property int columns: Math.min(5, count)
            readonly property real cellW: Math.min(300, (width - 160 - (columns - 1) * 24) / columns)
            readonly property real cellH: cellW * (screen?.height ?? 1080) / (screen?.width ?? 1920)
            readonly property real scaleK: cellW / (screen?.width ?? 1920)
            property int picked: activeWs
            property bool dragging: false // a window is being dragged to another space
            property var results: []
            property int selected: 0

            property real progress: 0
            Component.onCompleted: if (shown) open()
            onShownChanged: shown ? open() : closeAnim.start()
            function open() {
                Hyprland.refreshToplevels();
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
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

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
                        selectionColor: Qt.alpha(Theme.accent, 0.5)
                        clip: true
                        font { family: Theme.font.family; pixelSize: 20 }
                        onTextChanged: {
                            win.results = Search.query(text).filter(r => !r.ai);
                            win.selected = 0;
                        }
                        Keys.onEscapePressed: text !== "" ? text = "" : Panels.close()
                        Keys.onReturnPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onEnterPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onLeftPressed: e => { if (text === "") win.picked = Math.max(1, win.picked - 1); else e.accepted = false; }
                        Keys.onRightPressed: e => { if (text === "") win.picked = Math.min(win.count, win.picked + 1); else e.accepted = false; }
                        Keys.onUpPressed: text !== "" ? win.selected = Math.max(0, win.selected - 1) : win.picked = Math.max(1, win.picked - win.columns)
                        Keys.onDownPressed: text !== "" ? win.selected = Math.min(win.results.length - 1, win.selected + 1) : win.picked = Math.min(win.count, win.picked + win.columns)
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
                    visible: win.results.length === 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: Math.max(searchBox.y + searchBox.height + 40, (parent.height - height) / 2)
                    columns: win.columns
                    spacing: 24

                    Repeater {
                        model: win.count
                        Column {
                            id: space
                            required property int index
                            readonly property int wsId: index + 1
                            readonly property var wins: Hyprland.toplevels.values.filter(t => t.workspace?.id === space.wsId && t.lastIpcObject?.at)
                            spacing: 8

                            Rectangle {
                                id: screenRect
                                width: win.cellW
                                height: win.cellH
                                radius: 14
                                clip: !win.dragging // let a dragged window leave its space
                                z: win.dragging ? 1 : 0
                                color: "black"
                                border.width: space.wsId === win.picked || dropArea.containsDrag ? 3 : space.wsId === win.activeWs ? 2 : 1
                                border.color: dropArea.containsDrag ? Theme.accent : space.wsId === win.picked ? Theme.accent : space.wsId === win.activeWs ? Qt.alpha("white", 0.6) : Qt.alpha("white", 0.15)
                                scale: spaceMouse.pressed ? 0.97 : spaceMouse.containsMouse ? 1.02 : 1
                                Behavior on scale { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.04; epsilon: 0.002 } }

                                Image {
                                    anchors.fill: parent
                                    source: Wallpaper.source
                                    fillMode: Image.PreserveAspectCrop
                                    sourceSize: Qt.size(win.cellW * 2, win.cellH * 2)
                                    asynchronous: true
                                    opacity: 0.85
                                }
                                MouseArea {
                                    id: spaceMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onEntered: win.picked = space.wsId
                                    onClicked: win.go(space.wsId)
                                }
                                DropArea {
                                    id: dropArea
                                    anchors.fill: parent
                                    onDropped: drop => {
                                        if (drop.source?.toplevel)
                                            win.moveWindow(drop.source.toplevel, space.wsId);
                                    }
                                }

                                // Windows, at their real positions
                                Repeater {
                                    model: space.wins
                                    Rectangle {
                                        id: thumb
                                        required property var modelData
                                        readonly property var toplevel: modelData
                                        readonly property var o: modelData.lastIpcObject
                                        x: (o.at[0] - (win.monitor?.x ?? 0)) * win.scaleK
                                        y: (o.at[1] - (win.monitor?.y ?? 0)) * win.scaleK
                                        width: Math.max(16, o.size[0] * win.scaleK)
                                        height: Math.max(12, o.size[1] * win.scaleK)
                                        radius: 6
                                        color: Theme.dark ? "#1A1A1D" : "#FFFFFF"
                                        border.width: 1
                                        border.color: Qt.alpha("white", 0.2)
                                        clip: true
                                        Drag.active: thumbDrag.active
                                        Drag.source: thumb
                                        Drag.hotSpot.x: width / 2
                                        Drag.hotSpot.y: height / 2
                                        z: thumbDrag.active ? 10 : 1
                                        ScreencopyView {
                                            anchors.fill: parent
                                            captureSource: win.shown ? (thumb.modelData.wayland ?? null) : null
                                            live: false
                                            constraintSize: Qt.size(thumb.width * 2, thumb.height * 2)
                                        }
                                        AppIcon {
                                            anchors.centerIn: parent
                                            size: Math.min(28, thumb.height * 0.6)
                                            source: Apps.iconFor(thumb.o.class ?? "")
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            onEntered: win.picked = space.wsId
                                            onClicked: win.focusWindow(thumb.modelData)
                                        }
                                        DragHandler {
                                            id: thumbDrag
                                            onActiveChanged: {
                                                win.dragging = active;
                                                if (!active)
                                                    thumb.Drag.drop();
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
                            }
                        }
                    }
                }
            }
        }
    }
}
