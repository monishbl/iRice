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
        fingerTries = 0; // (someone's here: the reader gets its tries back)
        faceTries = 0;
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
        fingerTries = 0; // (someone's here: the reader may have stopped after five tries)
        faceTries = 0; // (and the camera looks again)
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

    // ── A finger, next to the password (Settings → Lock Screen →
    // Fingerprint) ─────────────────────────────────────────────────────
    // A second PAM conversation, from the shell's own PAM file (only
    // pam_fprintd: the system's files aren't touched), running while the
    // lock is up. Each waits up to 30 s; then another, up to five failed
    // tries in a row (typing, or waking StandBy, gives them back).
    property string fingerText: "" // what the reader wants, under the field
    property int fingerTries: 0
    readonly property bool fingerWanted: held && !leaving && Fingerprint.lockScreen && fingerTries < 5
    onFingerWantedChanged: {
        if (fingerWanted) {
            fingerRetry.restart();
        } else {
            fingerRetry.stop();
            if (finger.active)
                finger.abort();
            fingerText = "";
        }
        if (!held)
            fingerTries = 0;
    }
    Timer {
        id: fingerRetry
        interval: 400 // (a moment after the lock shows, or after a try)
        onTriggered: if (root.fingerWanted && !finger.active) {
            root.fingerText = "Touch the fingerprint reader to unlock";
            finger.start();
        }
    }
    PamContext {
        id: finger
        config: "irice-fingerprint"
        configDirectory: Quickshell.shellDir + "/assets/pam"
        onMessageChanged: {
            const m = message ?? "";
            if (/fail|match|no match|not recogni/i.test(m))
                root.fingerText = "Not recognised. Try again";
            else if (/place|swipe|touch|finger/i.test(m))
                root.fingerText = "Touch the fingerprint reader to unlock";
        }
        onCompleted: result => {
            if (result === PamResult.Success) {
                System.locked = false;
                root.error = "";
                root.fails = 0;
                root.fingerTries = 0;
                root.fingerText = "";
                return;
            }
            root.fingerTries++;
            if (root.fingerWanted)
                fingerRetry.restart();
        }
        onError: {
            root.fingerTries++;
            if (root.fingerWanted)
                fingerRetry.restart();
        }
    }

    // ── Your face, next to the password (Settings → Lock Screen → Face) ──
    // A third PAM conversation, from the shell's own PAM file (pam_howdy:
    // Howdy with the infrared camera). It looks when the lock shows and each
    // time someone's back (woke), three tries each time; never in StandBy.
    property string faceText: ""
    property int faceTries: 0
    readonly property bool faceWanted: held && !leaving && !standby && Face.lockScreen && faceTries < 3
    onFaceWantedChanged: {
        if (faceWanted) {
            faceRetry.restart();
        } else {
            faceRetry.stop();
            if (face.active)
                face.abort();
            faceText = "";
        }
    }
    Timer {
        id: faceRetry
        interval: 300
        onTriggered: if (root.faceWanted && !face.active) {
            root.faceText = "Looking for your face…";
            face.start();
        }
    }
    PamContext {
        id: face
        config: "irice-face"
        configDirectory: Quickshell.shellDir + "/assets/pam"
        onCompleted: result => {
            if (result === PamResult.Success) {
                System.locked = false;
                root.error = "";
                root.fails = 0;
                root.faceTries = 0;
                root.faceText = "";
                return;
            }
            root.faceTries++;
            root.faceText = root.faceTries < 3 ? "Face not recognised" : "";
            if (root.faceWanted)
                faceRetry.restart();
        }
        onError: {
            root.faceTries = 3; // (Howdy or the camera isn't working: the password is there)
            root.faceText = "";
        }
    }

    PamContext {
        id: pam
        onResponseRequiredChanged: {
            if (responseRequired) {
                respond(root.password);
            }
        }
        // pam_faillock says so itself unless set to be silent ("The account
        // is locked due to 3 failed logins. (9 minutes left to unlock)")
        onMessageChanged: if (/locked/i.test(message ?? ""))
            root.pamLocked = true
        onCompleted: result => {
            if (result === PamResult.Success) {
                System.locked = false;
                root.error = "";
                root.fails = 0;
            } else {
                root.fails++;
                root.error = root.pamLocked ? root.lockedText(root.faillockConf.unlock) : "Incorrect password. Try again.";
                Sounds.play("error");
                root.checkLockout();
            }
            root.pamLocked = false;
            root.password = "";
        }
        onError: root.error = "Couldn't check your password. Try again."
    }

    // ── Too many attempts (pam_faillock) ────────────────────────────────
    // After `deny` failures within `fail_interval`, pam_faillock refuses even
    // the right password until `unlock_time` has passed, and PAM's answer
    // looks just like a wrong password. After each failure: `faillock --user`
    // (the user's own tally is readable without root) says whether that's
    // the case and for how long; where it can't be read, three failures in a
    // row are taken as a lockout, with the time from faillock.conf.
    property int fails: 0 // in a row, since the last unlock
    property bool pamLocked: false
    property var faillockConf: ({ deny: 3, unlock: 600, interval: 900 }) // pam_faillock's defaults
    function lockedText(seconds) {
        if (seconds <= 0)
            return "Too many attempts: this account stays locked until it's unlocked with faillock --reset from another session.";
        const m = Math.max(1, Math.ceil(seconds / 60));
        return "Too many attempts: try again in about " + m + (m === 1 ? " minute." : " minutes.");
    }
    function checkLockout() {
        if (!faillockCheck.running)
            faillockCheck.running = true;
    }
    // faillockConf, then the tally: "CONF deny unlock interval" and
    // "2026-10-30 13:27:01 TTY /dev/pts/1 V" lines ("NOREAD" when it can't be read)
    Process {
        id: faillockCheck
        command: ["sh", "-c", "f=/etc/security/faillock.conf; v() { [ -r $f ] && sed -n \"s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*\\([0-9a-z]*\\).*/\\1/p\" $f | tail -1; }; echo CONF \"$(v deny)\" \"$(v unlock_time)\" \"$(v fail_interval)\"; command -v faillock >/dev/null 2>&1 && faillock --user \"$(id -un)\" 2>/dev/null || echo NOREAD"]
        stdout: StdioCollector {
            onStreamFinished: root.judgeLockout(text)
        }
    }
    function judgeLockout(text) {
        const c = text.match(/^CONF (\S*) (\S*) (\S*)/m);
        if (c)
            faillockConf = {
                deny: parseInt(c[1]) > 0 ? parseInt(c[1]) : 3,
                unlock: c[2] === "never" ? 0 : (c[2] !== "" && !isNaN(parseInt(c[2])) ? parseInt(c[2]) : 600),
                interval: parseInt(c[3]) > 0 ? parseInt(c[3]) : 900
            };
        const conf = faillockConf;
        let left = -1; // seconds until it unlocks; -1: not locked
        if (/^NOREAD/m.test(text) || !/^When\s/m.test(text)) {
            if (fails >= conf.deny)
                left = conf.unlock;
        } else {
            const now = Date.now();
            const times = [];
            const re = /^(\d{4})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)\s.*\sV\s*$/gm;
            let m;
            while ((m = re.exec(text)) !== null)
                times.push(new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]).getTime());
            const recent = times.filter(t => now - t <= conf.interval * 1000);
            if (recent.length >= conf.deny) {
                const last = Math.max.apply(null, recent);
                left = conf.unlock === 0 ? 0 : Math.max(0, Math.round((last + conf.unlock * 1000 - now) / 1000));
                if (conf.unlock !== 0 && left <= 0)
                    left = -1;
            }
        }
        if (left >= 0 && error !== "")
            error = lockedText(left);
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
