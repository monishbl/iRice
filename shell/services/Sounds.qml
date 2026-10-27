pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Notifications
import Quickshell.Services.UPower
import qs.common

// Short UI sounds, like macOS: a chime when the session starts (not when the
// shell reloads), a blip when the charger goes in, two-tones when a display or
// USB device comes or goes, a latch for locking, glass taps for notifications,
// a pop for the volume keys, a knock for a wrong password, the shutter for
// screenshots, paper for emptying the Trash.
// iRice's own sounds are in assets/sounds/<style> (made by
// tools/make-sounds.py; the style is Settings → Notifications → Style); the
// shutter and paper come from the freedesktop sound theme. Each can be
// switched off, and some replaced by your own file (Settings → Notifications
// → Sound effects).
Singleton {
    id: root

    readonly property var cfg: Config.options.sounds
    readonly property string dir: "/usr/share/sounds/freedesktop/stereo/"
    readonly property var packs: ["marimba", "taps", "soft", "keys", "minimal", "glass"]
    readonly property string own: Quickshell.shellDir + "/assets/sounds/" + (packs.includes(cfg.pack) ? cfg.pack : "marimba") + "/"
    readonly property var files: ({
            startup: own + "startup.ogg",
            charging: own + "charging.ogg",
            displays: own + "display-connect.ogg",
            displaysOff: own + "display-disconnect.ogg",
            displaysLeft: own + "display-connect-left.ogg", // directional (DisplayFx)
            displaysRight: own + "display-connect-right.ogg",
            displaysOffLeft: own + "display-disconnect-left.ogg",
            displaysOffRight: own + "display-disconnect-right.ogg",
            lock: own + "lock.ogg",
            unlock: own + "unlock.ogg",
            notifications: own + "notification.ogg",
            battery: own + "battery.ogg",
            volumeKeys: own + "volume.ogg",
            usb: own + "usb-connect.ogg",
            usbOff: own + "usb-disconnect.ogg",
            received: own + "received.ogg",
            recording: own + "record-start.ogg",
            recordingOff: own + "record-stop.ogg",
            error: own + "error.ogg",
            screenshot: dir + "camera-shutter.oga",
            trash: dir + "trash-empty.oga"
        })
    // The setting a sound follows (and whose custom file replaces it), when
    // it isn't its own name
    readonly property var setting: ({ displaysOff: "displays", displaysLeft: "displays", displaysRight: "displays", displaysOffLeft: "displays", displaysOffRight: "displays", unlock: "lock", usbOff: "usb", recordingOff: "recording" })
    // Sounds that may only play this often (ms): the volume keys repeat
    readonly property var spacing: ({ volumeKeys: 70, notifications: 600, usb: 400, usbOff: 400 })
    property var lastPlayed: ({})

    // The file a sound plays: yours if you picked one
    function file(name) {
        const key = setting[name] ?? name;
        const custom = cfg[key + "File"];
        return custom && (name === key) ? custom : (files[name] ?? "");
    }

    // preview: play even when the sound is switched off (Settings' Play buttons)
    function play(name, preview) {
        const key = setting[name] ?? name;
        if (!preview && (!cfg.enabled || cfg[key] === false))
            return;
        const now = Date.now();
        if (!preview && now - (lastPlayed[name] ?? 0) < (spacing[name] ?? 0))
            return;
        lastPlayed[name] = now;
        playFile(file(name));
    }
    function playFile(f) {
        if (!f)
            return;
        const v = Math.max(0, Math.min(1, cfg.volume));
        if (Tools.has("pw-play"))
            Quickshell.execDetached(["pw-play", "--volume", v.toFixed(2), f]);
        else if (Tools.has("paplay"))
            Quickshell.execDetached(["paplay", "--volume", String(Math.round(v * 65536)), f]);
        else if (Tools.has("canberra-gtk-play"))
            Quickshell.execDetached(["canberra-gtk-play", "-f", f]);
    }

    // A banner that's about to show (Notifications calls this once Focus has
    // let it through). Low urgency stays quiet, and so do apps that ask for
    // silence; an app that names its own sound file gets that instead.
    function notify(n) {
        if (!cfg.enabled || !cfg.notifications || n.urgency === NotificationUrgency.Low)
            return;
        const h = n.hints ?? {};
        if (h["suppress-sound"])
            return;
        const own = String(h["sound-file"] ?? "");
        if (own.startsWith("/")) {
            const now = Date.now();
            if (now - (lastPlayed.notifications ?? 0) < spacing.notifications)
                return;
            lastPlayed.notifications = now;
            playFile(own);
            return;
        }
        play("notifications");
    }

    // Charger, display, lock and recording sounds wait until the shell has
    // settled, so starting (or reloading) on the charger, with two displays or
    // on the lock screen stays quiet
    property bool armed: false
    Timer {
        running: true
        interval: 5000
        onTriggered: root.armed = true
    }

    readonly property bool onBattery: UPower.onBattery
    onOnBatteryChanged: if (armed && !onBattery) play("charging")

    readonly property bool locked: System.locked
    onLockedChanged: if (armed) play(locked ? "lock" : "unlock")

    readonly property bool recordingNow: ScreenTools.recording
    onRecordingNowChanged: if (armed) play(recordingNow ? "recording" : "recordingOff")

    // A hotplug can add and remove outputs several times in a second; one
    // sound for where it ends up
    property int screenCount: Quickshell.screens.length
    property int heardCount: Quickshell.screens.length
    onScreenCountChanged: if (armed) settle.restart(); else heardCount = screenCount
    Timer {
        id: settle
        interval: 800
        onTriggered: {
            // Directional sound on: DisplayFx plays it, panned toward the display
            if (Config.options.displays.directionalSound)
                root.heardCount = root.screenCount;
            else if (root.screenCount > root.heardCount)
                root.play("displays");
            else if (root.screenCount < root.heardCount)
                root.play("displaysOff");
            root.heardCount = root.screenCount;
        }
    }

    // Startup: once per compositor session. The mark carries the Hyprland
    // instance, because the runtime dir outlives a session on another console
    // (the tty session shares /run/user/<uid> with your usual desktop).
    function checkStartup() {
        if (Tools.ready && !startup.running && !startup.done)
            startup.running = true;
    }
    Component.onCompleted: checkStartup()
    Connections {
        target: Tools
        function onReadyChanged() {
            root.checkStartup();
        }
    }
    Process {
        id: startup
        property bool done: false
        command: ["sh", "-c", "m=\"${XDG_RUNTIME_DIR:-/tmp}/irice-session-started-${HYPRLAND_INSTANCE_SIGNATURE:-none}\"; [ -e \"$m\" ] && exit 1; : > \"$m\""]
        onExited: code => {
            done = true;
            if (code === 0)
                root.play("startup");
        }
    }
}
