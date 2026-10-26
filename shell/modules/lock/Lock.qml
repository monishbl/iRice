import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pam
import Quickshell.Wayland
import qs.common
import qs.services

// Session lock. Every screen shows the clock; the password field lives on each
// screen too, sharing one PAM conversation.
Scope {
    id: root

    property string password: ""
    property string error: ""
    readonly property bool busy: pam.active

    function submit() {
        if (busy || password === "")
            return;
        error = "";
        pam.start();
    }

    // StandBy: on a charging laptop left alone for 5 minutes, lock the
    // screen; the lock screen then shows StandBy (see LockSurface)
    IdleMonitor {
        timeout: 300
        enabled: Config.options.lock.standby && Island.charging && !System.locked
        onIsIdleChanged: if (isIdle && enabled) System.locked = true
    }

    WlSessionLock {
        id: lock
        locked: System.locked

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
