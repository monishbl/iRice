import QtQuick
import Quickshell.Widgets
import qs.common
import qs.services

// Material's carousel for the players (Quick Settings): every player with
// something loaded keeps its own place in a row; the current one is wide,
// the others narrow slices showing their art (MediaPeek). Choosing one
// (a click on its slice, or a swipe on the row) doesn't move anything: it
// widens where it is while the current one narrows, each card's content
// revealed or covered by its own rounded clip, as Material 3's carousel.
// Too narrow for slices (a 2×2 card): only the current one; a swipe still
// switches.
//   MediaCarousel { width: 368; height: 136; card: Component { QSMedia {} } }
// (the card gets `player`, the one it shows)
Item {
    id: root

    property Component card
    property real gap: 8
    property real peekRadius: 20
    property real peekWidth: 40

    // In a steady order (the players' own), at most three: the current one
    // and the first two others with a track
    readonly property var list: {
        const all = Media.players.filter(p => p === Media.player || (p.trackTitle ?? "") !== "");
        const others = all.filter(p => p !== Media.player).slice(0, 2);
        const keep = all.filter(p => p === Media.player || others.includes(p));
        return keep.length > 0 ? keep : [Media.player];
    }
    readonly property bool room: width >= 300
    readonly property var shown: room ? list : [Media.player]
    readonly property real full: width - (shown.length - 1) * (peekWidth + gap)

    function step(d) {
        const i = list.indexOf(Media.player);
        const n = list[Math.max(0, Math.min(list.length - 1, i + d))];
        if (n && n !== Media.player)
            Media.pinned = n;
    }

    Row {
        height: parent.height
        spacing: root.gap
        Repeater {
            model: root.shown
            Item {
                id: slot
                required property var modelData
                readonly property bool current: modelData === Media.player
                width: current ? root.full : root.peekWidth
                height: root.height
                Behavior on width {
                    SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.3 }
                }
                // 0 a slice … 1 the full card
                readonly property real t: root.full > root.peekWidth ? Math.max(0, Math.min(1, (width - root.peekWidth) / (root.full - root.peekWidth))) : 1

                ClippingRectangle {
                    anchors.fill: parent
                    radius: Math.min(root.peekRadius, width / 2)
                    color: "transparent"
                    // The card at its full width, uncovered as the slot widens
                    Loader {
                        width: root.full
                        height: slot.height
                        opacity: Math.min(1, slot.t * 1.6)
                        visible: opacity > 0.01
                        enabled: slot.current
                        sourceComponent: root.card
                        onLoaded: item.player = Qt.binding(() => slot.modelData)
                    }
                }
                MediaPeek {
                    anchors.fill: parent
                    player: slot.modelData
                    radius: root.peekRadius
                    opacity: 1 - Math.min(1, slot.t * 2.5)
                    visible: opacity > 0.01
                    enabled: !slot.current
                }
            }
        }
    }

    // A swipe on the row: left brings the next player, right the one before
    DragHandler {
        target: null
        xAxis.enabled: true
        yAxis.enabled: false
        enabled: root.list.length > 1
        onActiveChanged: {
            if (active)
                return;
            const dx = centroid.position.x - centroid.pressPosition.x;
            if (Math.abs(dx) >= 48)
                root.step(dx < 0 ? 1 : -1);
        }
    }
}
