import QtQuick

// A SpringAnimation on one of the Material look's springs (Theme.motion's
// springFast, springDefault, springSlow, springPress) for a file both looks
// share: Glass's Theme has none of them, so there it's a plain spring that
// shouldn't run (its Behavior is enabled only in Material).
//   Behavior on radius { enabled: Theme.material; MaterialSpring { speed: "fast"; epsilon: 0.1 } }
SpringAnimation {
    property string speed: "default" // fast | default | slow | press
    readonly property var preset: !Theme.material ? null : speed === "fast" ? Theme.motion.springFast : speed === "slow" ? Theme.motion.springSlow : speed === "press" ? Theme.motion.springPress : Theme.motion.springDefault
    spring: preset ? preset.spring : 1
    damping: preset ? preset.damping : 1
}
