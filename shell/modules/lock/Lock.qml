import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pam
import Quickshell.Wayland
import qs.common
import qs.services

// Session lock. Every screen shows the clock; the password field lives on each
// screen too, sharing one PAM conversation.
//
// Transitions (Settings → Lock Screen), like iPhone:
//   unlocking  zoom: the lock's content lifts away; the wallpaper settles from
//              a little larger, widgets and icons zoom in from the centre in
//              turn, and windows, dock and menu bar fade in (LockFx and the
//              veil below); fade: the same without movement (unlockTransition)
//   locking    the lock covers the screen at once (no unlocked moment while
//              anything animates), then its own content plays the chosen
//              entrance: zoom or fade (lockTransition, LockSurface)
// StandBy state (idle, burn-in rests and shifts) lives here, shared by every
// screen, so they wake together and can keep their clocks in step.
Scope {
    id: root

    property string password: ""
    property string error: ""
    readonly property bool busy: pam.active
    readonly property var cfg: Config.options.lock

    function submit() {
        if (busy || password === "")
            return;
        error = "";
        pam.start();
    }

    // ── Lock and unlock, with their transitions ─────────────────────────
    // `held` is what the compositor sees; System.locked is what was asked
    property bool held: false
    Component.onCompleted: held = System.locked // locked at start (a restart while locked): no transition
    property bool leaving: false // the lock's content lifting away
    readonly property int leaveMs: 220
    // The veil: the wallpaper in a layer above the windows, showing exactly
    // what the desktop's own wallpaper shows (same picture, same zoom). It
    // covers the windows, dock and menu bar while the lock comes and goes,
    // and fading it is what fades them; the wallpaper underneath and the
    // widgets and icons do their own zoom (LockFx).
    property bool veiled: false

    Connections {
        target: System
        function onLockedChanged() {
            leaveTimer.stop();
            if (System.locked) {
                root.leaving = false;
                if (root.held)
                    return;
                // Acquire the compositor lock immediately. Animation must never
                // leave an unlocked, input-transparent password-entry interval.
                root.held = true;
                LockFx.rest();
            } else {
                if (!root.held) { // locked and unlocked again before the lock showed
                    root.veiled = false;
                    LockFx.reveal(300);
                    return;
                }
                const t = root.cfg.unlockTransition;
                if (t === "none") {
                    root.veiled = false;
                    LockFx.rest();
                    root.held = false;
                    return;
                }
                // Lock → desktop: while the lock's content lifts away, the
                // desktop is set back behind it and the veil goes up, so the
                // first frame without the lock looks just like the last one
                LockFx.prepare(t === "zoom");
                root.veiled = true;
                root.leaving = true;
                leaveTimer.restart();
            }
        }
    }
    Timer {
        id: leaveTimer
        interval: root.leaveMs
        onTriggered: {
            revealTimer.restart(); // (in case the lock doesn't report letting go)
            root.held = false;
            root.leaving = false;
        }
    }
    // The desktop settles once the compositor has let go of the lock and
    // draws it again; started earlier, its first frames were never seen
    Timer {
        id: revealTimer
        interval: 150
        onTriggered: if (!System.locked && LockFx.desktop === 0) LockFx.reveal(root.cfg.unlockTransition === "zoom" ? 560 : 360)
    }
    // Lowered once the desktop is back at rest, or once the lock is up
    Connections {
        target: LockFx
        function onActiveChanged() {
            if (!LockFx.active && !System.locked)
                root.veiled = false;
        }
    }
    Connections {
        target: lock
        function onSecureChanged() {
            System.lockSecure = lock.secure;
            if (lock.secure && System.locked) {
                root.veiled = false;
                LockFx.rest(); // (hidden behind the lock)
            } else if (!lock.secure && !System.locked && revealTimer.running) {
                revealTimer.interval = 60; // a frame or so for the desktop to show
                revealTimer.restart();
                revealTimer.interval = 150;
            }
        }
    }

    Variants {
        model: root.veiled ? Quickshell.screens : []
        PanelWindow {
            required property var modelData
            screen: modelData
            WlrLayershell.namespace: "irice:lockveil"
            WlrLayershell.layer: WlrLayer.Overlay
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"
            mask: Region { width: 0; height: 0 }
            Image {
                anchors.fill: parent
                source: Wallpaper.source
                fillMode: Image.PreserveAspectCrop
                // the screen's size from the start, so the first frame is
                // already the same picture as the desktop's and the lock's
                sourceSize: Qt.size(modelData.width, modelData.height)
                asynchronous: false
                scale: LockFx.wallpaperScale
                opacity: LockFx.veil
            }
        }
    }

    // ── StandBy, shared by every screen ─────────────────────────────────
    // On a charging laptop left alone for 5 minutes, lock the screen; the
    // lock screen then shows StandBy after 20 s more (see LockSurface)
    IdleMonitor {
        timeout: 300
        enabled: root.cfg.standby && Island.charging && !System.locked && !System.keepAwake
        onIsIdleChanged: if (isIdle && enabled) System.locked = true
    }
    readonly property bool canStandBy: cfg.standby && Island.charging
    property bool idle: false
    Timer {
        id: idleTimer
        interval: 20000
        running: root.held && root.canStandBy && !root.idle
        onTriggered: root.idle = true
    }
    function woke() {
        idle = false;
        idleTimer.restart();
    }
    onHeldChanged: if (!held) idle = false
    onPasswordChanged: woke()
    readonly property bool standby: held && !leaving && canStandBy && idle && password === "" && !busy

    // Burn-in protection (lock.standbyShift): every minute the content moves
    // up or down to a new spot (always centred across) (`drifts` counts the moves; fx, fy are the spot as a
    // fraction of the room each screen has, used when lock.standbySameClock
    // keeps every screen's clock in the same place), and every 15 minutes
    // the screens rest dark for a minute.
    property int drifts: 0
    property real fx: 0
    property real fy: 0
    property bool resting: false
    property int minutes: 0
    function drift() {
        if (!cfg.standbyShift) {
            fx = 0;
            fy = 0;
        } else {
            // Up or down only (the clock stays centred across), well away
            // from the last spot so no pixel holds a colour for long
            let y = 0;
            for (let i = 0; i < 8; i++) {
                y = Math.random() * 2 - 1;
                if (Math.abs(y - fy) > 0.5)
                    break;
            }
            fx = 0;
            fy = y;
        }
        drifts++;
    }
    onStandbyChanged: {
        minutes = 0;
        resting = false;
        restTimer.stop();
        if (standby)
            drift();
    }
    SystemClock {
        id: minuteClock
        precision: SystemClock.Minutes
        enabled: root.standby
        onDateChanged: {
            if (!root.standby || !root.cfg.standbyShift || root.resting)
                return;
            if (++root.minutes >= 15) {
                root.minutes = 0;
                root.resting = true;
                restTimer.restart();
            } else {
                root.drift();
            }
        }
    }
    Timer {
        id: restTimer
        interval: 60000
        onTriggered: {
            root.drift(); // moves while dark, so it comes back somewhere new
            root.resting = false;
        }
    }
    Connections {
        target: root.cfg
        function onStandbyShiftChanged() { root.drift(); }
    }
    // Someone's using StandBy during a rest: light it up again
    function endRest() {
        if (!resting)
            return;
        restTimer.stop();
        minutes = 0;
        resting = false;
    }

    WlSessionLock {
        id: lock
        locked: root.held

        LockSurface {
            lockScope: root
        }
    }

    PamContext {
        id: pam
        onResponseRequiredChanged: {
            if (responseRequired) {
                respond(root.password);
            }
        }
        onCompleted: result => {
            if (result === PamResult.Success) {
                System.locked = false;
                root.error = "";
            } else {
                root.error = "Incorrect password. Try again.";
                Sounds.play("error");
            }
            root.password = "";
        }
        onError: root.error = "Couldn't check your password. Try again."
    }

    // Sandbox only: `./dev.sh ipc lock unlock` (the sandbox user has no password)
    IpcHandler {
        target: "lock"
        function lock(): void { System.locked = true; }
        function unlock(): void {
            if (System.sandboxed)
                System.locked = false;
        }
    }
}
