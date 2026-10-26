import QtQuick

// Elastic pull for a panel, on press and drag: while the button is held, the
// target follows the drag like rubber (resisting more the further it goes,
// up to Theme.motion.pull px) and stretches a little that way; on release it
// springs back. Nothing happens on hover. Positions are window coordinates.
//   ElasticPull { id: pull; target: body; active: dragging; from: pressPoint; pointer: currentPoint }
//   body.transform: [pull.translate, pull.stretch]
QtObject {
    id: root

    property Item target
    property bool active: false
    property point from: Qt.point(0, 0) // where the press started
    property point pointer: Qt.point(0, 0) // where it is now
    property real strength: Theme.motion.pull * 1.6

    // Rubber band: grows with the drag but never past `strength`
    function rubber(d) {
        return Math.sign(d) * strength * (1 - Math.exp(-Math.abs(d) / (strength * 6)));
    }
    readonly property point lean: active && target ? Qt.point(rubber(pointer.x - from.x), rubber(pointer.y - from.y)) : Qt.point(0, 0)

    property real lx: lean.x
    property real ly: lean.y
    Behavior on lx { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }
    Behavior on ly { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }

    readonly property Translate translate: Translate { x: root.lx; y: root.ly }
    // Stretch along the pull, anchored on the side away from it
    readonly property Scale stretch: Scale {
        origin.x: root.target ? root.target.width / 2 - Math.sign(root.lx) * root.target.width / 2 : 0
        origin.y: root.target ? root.target.height / 2 - Math.sign(root.ly) * root.target.height / 2 : 0
        xScale: 1 + Math.abs(root.lx) / Math.max(1, root.target?.width ?? 1) * 0.9
        yScale: 1 + Math.abs(root.ly) / Math.max(1, root.target?.height ?? 1) * 0.9
    }
}
