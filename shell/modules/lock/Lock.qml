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
// Transitions (Settings → Lock Screen):
//   locking    fade: the wallpaper fades in over the desktop, then the lock
//              takes over showing the same picture (lockTransition)
//   unlocking  zoom: the lock's content lifts away, growing and fading, then
//              the desktop settles from a little closer (LockFx); fade: the
//              same without the zoom (unlockTransition)
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
    property bool fading: false // the wallpaper fading in before the lock
    property bool leaving: false // the lock's content lifting away
    readonly property int leaveMs: cfg.unlockTransition === "zoom" ? 200 : 160

    Connections {
        target: System
        function onLockedChanged() {
            if (System.locked) {
                leaveTimer.stop();
                root.leaving = false;
                LockFx.desktop = 1; // an unlock cut short: nothing stays set back
                if (root.held)
                    return;
                if (root.cfg.lockTransition === "fade" && Wallpaper.started) {
                    root.fading = true;
                    fadeTimer.restart();
                } else {
                    root.held = true;
                }
            } else {
                fadeTimer.stop();
                root.fading = false;
                if (!root.held)
                    return;
                const t = root.cfg.unlockTransition;
                if (t === "none") {
                    LockFx.desktop = 1;
                    root.held = false;
                    return;
                }
                // The desktop is set back while still hidden, so its first
                // frame after the lock is already the start of the settle
                LockFx.prepare(t === "zoom");
                root.leaving = true;
                leaveTimer.restart();
            }
        }
    }
    Timer {
        id: fadeTimer
        interval: 240
        onTriggered: root.held = true
    }
    Timer {
        id: leaveTimer
        interval: root.leaveMs
        onTriggered: {
            root.held = false;
            root.leaving = false;
            LockFx.reveal(root.cfg.unlockTransition === "zoom" ? 420 : 300);
        }
    }

    // Desktop → lock: the lock's wallpaper fades in over the desktop; the lock
    // surface appears on top showing the same picture, then this goes away
    Variants {
        model: root.fading || (root.held && !lock.secure) ? Quickshell.screens : []
        PanelWindow {
            required property var modelData
            screen: modelData
            WlrLayershell.namespace: "irice:lockfade"
            WlrLayershell.layer: WlrLayer.Overlay
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"
            mask: Region { width: 0; height: 0 }
            Image {
                id: fadePicture
                anchors.fill: parent
                source: Wallpaper.source
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(width, height)
                asynchronous: true
                opacity: 0
                // Starts once the picture is decoded, so it never pops in
                onStatusChanged: if (status === Image.Ready) fadeIn.start()
                NumberAnimation on opacity {
                    id: fadeIn
                    running: false
                    to: 1
                    duration: 220
                    easing.type: Easing.OutCubic
                }
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
    // to a new spot (`drifts` counts the moves; fx, fy are the spot as a
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
            // Well away from the last spot, so no pixel holds a colour for long
            let x = 0, y = 0;
            for (let i = 0; i < 6; i++) {
                x = Math.random() * 2 - 1;
                y = Math.random() * 2 - 1;
                if (Math.hypot(x - fx, y - fy) > 0.5)
                    break;
            }
            fx = x;
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
