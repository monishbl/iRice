import QtQuick
import qs.common
import qs.services

// Android 17's media carousel: the current player's card (`card`, as wide as
// there's room for), and the other players with something loaded beside it
// as narrow cards (MediaPeek); the current card shrinks to make room. Swipe
// the card sideways, or click a narrow one, to switch.
//   MediaCarousel { width: 368; height: 136; card: Component { QSMedia {} } }
Item {
    id: root

    property Component card
    property real gap: 8
    property real peekRadius: 20
    // (the newest two that have a track; the current one isn't among them)
    readonly property var others: Media.players.filter(p => p !== Media.player && (p.trackTitle ?? "") !== "").slice(-2)
    readonly property real peeksWidth: others.length * (40 + gap)

    Loader {
        id: current
        height: parent.height
        width: root.width - root.peeksWidth
        Behavior on width {
            SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.5 }
        }
        sourceComponent: root.card
        // A swipe on it: left brings the next player, right the one before
        DragHandler {
            target: null
            xAxis.enabled: true
            yAxis.enabled: false
            enabled: root.others.length > 0
            onActiveChanged: {
                if (active)
                    return;
                const dx = centroid.position.x - centroid.pressPosition.x;
                if (Math.abs(dx) < 48)
                    return;
                Media.pinned = dx < 0 ? root.others[0] : root.others[root.others.length - 1];
            }
        }
    }
    Row {
        anchors.right: parent.right
        height: parent.height
        spacing: root.gap
        Repeater {
            model: root.others
            MediaPeek {
                required property var modelData
                player: modelData
                width: 40
                height: root.height
                radius: root.peekRadius
            }
        }
    }
}
