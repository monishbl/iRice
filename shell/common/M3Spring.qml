import QtQuick

// One of Material 3 Expressive's springs (Theme.motion's springFast,
// springDefault or springSlow), for a file both looks share. Glass's Theme
// has no such springs, so there it falls back to Glass's own spring (the
// Behavior holding it is normally off in Glass anyway).
//   Behavior on radius { enabled: Theme.material; M3Spring { speed: "fast"; epsilon: 0.25 } }
SpringAnimation {
    property string speed: "default" // "fast", "default" or "slow"
    readonly property var m3: Theme.material ? (speed === "fast" ? Theme.motion.springFast : speed === "slow" ? Theme.motion.springSlow : Theme.motion.springDefault) : null
    spring: m3 ? m3.spring : Theme.motion.spring
    damping: m3 ? m3.damping : Theme.motion.damping
}
