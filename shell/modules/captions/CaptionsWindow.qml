import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services

// Live captions on screen (services/Captions), on the display you're using:
//   bar    a floating bar above the Dock; drag it anywhere (the place is kept)
//   strip  a strip across the bottom that windows make room for
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

            Rectangle {
                anchors.fill: parent
                radius: 14
                color: Qt.rgba(0.07, 0.07, 0.08, 0.82)
                border.width: 1
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
}
