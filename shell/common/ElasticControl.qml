import QtQuick

// Elastic feel for a small control, on press: it squishes, and while held it
// follows the pointer a little (rubber-banded), springing back on release.
// Nothing happens on hover. Drop it inside the control:
//   ElasticControl { id: elastic }  and  transform: elastic.effects
//   (optionally set `pressed` from the control's MouseArea)
// Material (3 Expressive): no rubber band (Material doesn't pull); it swells
// a little (~6%, ~10 px at most for big ones) while held and springs back
// past its size on release, settling with a small bounce; the control
// squares its corners off itself (`held`, or `morph` 0 → 1 on the same
// spring).
Item {
    id: root

    anchors.fill: parent
    property bool pressed: false
    readonly property var effects: [translate, scale]

    // Passive: sees presses without taking them from the control's MouseArea
    PointHandler { id: point; acceptedButtons: Qt.LeftButton }
    readonly property bool held: point.active || pressed
    // (Glass only: Material's pull is 0, and the rubber band divides by it)
    readonly property bool follows: !Theme.material
    readonly property real limit: Theme.motion.controlPull * 1.6
    function rubber(d) {
        return Math.sign(d) * limit * (1 - Math.exp(-Math.abs(d) / (limit * 5)));
    }
    property real px: follows && point.active ? rubber(point.point.scenePosition.x - point.point.scenePressPosition.x) : 0
    property real py: follows && point.active ? rubber(point.point.scenePosition.y - point.point.scenePressPosition.y) : 0
    property real s: Theme.material ? (held && enabled ? 1 + Theme.motion.pressGrow(Math.max(width, height)) : 1) : held ? Theme.motion.squish : 1
    Behavior on px { enabled: root.follows; SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }
    Behavior on py { enabled: root.follows; SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }
    Behavior on s {
        SpringAnimation {
            spring: Theme.material ? Theme.motion.springPress.spring : 6
            damping: Theme.material ? Theme.motion.springPress.damping : Theme.motion.damping + 0.05
            epsilon: Theme.material ? 0.0005 : 0.002
        }
    }
    // Material: 0 → 1 while held, on the press spring: for corners that square off
    property real morph: held ? 1 : 0
    Behavior on morph { enabled: Theme.material; SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.002 } }

    Translate { id: translate; x: root.px; y: root.py }
    Scale { id: scale; origin.x: root.width / 2; origin.y: root.height / 2; xScale: root.s; yScale: root.s }
}
