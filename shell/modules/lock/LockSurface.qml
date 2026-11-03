import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// One screen of the lock (Settings → Lock Screen): the wallpaper, the lock
// itself as each look draws it (LockGlass, LockMaterial), StandBy over them,
// and the rules for both: lock and unlock transitions, StandBy and the
// morph into it, what wakes it, where the keyboard goes.
WlSessionLockSurface {
    id: surface

    required property var lockScope
    // Glass host contract (see Glass.qml)
    readonly property var backdrop: backdropItem.texture
    readonly property real screenX: 0
    readonly property real screenY: 0

    readonly property var cfg: Config.options.lock
    readonly property bool ios: cfg.layout !== "macos"
    readonly property string material: cfg.material === "system" ? (Theme.glass.tier === "liquid" ? "glass" : Theme.glass.tier === "frosted" ? "frost" : "solid") : cfg.material
    readonly property bool darkInk: cfg.text === "dark" || (cfg.text === "auto" && Wallpaper.topIsLight)

    color: "black"

    // ── StandBy: charging and left alone on the lock screen ─────────────
    // (shared by every screen: Lock.qml)
    readonly property bool standby: lockScope.standby
    function woke() {
        lockScope.woke();
    }
    // lock.standbyScreens: a screen left out stays dark during StandBy
    readonly property bool standbyHere: cfg.standbyScreens.length === 0 || cfg.standbyScreens.includes(screen?.name ?? "")

    // Lock → StandBy (lock.standbyTransition): `sb` runs 0 → 1. morph: the
    // lock's clock flies into StandBy's spot, thinning and dimming on the
    // way (two copies cross-fade, one per weight); fade: StandBy fades in.
    readonly property string sbMode: cfg.standbyTransition
    property real sb: standby ? 1 : 0
    Behavior on sb {
        enabled: surface.sbMode !== "none"
        NumberAnimation { duration: surface.sbMode === "morph" ? 650 : 400; easing.type: Easing.InOutCubic }
    }
    readonly property bool morphOn: sbMode === "morph" && standbyHere && standbyView.view === 0 && (lockView.item?.morphable ?? true)
    readonly property bool morphing: morphOn && sb > 0 && sb < 1
    function ramp(a, b) { // sb mapped to 0..1 over [a, b], smoothstepped
        const t = Math.max(0, Math.min(1, (sb - a) / (b - a)));
        return t * t * (3 - 2 * t);
    }

    // Unlock (lock.unlockTransition, run by Lock.qml): the content lifts
    // away, growing a little and fading; with zoom the wallpaper leans in
    // too, matching where the desktop starts its settle (LockFx)
    readonly property bool zoomOut: cfg.unlockTransition === "zoom"
    property real exit: lockScope.leaving ? 1 : 0
    Behavior on exit { NumberAnimation { duration: surface.lockScope.leaveMs; easing.type: Easing.OutCubic } }
    // Keep the screen on while StandBy shows (like iPhone on its charger)
    IdleInhibitor {
        window: surface
        enabled: surface.standby
    }

    Backdrop {
        id: backdropItem
        screen: surface.screen
    }

    Image {
        id: wallpaper
        anchors.fill: parent
        // Unlocking it leans in to where the desktop's wallpaper starts
        // (LockFx); after a zoomed lock it settles from there
        scale: 1 + 0.05 * Math.max(surface.zoomOut ? surface.exit : 0, surface.enter)
        source: Wallpaper.source
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(width, height)
        asynchronous: true
    }
    // Frost (Glass): the wallpaper blurred once, shown through the cards
    readonly property Item frost: frosted
    Item {
        id: frosted
        anchors.fill: parent
        visible: false
        layer.enabled: surface.material === "frost" && !Theme.material
        Image {
            anchors.fill: parent
            source: surface.material === "frost" && !Theme.material ? Wallpaper.source : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(parent.width / 4, parent.height / 4)
            asynchronous: true
            layer.enabled: true
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1
                blurMax: 32
                saturation: 0.2
            }
        }
    }

    // Lock (lock.lockTransition): the lock covers the screen at once (Lock.qml
    // never leaves an unlocked moment for an animation), and the transition
    // plays inside it: zoom, the wallpaper settles from a little larger while
    // the clock and cards rise into place; fade, they only fade in; none,
    // everything is there at once
    readonly property string lockFx: cfg.lockTransition
    property real reveal: lockFx === "none" ? 1 : 0
    property real enter: 0
    Component.onCompleted: {
        if (lockFx === "zoom") {
            enter = 1;
            settleIn.start();
        }
        reveal = 1;
    }
    NumberAnimation { id: settleIn; target: surface; property: "enter"; to: 0; duration: 450; easing.type: Easing.OutCubic }
    Behavior on reveal {
        enabled: surface.lockFx !== "none"
        NumberAnimation { duration: Theme.motion.large; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized }
    }

    // Each look draws its own lock over the same wallpaper and under the
    // same StandBy (LockGlass, LockMaterial: Android's); this file keeps
    // every rule about what shows when
    function focusField() {
        lockView.item?.focusField();
    }
    Loader {
        id: lockView
        anchors.fill: parent
        sourceComponent: Theme.material ? materialLock : glassLock
    }
    Component {
        id: glassLock
        LockGlass {
            host: surface
        }
    }
    Component {
        id: materialLock
        LockMaterial {
            host: surface
        }
    }

    // Over everything while it shows; a key or click brings the lock back
    // (arrows, swipes and the wheel change the view instead); moving the
    // mouse does too, past a little jitter (lock.standbyWakeOnMouse)
    StandBy {
        id: standbyView
        anchors.fill: parent
        lockScope: surface.lockScope
        blank: !surface.standbyHere
        visible: opacity > 0.01
        opacity: surface.sb
        clockFade: surface.morphOn && surface.sb < 1 ? 0 : 1
        dateFade: surface.morphOn ? surface.ramp(0.6, 1) : 1
        focus: surface.standby
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Left || e.key === Qt.Key_Right) {
                standbyView.step(e.key === Qt.Key_Right ? 1 : -1);
            } else {
                surface.woke();
                surface.focusField();
            }
            e.accepted = true;
        }
        MouseArea {
            anchors.fill: parent
            enabled: surface.standby
            hoverEnabled: true
            property point from: Qt.point(-1, -1)
            onEnabledChanged: from = Qt.point(-1, -1)
            onPositionChanged: m => {
                if (!surface.cfg.standbyWakeOnMouse || surface.sb < 1)
                    return;
                if (from.x < 0) {
                    from = Qt.point(m.x, m.y);
                } else if (Math.hypot(m.x - from.x, m.y - from.y) > 24) {
                    surface.woke();
                    surface.focusField();
                }
            }
            onClicked: {
                surface.woke();
                surface.focusField();
            }
            onWheel: w => standbyView.step(w.angleDelta.x + w.angleDelta.y < 0 ? 1 : -1)
        }
    }

    // ── The morph's flying clock ────────────────────────────────────────
    // Glass: centre and size go from the lock's clock to StandBy's; the
    // bold copy hands over to one in StandBy's weight and brightness, which
    // ends exactly where (and as) StandBy's own clock is, so nothing pops.
    // Material: the lock's big clock itself flies to StandBy's, thinning
    // through the font's weight axis on the way (as a Pixel's clock does
    // going always-on) and dimming to StandBy's colour and brightness.
    readonly property point flyTo: Qt.point(standbyView.clockTarget.x + standbyView.clockTarget.width / 2, standbyView.clockTarget.y + standbyView.clockTarget.height / 2)
    Loader {
        anchors.fill: parent
        active: !Theme.material && lockView.item !== null
        sourceComponent: Item {
            readonly property Item lockClock: lockView.item.bigClock
            readonly property point flyFrom: lockView.item.clockCentre
            readonly property real flyX: flyFrom.x + (surface.flyTo.x - flyFrom.x) * surface.sb
            readonly property real flyY: flyFrom.y + (surface.flyTo.y - flyFrom.y) * surface.sb
            readonly property real sizeRatio: standbyView.clockSize / lockClock.font.pixelSize
            Text {
                visible: surface.morphing
                x: parent.flyX - width / 2
                y: parent.flyY - height / 2
                scale: 1 + (parent.sizeRatio - 1) * surface.sb
                opacity: 1 - surface.ramp(0.3, 0.8)
                text: parent.lockClock.text
                color: parent.lockClock.color
                font: parent.lockClock.font
            }
            Text {
                visible: surface.morphing
                x: parent.flyX - width / 2
                y: parent.flyY - height / 2
                scale: 1 / parent.sizeRatio + (1 - 1 / parent.sizeRatio) * surface.sb
                opacity: surface.ramp(0.2, 0.75) * standbyView.clockOpacity
                text: parent.lockClock.text
                color: standbyView.ink
                font { family: Theme.font.family; pixelSize: standbyView.clockSize; weight: standbyView.clockWeight; letterSpacing: -standbyView.clockSize * 0.03; features: { "tnum": 1 } }
            }
        }
    }
    function mix(a, b, t) {
        return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t);
    }
    Loader {
        active: Theme.material && surface.morphing && lockView.item !== null
        sourceComponent: Component {
            MaterialClock {
                readonly property var lock: lockView.item
                readonly property rect from: lock.clockRect
                readonly property rect to: standbyView.clockTarget
                readonly property real t: surface.sb
                readonly property real ratio: lock.clockSize > 0 ? standbyView.mClockSize / lock.clockSize : 1
                x: from.x + from.width / 2 + (to.x + to.width / 2 - from.x - from.width / 2) * t - width / 2
                y: from.y + from.height / 2 + (to.y + to.height / 2 - from.y - from.height / 2) * t - height / 2
                width: implicitWidth
                height: implicitHeight
                scale: 1 + (ratio - 1) * t
                time: lock.now
                size: lock.clockSize
                weight: lock.clockWeight + (standbyView.mClockWeight - lock.clockWeight) * t
                widthAxis: lock.clockWidth + (standbyView.mClockWidth - lock.clockWidth) * t
                color: surface.mix(lock.clockInk, standbyView.mInk, t)
                color2: surface.mix(lock.clockInk2, standbyView.mInk2, t)
                opacity: 1 + (standbyView.clockOpacity - 1) * t
                align: lock.split ? Text.AlignLeft : Text.AlignHCenter
            }
        }
    }
    // Any use of the lock screen keeps StandBy away
    Connections {
        target: surface.lockScope
        function onPasswordChanged() { surface.woke(); }
    }
    MouseArea {
        anchors.fill: parent
        z: -1
        hoverEnabled: true
        acceptedButtons: Qt.NoButton
        onPositionChanged: if (!surface.standby) surface.woke()
    }
}
