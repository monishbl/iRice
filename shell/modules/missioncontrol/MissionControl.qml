import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// Mission Control: Spaces along the top (plain containers with a tile per
// window), the current Space's windows spread out below as live previews.
// Click a window to focus it, a Space to switch.
// App Exposé (Panels.appExpose): the same, but only one app's windows from
// every space, with the app's name on top instead of the Spaces.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "missionControl"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "missioncontrol"

            readonly property bool shown: Panels.isOpen("missionControl", screen)
            readonly property var monitor: Hyprland.monitorFor(screen)
            readonly property int activeWs: monitor?.activeWorkspace?.id ?? 1
            readonly property var spaces: Hyprland.workspaces.values.filter(w => w.id > 0 && w.monitor === monitor).sort((a, b) => a.id - b.id)
            // App Exposé: the app this opened for (kept while it animates out)
            property string exposeFor: ""
            readonly property var exposeEntry: exposeFor ? (DesktopEntries.byId(exposeFor) ?? Apps.entryFor(exposeFor)) : null
            function ofApp(t) {
                const cls = t.lastIpcObject?.class ?? "";
                return cls === exposeFor || Apps.entryFor(cls)?.id === exposeFor;
            }
            readonly property var windows: exposeFor ? Hyprland.toplevels.values.filter(t => t.lastIpcObject?.size && (t.workspace?.id ?? 0) > 0 && ofApp(t)) : Windows.on(activeWs).filter(t => t.lastIpcObject?.size)
            Connections {
                target: Panels
                function onExposeAppChanged() {
                    if (Panels.exposeApp !== "" && win.shown)
                        win.exposeFor = Panels.exposeApp;
                }
            }

            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: {
                progress = Qt.binding(() => shown ? 1 : 0);
                Hyprland.refreshToplevels();
            }
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.panel; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            onShownChanged: if (shown) {
                exposeFor = Panels.exposeApp;
                Hyprland.refreshToplevels();
            }

            function focusWindow(t) {
                Panels.close();
                const id = t.workspace?.id ?? 0;
                if (id > 0 && id !== activeWs) // App Exposé: a window on another space
                    focusSpace(id);
                Qt.callLater(() => t.wayland?.activate());
            }
            function focusSpace(id) {
                Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
            }

            // Blurred wallpaper background (a borderless full-screen glass)
            Glass {
                anchors.fill: parent
                radius: 0
                bevel: 0
                refraction: 0
                shadowMargin: 0
                rim: 0
                forceLiquid: true
                opacity: win.progress
            }
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.18 * win.progress)
            }

            Item {
                anchors.fill: parent
                focus: win.shown
                Keys.onEscapePressed: Panels.close()
            }
            MouseArea {
                anchors.fill: parent
                onClicked: Panels.close()
            }

            // App Exposé: the app's icon and name where the Spaces would be
            Row {
                visible: win.exposeFor !== ""
                anchors.horizontalCenter: parent.horizontalCenter
                y: 56 + (1 - win.progress) * -30
                opacity: win.progress
                spacing: 12
                AppIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    size: 40
                    source: Apps.iconFor(win.exposeEntry ?? win.exposeFor)
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    Text {
                        text: win.exposeEntry?.name ?? win.exposeFor
                        color: "white"
                        font { family: Theme.font.family; pixelSize: 20; weight: Font.DemiBold }
                    }
                    Text {
                        text: win.windows.length + (win.windows.length === 1 ? " window" : " windows")
                        color: Qt.rgba(1, 1, 1, 0.7)
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
            }

            // Spaces bar
            Row {
                id: spacesBar
                visible: win.exposeFor === ""
                anchors.horizontalCenter: parent.horizontalCenter
                y: 44 + (1 - win.progress) * -30
                opacity: win.progress
                spacing: 16

                Repeater {
                    model: win.spaces

                    Column {
                        id: space
                        required property var modelData
                        readonly property bool current: modelData.id === win.activeWs
                        readonly property real s: 150 / (win.screen?.width ?? 1920)
                        spacing: 6

                        Rectangle {
                            width: 150
                            height: Math.round((win.screen?.height ?? 1080) * space.s)
                            radius: 8
                            // A plain container, like Overview (no wallpaper)
                            color: Theme.dark ? "#1A1A1D" : "#F4F4F5"
                            border.width: space.current ? 2 : 1
                            border.color: space.current ? Theme.accent : (Theme.dark ? "#36363C" : "#D4D4D8")
                            clip: true

                            Repeater {
                                model: space.modelData.toplevels.values.filter(t => t.lastIpcObject?.at)
                                Rectangle {
                                    required property var modelData
                                    readonly property var ipc: modelData.lastIpcObject
                                    x: (ipc.at[0] - (win.monitor?.x ?? 0)) * space.s
                                    y: (ipc.at[1] - (win.monitor?.y ?? 0)) * space.s
                                    width: ipc.size[0] * space.s
                                    height: ipc.size[1] * space.s
                                    radius: 3
                                    color: Theme.dark ? "#2C2C31" : "#FFFFFF"
                                    border.width: 1
                                    border.color: Theme.dark ? "#36363C" : "#E4E4E7"
                                    IconImage {
                                        anchors.centerIn: parent
                                        implicitSize: Math.min(20, parent.height * 0.6)
                                        source: Apps.iconFor(parent.ipc.class)
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.focusSpace(space.modelData.id)
                            }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "Desktop " + space.modelData.id
                            color: space.current ? "white" : Qt.rgba(1, 1, 1, 0.7)
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: space.current ? Font.DemiBold : Font.Medium }
                        }
                    }
                }
            }

            // Windows of the current Space, laid out in the grid that keeps them largest.
            Item {
                id: stage
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom; top: parent.top; margins: 48; topMargin: 44 + spacesBar.height + 40 }
                opacity: win.progress
                scale: 0.96 + 0.04 * win.progress

                readonly property int gap: 32
                readonly property var layout: {
                    const n = win.windows.length;
                    if (n === 0)
                        return { cols: 1, rows: 1 };
                    let best = { cols: 1, rows: n, scale: 0 };
                    for (let cols = 1; cols <= n; cols++) {
                        const rows = Math.ceil(n / cols);
                        const cw = (width - gap * (cols - 1)) / cols, ch = (height - gap * (rows - 1) - 28 * rows) / rows;
                        let worst = Infinity;
                        for (const t of win.windows) {
                            const [w, h] = t.lastIpcObject.size;
                            worst = Math.min(worst, cw / w, ch / h);
                        }
                        if (worst > best.scale)
                            best = { cols: cols, rows: rows, scale: worst };
                    }
                    return best;
                }

                Text {
                    anchors.centerIn: parent
                    visible: win.windows.length === 0
                    text: win.exposeFor !== "" ? "No open windows" : "No windows on this desktop"
                    color: Qt.rgba(1, 1, 1, 0.7)
                    font { family: Theme.font.family; pixelSize: Theme.font.title }
                }

                Repeater {
                    model: win.windows

                    Item {
                        id: tile
                        required property var modelData
                        required property int index
                        readonly property var ipc: modelData.lastIpcObject
                        readonly property int col: index % stage.layout.cols
                        readonly property int row: Math.floor(index / stage.layout.cols)
                        readonly property int rowCount: Math.min(stage.layout.cols, win.windows.length - row * stage.layout.cols)
                        readonly property real cellW: (stage.width - stage.gap * (stage.layout.cols - 1)) / stage.layout.cols
                        readonly property real cellH: (stage.height - stage.gap * (stage.layout.rows - 1)) / stage.layout.rows
                        readonly property real k: Math.min(stage.layout.scale, 0.9)
                        readonly property real rowOffset: (stage.layout.cols - rowCount) * (cellW + stage.gap) / 2

                        width: ipc.size[0] * k
                        height: ipc.size[1] * k
                        x: rowOffset + col * (cellW + stage.gap) + (cellW - width) / 2
                        y: row * (cellH + stage.gap) + (cellH - 28 - height) / 2

                        ClippingRectangle {
                            anchors.fill: parent
                            radius: 12
                            color: Qt.rgba(0.1, 0.1, 0.12, 1)
                            scale: hover.containsMouse ? 1.02 : 1
                            Behavior on scale { NumberAnimation { duration: Theme.motion.hover } }
                            border.width: hover.containsMouse ? 3 : 0
                            border.color: Theme.accent

                            ScreencopyView {
                                anchors.fill: parent
                                captureSource: win.shown ? tile.modelData.wayland : null
                                live: win.shown
                                constraintSize: Qt.size(tile.width, tile.height)
                            }
                        }

                        IconImage {
                            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: -16 }
                            implicitSize: 40
                            source: Apps.iconFor(tile.ipc.class)
                        }

                        Rectangle {
                            visible: hover.containsMouse
                            anchors { horizontalCenter: parent.horizontalCenter; top: parent.bottom; topMargin: 30 }
                            width: Math.min(titleText.implicitWidth + 20, parent.width + 40)
                            height: 24
                            radius: 12
                            color: Qt.rgba(0.12, 0.12, 0.14, 0.9)
                            Text {
                                id: titleText
                                anchors.centerIn: parent
                                width: Math.min(implicitWidth, parent.width - 20)
                                elide: Text.ElideRight
                                text: tile.modelData.title || tile.ipc.class
                                color: "white"
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                            }
                        }

                        MouseArea {
                            id: hover
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: win.focusWindow(tile.modelData)
                        }
                    }
                }
            }
        }
    }
}
