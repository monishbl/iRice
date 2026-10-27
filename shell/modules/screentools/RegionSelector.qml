import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services

// Region picker: dims the screen, drag to select (live size readout), or
// click a highlighted window to take all of it. Space toggles window mode
// off; Esc cancels. Hides itself before the capture happens.
Scope {
    id: root

    readonly property string mode: ScreenTools.selecting
    property bool hiding: false

    Timer {
        id: captureDelay
        interval: 160 // let the compositor drop the overlay before grim runs
        property var screen
        property rect rect
        onTriggered: {
            ScreenTools.captured(root.mode, screen, rect);
            root.hiding = false;
        }
    }

    LazyLoader {
        active: root.mode !== ""

        PanelWindow {
            id: win
            screen: ScreenTools.selectingScreen
            visible: !root.hiding
            color: "transparent"
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "irice:region"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

            readonly property var monitor: Hyprland.monitorFor(screen)
            readonly property var windows: Hyprland.toplevels.values.filter(t => t.workspace?.id === monitor?.activeWorkspace?.id && t.lastIpcObject?.at).map(t => Qt.rect(t.lastIpcObject.at[0] - monitor.x, t.lastIpcObject.at[1] - monitor.y, t.lastIpcObject.size[0], t.lastIpcObject.size[1]))
            property point start
            property point end
            property bool dragging: false
            property var hovered: null // window rect under the pointer
            readonly property rect selection: dragging ? Qt.rect(Math.min(start.x, end.x), Math.min(start.y, end.y), Math.abs(end.x - start.x), Math.abs(end.y - start.y)) : (hovered ?? Qt.rect(0, 0, 0, 0))
            readonly property string verb: ({ screenshot: "capture", ocr: "copy text", translate: "translate", record: "record", "record-window": "record", ask: "ask AI about" })[root.mode] ?? ""

            function finish(r) {
                if (r.width < 4 || r.height < 4)
                    return;
                captureDelay.screen = screen;
                captureDelay.rect = r;
                root.hiding = true;
                captureDelay.start();
            }

            // Dim everything except the selection
            Item {
                anchors.fill: parent
                readonly property rect s: win.selection
                Rectangle { x: 0; y: 0; width: parent.width; height: parent.s.y; color: Qt.rgba(0, 0, 0, 0.35) }
                Rectangle { x: 0; y: parent.s.y + parent.s.height; width: parent.width; height: parent.height - y; color: Qt.rgba(0, 0, 0, 0.35) }
                Rectangle { x: 0; y: parent.s.y; width: parent.s.x; height: parent.s.height; color: Qt.rgba(0, 0, 0, 0.35) }
                Rectangle { x: parent.s.x + parent.s.width; y: parent.s.y; width: parent.width - x; height: parent.s.height; color: Qt.rgba(0, 0, 0, 0.35) }
                Rectangle {
                    x: parent.s.x - 1
                    y: parent.s.y - 1
                    width: parent.s.width + 2
                    height: parent.s.height + 2
                    visible: parent.s.width > 0
                    color: "transparent"
                    border.width: 1
                    border.color: "white"
                    radius: win.dragging ? 0 : 12
                }
            }

            // Size readout
            Rectangle {
                visible: win.selection.width > 0
                x: Math.min(win.selection.x + win.selection.width + 8, win.width - width - 8)
                y: Math.min(win.selection.y + win.selection.height + 8, win.height - height - 8)
                width: sizeText.implicitWidth + 16
                height: 22
                radius: 11
                color: Qt.rgba(0.1, 0.1, 0.12, 0.85)
                Text {
                    id: sizeText
                    anchors.centerIn: parent
                    text: Math.round(win.selection.width) + " × " + Math.round(win.selection.height)
                    color: "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                }
            }

            // Hint
            Rectangle {
                anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 48 }
                width: hint.implicitWidth + 28
                height: 32
                radius: 16
                color: Qt.rgba(0.1, 0.1, 0.12, 0.85)
                Text {
                    id: hint
                    anchors.centerIn: parent
                    text: root.mode === "record-window" ? "Click a window to record it. Esc to cancel." : "Drag to select an area to " + win.verb + ", or click a window. Esc to cancel."
                    color: "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
            }

            Item {
                anchors.fill: parent
                focus: true
                Keys.onEscapePressed: ScreenTools.cancelSelect()
                Keys.onReturnPressed: win.finish(win.selection.width > 0 ? Qt.rect(win.selection.x, win.selection.y, win.selection.width, win.selection.height) : Qt.rect(0, 0, win.width, win.height))
            }

            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.CrossCursor
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onPositionChanged: m => {
                    if (win.dragging) {
                        win.end = Qt.point(m.x, m.y);
                    } else {
                        win.hovered = win.windows.find(r => m.x >= r.x && m.x <= r.x + r.width && m.y >= r.y && m.y <= r.y + r.height) ?? null;
                    }
                }
                onPressed: m => {
                    if (m.button === Qt.RightButton) {
                        ScreenTools.cancelSelect();
                        return;
                    }
                    win.start = Qt.point(m.x, m.y);
                    win.end = win.start;
                    win.dragging = true;
                }
                onReleased: m => {
                    if (!win.dragging)
                        return;
                    const moved = Math.abs(win.end.x - win.start.x) > 4 || Math.abs(win.end.y - win.start.y) > 4;
                    // A copy: a rect read from a property stays tied to it,
                    // and the selection empties once dragging ends
                    const s = moved ? win.selection : win.hovered;
                    const r = s ? Qt.rect(s.x, s.y, s.width, s.height) : null;
                    win.dragging = false;
                    if (r)
                        win.finish(r);
                }
            }
        }
    }
}
