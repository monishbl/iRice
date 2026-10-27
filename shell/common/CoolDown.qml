pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import Quickshell.Services.UPower

// Cool-down: less work for the CPU and GPU while the laptop is hot, on
// battery, or when switched on (Settings → Appearance → Cool-down, and a
// Control Center control). While it's on, as set:
//   - glass turns frosted (or solid): the cheapest materials to draw
//   - Hyprland's blur gets lighter (fewer, smaller passes)
//   - animations get shorter (Theme.motion)
//   - a live wallpaper pauses (Background.qml)
//   - the laptop's own display drops to its lowest refresh rate (48 Hz here), so
//     everything on screen, apps too, draws fewer frames
// All of it comes back when cool-down ends.
Singleton {
    id: root

    readonly property var cfg: Config.options.cooling
    property bool hot: false // set by the island's temperature watch (services/Island.qml)
    // Ask mode (cooling.hotAction "ask"): hot only cools down once you say so
    // in the island's alert; "Not now" keeps it off until the laptop cools
    property bool accepted: false
    property bool declined: false
    onHotChanged: if (!hot) {
        accepted = false;
        declined = false;
    }
    property bool manual: false // the Control Center control
    // Turned off by hand while hot or on battery: stays off until that ends
    property bool snoozed: false
    readonly property bool onBattery: UPower.onBattery
    readonly property bool hotOn: cfg.whenHot && hot && (cfg.hotAction !== "ask" || accepted)
    readonly property bool automatic: Config.ready && (hotOn || (cfg.onBattery && onBattery))
    onAutomaticChanged: if (!automatic) snoozed = false
    readonly property bool active: Config.ready && (manual || (automatic && !snoozed))
    function toggle() {
        if (active) {
            manual = false;
            if (automatic)
                snoozed = true;
        } else {
            snoozed = false;
            manual = true;
        }
    }
    readonly property string why: !active ? (snoozed || declined ? "Off until it cools or you plug in" : "") : manual ? "Turned on" : hotOn ? "The CPU is hot" : "On battery"
    // qs -c irice ipc call cooldown toggle | on | off
    IpcHandler {
        target: "cooldown"
        function toggle(): void { root.toggle(); }
        function on(): void { root.snoozed = false; root.manual = true; }
        function off(): void { root.manual = false; if (root.automatic) root.snoozed = true; }
    }

    // Read by Theme and the background
    readonly property string glass: active ? cfg.glass : "keep" // keep | frosted | solid
    readonly property real motionScale: active && cfg.shortAnimations ? 0.6 : 1
    readonly property bool pauseLive: active && cfg.pauseLiveWallpaper

    function hyprEval(lua) {
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }

    // What cool-down changed is kept in the state file too, so a shell that
    // restarts mid cool-down still knows what to put back (and does, if
    // cool-down has ended meanwhile)
    readonly property bool stateReady: Config.ready && UiState.loaded
    onStateReadyChanged: {
        if (!stateReady)
            return;
        const s = UiState.data.coolDown;
        if (s) {
            savedBlur = s.blur ?? null;
            savedRates = s.rates ?? {};
        }
        applyBlur();
        monitorsRead.running = true;
    }
    function remember() {
        if (stateReady)
            UiState.data.coolDown = savedBlur === null && Object.keys(savedRates).length === 0 ? null : { blur: savedBlur, rates: savedRates };
    }
    onSavedBlurChanged: remember()
    onSavedRatesChanged: remember()

    // ---- Blur ------------------------------------------------------------
    // The config's values are read when cool-down starts, and put back after
    property var savedBlur: null
    readonly property bool lightBlur: active && cfg.lightBlur
    onLightBlurChanged: if (stateReady) applyBlur()
    function applyBlur() {
        if (lightBlur) {
            if (savedBlur === null)
                blurRead.running = true;
            else
                hyprEval("hl.config({ decoration = { blur = { passes = 1, size = 5 } } })");
        } else if (savedBlur !== null) {
            hyprEval("hl.config({ decoration = { blur = { passes = " + savedBlur.passes + ", size = " + savedBlur.size + " } } })");
            savedBlur = null;
        }
    }
    Process {
        id: blurRead
        command: ["sh", "-c", "hyprctl -j getoption decoration:blur:passes; hyprctl -j getoption decoration:blur:size"]
        stdout: StdioCollector {
            onStreamFinished: {
                const nums = (text.match(/"int":\s*(\d+)/g) ?? []).map(s => parseInt(s.split(":")[1]));
                // Cool-down may have ended while this was reading: then
                // nothing was changed and there's nothing to put back
                if (!root.lightBlur)
                    return;
                root.savedBlur = { passes: nums[0] ?? 3, size: nums[1] ?? 8 };
                root.applyBlur();
            }
        }
    }

    // ---- Refresh rate ----------------------------------------------------
    // The laptop's own panel switches to its lowest rate at the same
    // resolution (40 Hz or more); the rate it had comes back after. External
    // and mirrored screens are left alone.
    // Only once per cool-down (it starts and ends steadily, Island.qml), and
    // off unless asked for: the mode change blanks the screen for a moment
    readonly property bool lowRefresh: active && cfg.refreshDrop
    onLowRefreshChanged: if (stateReady) monitorsRead.running = true
    property var savedRates: ({}) // output name -> refresh rate before cool-down
    function setRate(m, rate) {
        root.hyprEval("hl.monitor({ output = \"" + m.name + "\", mode = \"" + m.width + "x" + m.height + "@" + rate.toFixed(2) + "\", position = \"" + m.x + "x" + m.y + "\", scale = " + m.scale + (m.transform ? ", transform = " + m.transform : "") + " })");
    }
    Process {
        id: monitorsRead
        command: ["hyprctl", "-j", "monitors"]
        stdout: StdioCollector {
            onStreamFinished: {
                let mons = [];
                try {
                    mons = JSON.parse(text);
                } catch (e) {
                    return;
                }
                const saved = Object.assign({}, root.savedRates);
                for (const m of mons) {
                    if (!/^(eDP|LVDS|DSI)/.test(m.name) || (m.mirrorOf && m.mirrorOf !== "none"))
                        continue;
                    if (!root.lowRefresh) {
                        // Only put back what cool-down changed
                        if (saved[m.name] !== undefined && Math.abs(saved[m.name] - m.refreshRate) >= 0.5)
                            root.setRate(m, saved[m.name]);
                        delete saved[m.name];
                        continue;
                    }
                    const same = (m.availableModes ?? []).map(s => s.match(/^(\d+)x(\d+)@([\d.]+)Hz$/)).filter(x => x && +x[1] === m.width && +x[2] === m.height).map(x => parseFloat(x[3]));
                    const pick = Math.min(...same.filter(r => r >= 40));
                    if (!isFinite(pick) || Math.abs(pick - m.refreshRate) < 0.5)
                        continue;
                    if (saved[m.name] === undefined)
                        saved[m.name] = m.refreshRate;
                    root.setRate(m, pick);
                }
                root.savedRates = saved;
            }
        }
    }

    // A config reload puts the file's blur and monitor settings back
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name !== "configreloaded" || !root.active)
                return;
            root.savedBlur = null;
            reapply.restart();
        }
    }
    Timer {
        id: reapply
        interval: 300
        onTriggered: {
            root.applyBlur();
            monitorsRead.running = true;
        }
    }
}
