import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services
import "../../common/ScreenGeometry.js" as ScreenGeometry

// Region picker: dims the screen, drag to select (live size readout), or
// click a highlighted window to take all of it. Space toggles window mode
// off; Esc cancels. Hides itself before the capture happens.
// Circle to search (mode "circle"): circle or scribble round anything (its
// bounding box, padded a little, is taken; the stroke shows as it's drawn),
// or drag a box, which a roughly straight drag is (ScreenGeometry.strokeKind).
Scope {
    id: root

    readonly property string mode: ScreenTools.selecting
    property bool hiding: false
    onModeChanged: {
        if (mode === "") {
            captureDelay.stop();
            hiding = false;
        }
    }

    component Stroke: Shape {
        id: stroke
        property var points: []
        property real weight: 5
        property color color: Theme.accent
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: stroke.color
            strokeWidth: stroke.weight
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathPolyline { path: stroke.points }
        }
    }

    Timer {
        id: captureDelay
        interval: 160 // let the compositor drop the overlay before grim runs
        property var screen
        property rect rect
        property string captureMode
        onTriggered: {
            ScreenTools.captured(captureMode, screen, rect);
            root.hiding = false;
        }
    }

    // (tessera-ui draws the screen tools: it tells and asks the shell here)
    RustBridge {}

    LazyLoader {
        active: root.mode !== "" && !RustUi.drawing("screentools")

        PanelWindow {
            id: win
            screen: ScreenTools.selectingScreen
            visible: !root.hiding
            color: "transparent"
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "tessera:region"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

            readonly property var monitor: Hyprland.monitorFor(screen)
            readonly property var windows: Hyprland.toplevels.values.filter(t => t.workspace?.id === monitor?.activeWorkspace?.id && t.lastIpcObject?.at).map(t => Qt.rect(t.lastIpcObject.at[0] - monitor.x, t.lastIpcObject.at[1] - monitor.y, t.lastIpcObject.size[0], t.lastIpcObject.size[1]))
            property point start
            property point end
            property bool dragging: false
            property var hovered: null // window rect under the pointer
            // Circle to search: the path drawn so far, and what it is
            readonly property bool circling: root.mode === "circle"
            property var stroke: []
            readonly property string kind: circling && dragging ? ScreenGeometry.strokeKind(stroke) : "box"
            readonly property rect selection: {
                const r = ScreenGeometry.clip(dragging ? (circling ? ScreenGeometry.strokeBox(stroke, 12, width, height) : {x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y)}) : (hovered ?? Qt.rect(0, 0, 0, 0)), width, height);
                return Qt.rect(r.x, r.y, r.width, r.height);
            }
            readonly property string verb: ({ screenshot: "capture", ocr: "copy text", translate: "translate", record: "record", "record-window": "record", ask: "ask AI about" })[root.mode] ?? ""

            function finish(r) {
                r = ScreenGeometry.clip(r, width, height);
                if (r.width < 4 || r.height < 4)
                    return;
                captureDelay.screen = screen;
                captureDelay.rect = Qt.rect(r.x, r.y, r.width, r.height);
                captureDelay.captureMode = root.mode;
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
                Squircle {
                    x: parent.s.x - 1
                    y: parent.s.y - 1
                    width: parent.s.width + 2
                    height: parent.s.height + 2
                    visible: parent.s.width > 0 && win.kind === "box" // (a circle shows only its stroke)
                    color: "transparent"
                    // (Material: a 2 px primary outline)
                    border.width: Theme.material ? 2 : 1
                    border.color: Theme.material ? Theme.m3.primary : "white"
                    radius: win.dragging ? 0 : Theme.material ? 16 : 12
                }
            }

            // Circle to search: the stroke, in the accent with a soft light
            // glow that keeps it apart from a background of the same colour
            // (drawn whole, then faded, so it doesn't build up where the
            // stroke crosses itself). It shows from the first pixel, not only
            // once the drag reads as a circle, so the pen never lags the hand
            Item {
                anchors.fill: parent
                visible: win.circling && win.dragging
                opacity: 0.45
                layer.enabled: visible
                Stroke { points: win.stroke; weight: 14; color: "white" }
            }
            Stroke {
                visible: win.circling && win.dragging
                points: win.stroke
                weight: 5
            }

            // Size readout
            Rectangle {
                visible: win.selection.width > 0 && !win.circling
                x: Math.min(win.selection.x + win.selection.width + 8, win.width - width - 8)
                y: Math.min(win.selection.y + win.selection.height + 8, win.height - height - 8)
                width: sizeText.implicitWidth + 16
                height: Theme.material ? 28 : 22
                radius: height / 2
                // (Material: a primary badge)
                color: Theme.material ? Theme.m3.primary : Qt.rgba(0.1, 0.1, 0.12, 0.85)
                Text {
                    id: sizeText
                    anchors.centerIn: parent
                    text: Math.round(win.selection.width) + " × " + Math.round(win.selection.height)
                    color: Theme.material ? Theme.m3.onPrimary : "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                }
            }

            // Hint
            Rectangle {
                anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 48 }
                width: hint.implicitWidth + (Theme.material ? 40 : 28)
                height: Theme.material ? 48 : 32
                radius: height / 2
                // (Material: a snackbar-like hint in inverseSurface, as a pill)
                color: Theme.material ? Theme.m3.inverseSurface : Qt.rgba(0.1, 0.1, 0.12, 0.85)
                Text {
                    id: hint
                    anchors.centerIn: parent
                    text: root.mode === "record-window" ? "Click a window to record it. Esc to cancel." : win.circling ? "Circle or scribble over anything to search it. Esc to cancel." : "Drag to select an area to " + win.verb + ", or click a window. Esc to cancel."
                    color: Theme.material ? Theme.m3.inverseOnSurface : "white"
                    font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body : Theme.font.small; weight: Font.Medium }
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
                        win.end = Qt.point(Math.max(0, Math.min(win.width, m.x)), Math.max(0, Math.min(win.height, m.y)));
                        // (a point every 2 px is smooth enough; copies: a
                        // point read from a property stays tied to it)
                        const last = win.stroke[win.stroke.length - 1];
                        if (win.circling && Math.hypot(win.end.x - last.x, win.end.y - last.y) >= 2)
                            win.stroke = win.stroke.concat([Qt.point(win.end.x, win.end.y)]);
                    } else {
                        win.hovered = win.windows.find(r => m.x >= r.x && m.x <= r.x + r.width && m.y >= r.y && m.y <= r.y + r.height) ?? null;
                    }
                }
                onPressed: m => {
                    if (m.button === Qt.RightButton) {
                        ScreenTools.cancelSelect();
                        return;
                    }
                    win.start = Qt.point(Math.max(0, Math.min(win.width, m.x)), Math.max(0, Math.min(win.height, m.y)));
                    win.end = win.start;
                    win.stroke = [Qt.point(win.start.x, win.start.y)];
                    win.dragging = true;
                }
                onReleased: m => {
                    if (!win.dragging)
                        return;
                    // (circling: back where it began still counts as drawn)
                    const moved = win.circling ? win.kind !== "none" : Math.abs(win.end.x - win.start.x) > 4 || Math.abs(win.end.y - win.start.y) > 4;
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
