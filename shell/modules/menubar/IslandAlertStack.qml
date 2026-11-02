import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Alerts that arrive while the island is open (hovered or pinned): the island
// stays as it is, and they drop in as a black-and-glass card just under it,
// like a notification under the iPhone's island. Newest on top; up to two
// older ones peek out behind it, narrower, like iOS's stacks (the rest wait
// their turn, and Recent keeps them all). The pointer on the card holds
// their time; swipe one up, or its close button, to put it away.
// (MenuBar shows it while IslandView.alertsBelow; `top` is the island's
// bottom edge, in the bar window's coordinates, which are the screen's.)
Scope {
    id: root

    required property var screen
    property bool active: false
    property real top: 40
    property bool unseen: false

    // The alerts, newest first (Island.alerts is newest last)
    readonly property var newest: {
        Island.byUid;
        const out = [];
        for (let i = Island.alerts.count - 1; i >= 0 && out.length < 3; i--)
            out.push(Island.alerts.get(i).uid);
        return out;
    }

    LazyLoader {
        active: root.active && root.newest.length > 0

        GlassWindow {
            id: win
            name: "island-alerts"
            screen: root.screen
            anchors.top: true
            implicitWidth: 392 + 2 * glass.shadowMargin
            // (room for the island at its tallest and the stack under it: the
            // card moves inside the window as the island springs, rather than
            // the window moving)
            implicitHeight: 640
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:island-alerts"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            mask: Region { item: stack }

            property real shown: 0
            Component.onCompleted: shown = 1
            Behavior on shown { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }

            HoverHandler {
                id: hover
                onHoveredChanged: Island.hold("below:" + (root.screen?.name ?? ""), hovered)
            }
            Component.onDestruction: Island.hold("below:" + (root.screen?.name ?? ""), false)

            Item {
                id: stack
                x: glass.shadowMargin
                // (slides down 8px as it appears)
                y: root.top + 10 - (1 - win.shown) * 8
                width: 392
                height: front.height + 7 * Math.max(0, Math.min(2, root.newest.length - 1))
                opacity: win.shown
                onYChanged: glass.updatePosition()
                onHeightChanged: glass.updatePosition()

                // The older ones, peeking out under the newest: only their
                // rounded lower edges, 7px each (the front card is partly
                // see-through, so they mustn't reach behind it)
                Repeater {
                    model: Math.max(0, Math.min(2, root.newest.length - 1))
                    Item {
                        required property int index
                        readonly property int depth: index + 1
                        z: -depth
                        x: depth * 12
                        y: front.height
                        width: stack.width - 2 * x
                        height: 7 * depth
                        clip: true
                        Rectangle {
                            y: parent.height - height
                            width: parent.width
                            height: 44
                            radius: Theme.radius.tile
                            color: Qt.rgba(0.1, 0.1, 0.11, 0.92 - parent.depth * 0.18)
                            border.width: 1
                            border.color: Qt.rgba(1, 1, 1, 0.1)
                        }
                    }
                }

                // The newest: the island's own look, black melting into glass
                Item {
                    id: front
                    width: stack.width
                    height: alert.implicitHeight + 32
                    Behavior on height { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }

                    Glass {
                        id: glass
                        anchors.fill: parent
                        anchors.margins: -shadowMargin
                        shadowMargin: 16
                        radius: Theme.radius.tile
                    }
                    Rectangle {
                        anchors.fill: parent
                        radius: Theme.radius.tile
                        gradient: Gradient {
                            GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.92) }
                            GradientStop { position: 1; color: Qt.rgba(0, 0, 0, 0.55) }
                        }
                        border.width: 1
                        border.color: Qt.rgba(1, 1, 1, 0.14)
                    }
                    IslandEvent {
                        id: alert
                        x: 18
                        y: 16
                        width: parent.width - 36
                        event: Island.byUid[root.newest[0]] ?? null
                        unseen: root.unseen
                        onActed: (uid, since) => Island.remove(uid, since)
                        onDismissRequested: Island.remove(root.newest[0])
                    }
                    // How many are waiting behind it
                    Text {
                        visible: Island.alerts.count > 1
                        anchors { right: parent.right; bottom: parent.bottom; rightMargin: 14; bottomMargin: 6 }
                        text: "+" + (Island.alerts.count - 1)
                        color: Qt.rgba(1, 1, 1, 0.5)
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }
}
