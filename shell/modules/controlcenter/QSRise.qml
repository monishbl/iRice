import QtQuick
import qs.common

// How far a piece of Quick Settings has risen into place as the panel
// opens: 0 → 1 on Material's fast spatial spring (it overshoots a little),
// its turn coming `order` steps after the panel starts to open, so the
// sliders, tiles and media card arrive one after another.
//   QSRise { id: rise; order: index; stagger: qs.stagger }
//   scale: 0.86 + 0.14 * rise.value; opacity: Math.min(1, rise.value)
QtObject {
    id: root

    property real stagger: 1 // 0 → 1 while the panel opens (QuickSettings)
    property int order: 0
    readonly property bool due: stagger >= 1 || stagger * 24 >= order + 1
    property real value: 0
    // (made after the panel opened, a tile just added: already in place)
    Component.onCompleted: if (due) value = 1
    onDueChanged: {
        if (due) {
            anim.restart();
        } else {
            anim.stop();
            value = 0;
        }
    }
    property SpringAnimation anim: SpringAnimation {
        target: root
        property: "value"
        to: 1
        spring: Theme.motion.springFast.spring
        damping: Theme.motion.springFast.damping
        epsilon: 0.004
    }
}
