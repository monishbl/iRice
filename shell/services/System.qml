pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Small system switches shown in Control Center.
Singleton {
    id: root

    readonly property bool dnd: Focus.on // any Focus mode silences banners
    property bool nightLight: false
    property int nightLightTemp: 4500
    // Caffeine: the screen stays awake (no dimming, locking or sleep while
    // idle) until turned off, or for a while (keepAwakeUntil, ms since epoch).
    // An idle inhibitor on the menu bar does the work (MenuBar.qml).
    property bool keepAwake: false
    property real keepAwakeUntil: 0
    property int keepAwakeMinutes: 0 // the length chosen; 0 = until turned off
    function setKeepAwake(on, minutes) {
        keepAwakeUntil = on && minutes > 0 ? Date.now() + minutes * 60000 : 0;
        keepAwakeMinutes = on ? Math.max(0, minutes) : 0;
        if (keepAwake === on && !on)
            return;
        keepAwake = on;
        Island.post({ kind: "custom", icon: "coffee", title: on ? "Caffeine on" : "Caffeine off", subtitle: on ? (minutes > 0 ? "Staying awake for " + (minutes >= 60 ? (minutes / 60) + (minutes === 60 ? " hour" : " hours") : minutes + " min") : "Staying awake until you turn it off") : "Sleeps when idle again", color: Theme.accents.orange });
    }
    // qs -c irice ipc call caffeine toggle | on <minutes, 0 = until off> | off
    IpcHandler {
        target: "caffeine"
        function toggle(): void { root.setKeepAwake(!root.keepAwake, 0); }
        function on(minutes: int): void { root.setKeepAwake(true, minutes); }
        function off(): void { root.setKeepAwake(false, 0); }
    }
    readonly property string keepAwakeStatus: !keepAwake ? "" : keepAwakeUntil > 0 ? "Until " + Qt.formatTime(new Date(keepAwakeUntil), "h:mm AP") : "Until off"
    Timer {
        running: root.keepAwake && root.keepAwakeUntil > 0
        interval: Math.max(1000, root.keepAwakeUntil - Date.now())
        onTriggered: root.setKeepAwake(false, 0)
    }

    function toggleDarkMode() {
        Config.options.appearance.mode = Theme.dark ? "light" : "dark";
    }

    // hyprsunset applies the warm filter while it runs and resets on exit.
    Process {
        running: root.nightLight
        command: ["hyprsunset", "-t", String(root.nightLightTemp)]
    }

    function run(cmd) {
        Quickshell.execDetached(cmd);
    }

    // Repair Shell: a separate script (it outlives this shell) that removes
    // leftover and duplicate shells and starts one fresh. Apps keep running.
    function repairShell() {
        Panels.close();
        const script = decodeURIComponent(Qt.resolvedUrl("../assets/scripts/repair-shell.sh").toString().replace("file://", ""));
        Quickshell.execDetached(["setsid", "-f", script]);
    }

    // In the dev sandbox, systemctl would reach the real machine through the
    // shared system bus, so power actions only report what they would do.
    readonly property bool sandboxed: Quickshell.env("IRICE_SANDBOX") === "1"
    readonly property var powerCommands: ({
            "sleep": ["systemctl", "suspend"],
            "restart": ["systemctl", "reboot"],
            "shutdown": ["systemctl", "poweroff"]
        })

    function power(action) {
        if (action === "logout") {
            Hyprland.dispatch("hl.dsp.exit()");
            return;
        }
        if (action === "lock") {
            locked = true;
            return;
        }
        const cmd = powerCommands[action];
        if (!cmd)
            return;
        if (sandboxed)
            Quickshell.execDetached(["notify-send", "-a", "iRice sandbox", "Skipped " + action, cmd.join(" ")]);
        else
            Quickshell.execDetached(cmd);
    }

    property bool locked: false
    // Remembered for this Hyprland session, so a shell that restarts while the
    // screen is locked (a crash, a repair) locks again straight away
    readonly property string lockMark: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-locked-" + (Quickshell.env("HYPRLAND_INSTANCE_SIGNATURE") || "none")
    onLockedChanged: Quickshell.execDetached(locked ? ["touch", lockMark] : ["rm", "-f", lockMark])
    Process {
        running: true
        command: ["test", "-e", root.lockMark]
        onExited: code => {
            if (code === 0)
                root.locked = true;
        }
    }

    // ── Extra Control Center toggles ──────────────────────────────────────
    // Game mode: no animations, blur, shadows, gaps or rounding. Turning it off
    // reloads the Hyprland config, which restores everything.
    property bool gameMode: false
    onGameModeChanged: {
        if (gameMode)
            hyprEval('hl.config({ animations = { enabled = false }, decoration = { blur = { enabled = false }, shadow = { enabled = false }, rounding = 0, active_opacity = 1.0, inactive_opacity = 1.0 }, general = { gaps_in = 0, gaps_out = 0 } })');
        else
            Quickshell.execDetached(["sh", "-c", "hyprctl reload config-only >/dev/null"]);
        // (the reload's configreloaded event re-applies corners and the shader)
    }

    // Gestures are set in the Hyprland config, which reads this setting:
    // reload it once the change is saved
    Connections {
        target: Config.options.input
        function onThreeFingerDragChanged() { gestureReload.restart(); }
    }
    // Same for the window tab style (hypr/irice/tabs.lua)
    Connections {
        target: Config.options.appearance
        function onWindowTabsChanged() { gestureReload.restart(); }
    }
    Timer {
        id: gestureReload
        interval: 800 // after Config's write
        onTriggered: Quickshell.execDetached(["sh", "-c", "hyprctl reload config-only >/dev/null"])
    }

    // Anti-flashbang: a screen shader that tames very bright content.
    property bool antiFlashbang: false
    readonly property string shaderPath: decodeURIComponent(Qt.resolvedUrl("../assets/hypr/antiflashbang.glsl").toString().replace("file://", ""))
    onAntiFlashbangChanged: applyShader()
    function applyShader() {
        hyprEval('hl.config({ decoration = { screen_shader = "' + (antiFlashbang ? shaderPath : "") + '" } })');
    }

    // EasyEffects: global bypass on/off (effects active = toggle on)
    property bool easyEffects: true
    function setEasyEffects(on) {
        easyEffects = on;
        Quickshell.execDetached(["easyeffects", "--bypass", on ? "2" : "1"]);
    }

    // Cloudflare WARP
    property bool warp: false
    function setWarp(on) {
        warp = on;
        Quickshell.execDetached(["warp-cli", on ? "connect" : "disconnect"]);
    }
    Process {
        running: Tools.has("warp-cli")
        command: ["sh", "-c", "warp-cli status 2>/dev/null | grep -qi 'status update: connected' && echo on"]
        stdout: StdioCollector { onStreamFinished: root.warp = text.trim() === "on" }
    }

    function hyprEval(lua) {
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }

    // Window corners follow Settings → Appearance (continuous curve)
    readonly property int windowRadius: Config.ready ? Config.options.appearance.windowRadius : -1
    onWindowRadiusChanged: applyRadius()
    function applyRadius() {
        if (windowRadius >= 0 && !gameMode)
            hyprEval("hl.config({ decoration = { rounding = " + windowRadius + ", rounding_power = 4.0 } })");
    }
    // Pointer theme follows Settings → Appearance (cursor.lua sets it at login)
    readonly property string cursor: Config.ready ? Config.options.appearance.cursor : ""
    onCursorChanged: if (cursor !== "") {
        // (Hyprland draws macOS-hypr, the sharper hyprcursor build, when it's installed)
        const t = cursor === "bibata" ? "Bibata-Modern-Classic" : "macOS";
        Quickshell.execDetached(["sh", "-c", "h=$1; [ \"$1\" = macOS ] && [ -d /usr/share/icons/macOS-hypr ] && h=macOS-hypr; hyprctl setcursor \"$h\" 24 >/dev/null; gsettings set org.gnome.desktop.interface cursor-theme \"$1\" 2>/dev/null", "sh", t]);
    }

    // Window transparency follows Settings → Appearance: the focused window a
    // touch see-through, the others a little more, with the blur behind
    // them. Fullscreen stays opaque, and so do video and games (rules.lua).
    readonly property string windowOpacity: Config.ready ? Config.options.appearance.windowOpacity : ""
    readonly property var opacityLevels: ({ "off": [1, 1], "subtle": [0.97, 0.93], "more": [0.92, 0.86] })
    onWindowOpacityChanged: applyOpacity()
    function applyOpacity() {
        const o = opacityLevels[windowOpacity];
        if (!o || gameMode)
            return;
        hyprEval("hl.config({ decoration = { active_opacity = " + o[0] + ", inactive_opacity = " + o[1] + ", fullscreen_opacity = 1.0 } })");
    }

    // A config reload (saving a config file, or leaving game mode) puts back
    // the file's values; re-apply what the shell sets
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name !== "configreloaded")
                return;
            root.applyRadius();
            root.applyOpacity();
            if (root.antiFlashbang)
                root.applyShader();
        }
    }

    // Listen for a few seconds and name the song that's playing (SongRec).
    function identifySong() {
        Quickshell.execDetached(["sh", "-c", "command -v songrec >/dev/null || { notify-send -a 'Identify Song' 'SongRec is not installed' 'Install the songrec package to identify music.'; exit; }; notify-send -a 'Identify Song' 'Listening…' 'Play the song near your microphone.'; r=$(timeout 25 songrec recognize 2>/dev/null); if [ -n \"$r\" ]; then notify-send -a 'Identify Song' \"$r\" 'Copied to the clipboard'; printf %s \"$r\" | wl-copy; else notify-send -a 'Identify Song' 'No match' 'Try again with the music louder.'; fi"]);
    }
}
