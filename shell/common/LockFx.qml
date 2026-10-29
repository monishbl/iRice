pragma Singleton
import QtQuick
import Quickshell

// The desktop's half of the lock transitions (Settings → Lock Screen), like
// iPhone's: unlocking, the wallpaper settles from a little larger while the
// desktop widgets and icons zoom in from the centre of the screen, the outer
// ones a moment later; windows, the dock and the menu bar fade in (a veil
// showing the wallpaper lifts off them, Lock.qml). Locking happens at once;
// the lock screen plays its own entrance (LockSurface).
//
// `desktop` runs 0 (set back: zoomed, hidden) .. 1 (at rest), linearly; each
// piece eases its own share of it:
//   LockFx.wallpaperScale                      the wallpaper (and the veil)
//   LockFx.itemScale(cx, cy, w, h), itemOpacity(…)   a widget or icon centred at cx, cy
//   LockFx.veil                                how much the veil covers
Singleton {
    id: root

    property real desktop: 1
    property bool zoom: true // zoom (wallpaper and items move) or fade only
    readonly property bool active: desktop < 1

    readonly property real wallpaperScale: zoom ? 1 + 0.05 * (1 - ease(desktop)) : 1
    readonly property real veil: 1 - ease(Math.min(1, desktop / 0.8))

    function ease(t) { // ease-out cubic
        const c = Math.max(0, Math.min(1, t));
        return 1 - Math.pow(1 - c, 3);
    }
    // An item's own progress: items further from the centre start later
    function progress(cx, cy, w, h) {
        if (w <= 0 || h <= 0)
            return ease(desktop);
        const d = Math.min(1, Math.hypot((cx - w / 2) / (w / 2), (cy - h / 2) / (h / 2)) / Math.SQRT2);
        const delay = 0.3 * d;
        return ease((desktop - delay) / (1 - 0.3));
    }
    function itemScale(cx, cy, w, h) {
        return zoom ? 1 + 0.22 * (1 - progress(cx, cy, w, h)) : 1;
    }
    function itemOpacity(cx, cy, w, h) {
        return progress(cx, cy, w, h);
    }

    // Unlock: set back at once (still hidden by the lock), then settle
    function prepare(withZoom) {
        anim.stop();
        zoom = withZoom;
        desktop = 0;
    }
    function reveal(ms) {
        anim.stop();
        anim.to = 1;
        anim.duration = ms;
        anim.start();
    }
    function rest() {
        anim.stop();
        desktop = 1;
    }

    NumberAnimation {
        id: anim
        target: root
        property: "desktop"
        easing.type: Easing.Linear // each piece eases its own share
    }
}
