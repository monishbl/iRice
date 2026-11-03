import QtQuick

// A control's press in the Material look (Material 3 Expressive): it swells
// a little (~6%, ~10 px at most for big ones) while held and springs back
// past its size on release, settling with a small bounce; the control
// squares its corners off itself (`held`, or `morph` 0 → 1 on the same
// spring). No rubber-band follow: Material doesn't pull. The same interface
// as the Glass one (common/ElasticControl.qml), whose rubber band divides by
// the pull: at 0 a pressed control's position became NaN.
//   ElasticControl { id: elastic; pressed: mouse.pressed }  and  transform: elastic.effects
Item {
    id: root

    anchors.fill: parent
    property bool pressed: false
    readonly property var effects: [translate, scale]

    PointHandler { id: point; acceptedButtons: Qt.LeftButton }
    readonly property bool held: point.active || pressed
    readonly property real px: 0
    readonly property real py: 0
    property real s: held && enabled ? 1 + Theme.motion.pressGrow(Math.max(width, height)) : 1
    Behavior on s { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.0005 } }
    // 0 → 1 while held, on the same spring: for corners that square off
    property real morph: held ? 1 : 0
    Behavior on morph { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.002 } }

    Translate { id: translate }
    Scale { id: scale; origin.x: root.width / 2; origin.y: root.height / 2; xScale: root.s; yScale: root.s }
}
