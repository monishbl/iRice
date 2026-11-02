import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services

// Live captions on screen (services/Captions), on the display you're using:
//   bar    a floating bar above the Dock; drag it anywhere (the place is kept)
//   strip  a strip across the bottom that windows make room for
//   video  over the bottom of the window that's playing (else the focused
//          one), up to three lines, like a film's subtitles; clicks go
//          through to the window
// (In the island, Island.activities shows them instead.) White on dark, like
// TV subtitles, whatever the theme: it has to read over any picture. It
// fades after a while with nothing said.
Scope {
    id: root

    readonly property bool shown: Captions.enabled && Captions.placement !== "island"
    readonly property var screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
    // What to say: the words, else why there are none
    readonly property string latest: !Captions.ready ? "Live captions need setting up: Settings → Accessibility"
        : Captions.error !== "" ? Captions.error
        : Captions.latest !== "" ? Captions.latest : "Captions appear here when something speaks"
    readonly property string previous: Captions.ready && Captions.error === "" ? Captions.previous : ""
    readonly property bool hint: !Captions.ready || Captions.error !== "" || Captions.latest === ""
    // (quiet for a while: faded, until someone speaks)
    readonly property bool dim: Captions.ready && Captions.error === "" && Captions.quiet && Captions.partial === ""

    component Words: Column {
        id: words
        property int size: 20
        spacing: 2
        Text {
            width: parent.width
            visible: text !== ""
            text: root.previous
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideLeft
            maximumLineCount: 1
            color: Qt.rgba(1, 1, 1, 0.6)
            font { family: Theme.font.family; pixelSize: words.size; weight: Font.Medium }
        }
        Text {
            width: parent.width
            text: root.latest
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideLeft
            color: root.hint ? Qt.rgba(1, 1, 1, 0.6) : "white"
            font { family: Theme.font.family; pixelSize: words.size; weight: Font.Medium }
            Accessible.role: Accessible.StaticText
            Accessible.name: "Live captions: " + text
        }
    }

    // ── The floating bar ────────────────────────────────────────────────
    LazyLoader {
        active: root.shown && Captions.placement === "bar"
        PanelWindow {
            id: bar
            screen: root.screen
            readonly property int w: Math.min(760, (root.screen?.width ?? 1280) - 64)
            implicitWidth: w
            implicitHeight: body.implicitHeight + 28
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:captions"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            anchors { left: true; bottom: true }
            // Its place (kept), or centred above the Dock
            readonly property real cx: Captions.cfg.barX >= 0 ? Captions.cfg.barX : ((root.screen?.width ?? 1280) - w) / 2
            readonly property real cy: Captions.cfg.barY >= 0 ? Captions.cfg.barY : 112
            margins { left: Math.max(0, Math.min(cx + drag.dx, (root.screen?.width ?? 1280) - w)); bottom: Math.max(0, Math.min(cy - drag.dy, (root.screen?.height ?? 800) - implicitHeight)) }

            // (Material: the same dark box, which has to read over any
            // picture, at Material's extra-large corner and without the
            // hairline, as Android's Live Caption box)
            Rectangle {
                anchors.fill: parent
                radius: Theme.material ? 24 : 14
                color: Qt.rgba(0.07, 0.07, 0.08, 0.82)
                border.width: Theme.material ? 0 : 1
                border.color: Qt.rgba(1, 1, 1, 0.08)
                opacity: root.dim ? 0.35 : 1
                Behavior on opacity { NumberAnimation { duration: 500; easing.type: Easing.OutCubic } }

                Words {
                    id: body
                    anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 20 }
                }
                // Drag to move; the place is kept when you let go
                MouseArea {
                    id: drag
                    anchors.fill: parent
                    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                    hoverEnabled: true
                    property real sx: 0
                    property real sy: 0
                    property real dx: 0
                    property real dy: 0
                    onPressed: e => {
                        const g = mapToGlobal(e.x, e.y);
                        sx = g.x;
                        sy = g.y;
                    }
                    onPositionChanged: e => {
                        if (!pressed)
                            return;
                        const g = mapToGlobal(e.x, e.y);
                        dx = g.x - sx;
                        dy = g.y - sy;
                    }
                    onReleased: {
                        Captions.cfg.barX = bar.margins.left;
                        Captions.cfg.barY = bar.margins.bottom;
                        dx = 0;
                        dy = 0;
                    }
                }
                // Off, from the bar itself (shown while the pointer is over it)
                IconButton {
                    anchors { right: parent.right; top: parent.top; margins: 6 }
                    visible: drag.containsMouse
                    icon: "x"
                    iconColor: Qt.rgba(1, 1, 1, 0.7)
                    tooltip: "Turn off live captions"
                    onClicked: Captions.cfg.enabled = false
                }
            }
        }
    }

    // ── The strip across the bottom ─────────────────────────────────────
    LazyLoader {
        active: root.shown && Captions.placement === "strip"
        PanelWindow {
            screen: root.screen
            implicitHeight: 76
            color: Qt.rgba(0.07, 0.07, 0.08, 0.92)
            anchors { left: true; right: true; bottom: true }
            exclusiveZone: implicitHeight // (windows make room for it)
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.namespace: "irice:captions"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            Words {
                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 64; rightMargin: 64 }
                size: 20
                opacity: root.dim ? 0.5 : 1
            }
            IconButton {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter; rightMargin: 16 }
                icon: "x"
                iconColor: Qt.rgba(1, 1, 1, 0.7)
                tooltip: "Turn off live captions"
                onClicked: Captions.cfg.enabled = false
            }
        }
    }

    // ── Over the video ──────────────────────────────────────────────────
    // The window: the playing app's on the space you're looking at, else the
    // focused one; the whole display without either. Its place in the
    // display's coordinates.
    readonly property var videoRect: {
        Desktop.revision; // (windows moved)
        const mon = Hyprland.monitorFor(root.screen);
        const full = { x: 0, y: 0, w: root.screen?.width ?? 1280, h: root.screen?.height ?? 800 };
        if (!mon)
            return full;
        const here = Windows.on(mon.activeWorkspace?.id).filter(t => t.lastIpcObject?.size && !t.lastIpcObject?.hidden);
        const p = Island.player;
        const want = String(p?.desktopEntry || p?.identity || "").toLowerCase();
        const t = (want !== "" ? here.find(w => String(w.lastIpcObject?.class ?? "").toLowerCase().includes(want.split(/[ .]/)[0])) : null)
            ?? here.find(w => w === Apps.activeHyprland) ?? null;
        if (!t)
            return full;
        const [x, y] = t.lastIpcObject.at;
        const [w, h] = t.lastIpcObject.size;
        return { x: x - mon.x, y: y - mon.y, w: w, h: h };
    }
    LazyLoader {
        active: root.shown && Captions.placement === "video"
        PanelWindow {
            id: over
            screen: root.screen
            readonly property var r: root.videoRect
            // (bigger on a big window, like a film's subtitles)
            readonly property int size: Math.round(Math.max(18, Math.min(28, r.w / 42)))
            implicitWidth: Math.max(240, Math.min(r.w - 48, 64 * size))
            implicitHeight: box.height
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:captions"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            anchors { left: true; top: true }
            margins {
                left: Math.max(0, Math.round(r.x + (r.w - implicitWidth) / 2))
                top: Math.max(0, Math.round(r.y + r.h - implicitHeight - Math.max(24, r.h * 0.08)))
            }
            mask: Region {}

            // The words on a dark box just their size, centred
            Rectangle {
                id: box
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(parent.width, lines.contentW + 32)
                height: lines.implicitHeight + 16
                radius: Theme.material ? 12 : 10
                color: Qt.rgba(0, 0, 0, 0.62)
                opacity: root.dim ? 0 : 1
                Behavior on opacity { NumberAnimation { duration: 500; easing.type: Easing.OutCubic } }
                CaptionLines {
                    id: lines
                    // (the text's own width, so a short line gets a short box)
                    readonly property real contentW: Math.min(over.width - 32, measure.implicitWidth)
                    anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 16; rightMargin: 16 }
                    size: over.size
                    lines: 3
                    centred: true
                }
                Text {
                    id: measure
                    visible: false
                    text: (Captions.previous !== "" ? Captions.previous + " " : "") + (lines.hint || Captions.latest)
                    font { family: Theme.font.family; pixelSize: over.size; weight: Font.Medium }
                }
            }
        }
    }
}
