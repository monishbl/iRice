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
    property bool keepAwake: false

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

    // ── Extra Control Center toggles ──────────────────────────────────────
    // Game mode: no animations, blur, shadows, gaps or rounding. Turning it off
    // reloads the Hyprland config, which restores everything.
    property bool gameMode: false
    onGameModeChanged: {
        if (gameMode)
            hyprEval('hl.config({ animations = { enabled = false }, decoration = { blur = { enabled = false }, shadow = { enabled = false }, rounding = 0 }, general = { gaps_in = 0, gaps_out = 0 } })');
        else
            Quickshell.execDetached(["sh", "-c", "hyprctl reload config-only >/dev/null"]);
        // (the reload's configreloaded event re-applies corners and the shader)
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
    // A config reload (saving a config file, or leaving game mode) puts back
    // the file's values; re-apply what the shell sets
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name !== "configreloaded")
                return;
            root.applyRadius();
            if (root.antiFlashbang)
                root.applyShader();
        }
    }

    // Listen for a few seconds and name the song that's playing (SongRec).
    function identifySong() {
        Quickshell.execDetached(["sh", "-c", "command -v songrec >/dev/null || { notify-send -a 'Identify Song' 'SongRec is not installed' 'Install the songrec package to identify music.'; exit; }; notify-send -a 'Identify Song' 'Listening…' 'Play the song near your microphone.'; r=$(timeout 25 songrec recognize 2>/dev/null); if [ -n \"$r\" ]; then notify-send -a 'Identify Song' \"$r\" 'Copied to the clipboard'; printf %s \"$r\" | wl-copy; else notify-send -a 'Identify Song' 'No match' 'Try again with the music louder.'; fi"]);
    }
}
