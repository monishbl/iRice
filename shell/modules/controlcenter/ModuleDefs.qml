import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.UPower
import qs.common
import qs.services

// Every Control Center module: what it's called, its icon and state, what a
// click does, which detail page it opens, and the sizes it comes in. Used by
// the grid, the editor and the gallery, so they always agree.
//   kind: toggle (on/off) | action (one-shot) | slider | media | info
QtObject {
    id: root

    readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wifiNetwork: wifiDevice?.networks.values.find(n => n.connected) ?? null
    readonly property var bt: Bluetooth.defaultAdapter
    readonly property var btConnected: Bluetooth.devices.values.filter(d => d.connected)

    readonly property var all: ({
            wifi: { label: "Wi-Fi", kind: "toggle", sizes: ["2x1", "1x1"], icon: !Networking.wifiEnabled ? "wifi-slash" : "wifi-high", on: Networking.wifiEnabled, status: !Networking.wifiEnabled ? "Off" : wifiNetwork?.name ?? "Not connected", page: "wifi", act: () => Networking.wifiEnabled = !Networking.wifiEnabled },
            bluetooth: { label: "Bluetooth", kind: "toggle", sizes: ["1x1", "2x1"], icon: "bluetooth", on: bt?.enabled ?? false, status: !(bt?.enabled) ? "Off" : btConnected.length > 0 ? btConnected[0].name : "On", page: "bluetooth", act: () => { if (bt) bt.enabled = !bt.enabled; } },
            focus: { label: Focus.on ? Focus.mode.name : "Focus", kind: "toggle", sizes: ["2x1", "1x1"], icon: Focus.mode?.icon ?? "moon", on: Focus.on, status: Focus.on && Focus.until > 0 ? Focus.statusText : "", page: "focus", act: () => Focus.on ? Focus.deactivate() : Focus.activate("dnd", 0, false) },
            nightLight: { label: "Night Light", kind: "toggle", sizes: ["1x1", "2x1"], icon: "moon-stars", on: System.nightLight, status: System.nightLight ? System.nightLightTemp + "K" : "Off", act: () => System.nightLight = !System.nightLight },
            darkMode: { label: Theme.dark ? "Dark Mode" : "Light Mode", kind: "toggle", sizes: ["1x1", "2x1"], icon: "circle-half", on: Theme.dark, status: Config.options.appearance.mode === "auto" ? "Auto" : "", act: () => System.toggleDarkMode() },
            keepAwake: { label: "Caffeine", kind: "toggle", sizes: ["1x1", "2x1"], icon: "coffee", on: System.keepAwake, status: System.keepAwakeStatus, page: "caffeine", act: () => System.setKeepAwake(!System.keepAwake, 0) },
            stageManager: { label: "Stage Manager", kind: "toggle", sizes: ["1x1", "2x1"], icon: "squares-four", on: Config.options.stageManager.enabled, status: "", act: () => Config.options.stageManager.enabled = !Config.options.stageManager.enabled },
            mic: { label: "Microphone", kind: "toggle", sizes: ["1x1", "2x1"], icon: Audio.micMuted ? "microphone-slash" : "microphone", on: !Audio.micMuted, status: Audio.micMuted ? "Muted" : "On", act: () => Audio.toggleMic() },
            lowPower: { label: "Low Power", kind: "toggle", sizes: ["1x1", "2x1"], icon: "leaf", on: PowerProfiles.profile === PowerProfile.PowerSaver, status: "", act: () => PowerProfiles.profile = PowerProfiles.profile === PowerProfile.PowerSaver ? PowerProfile.Balanced : PowerProfile.PowerSaver },
            gameMode: { label: "Game Mode", kind: "toggle", sizes: ["1x1", "2x1"], icon: "game-controller", on: System.gameMode, status: "", act: () => System.gameMode = !System.gameMode },
            antiFlashbang: { label: "Anti-Flash", kind: "toggle", sizes: ["1x1", "2x1"], icon: "sun-dim", on: System.antiFlashbang, status: "", act: () => System.antiFlashbang = !System.antiFlashbang },
            record: { label: "Record", kind: "toggle", sizes: ["1x1", "2x1"], icon: "record", on: ScreenTools.recording, status: ScreenTools.recording ? "Recording" : "", needs: "wf-recorder", act: () => { if (ScreenTools.recording) ScreenTools.stopRecording(); else ScreenTools.select("record"); } },
            easyEffects: { label: "EasyEffects", kind: "toggle", sizes: ["1x1", "2x1"], icon: "faders", on: System.easyEffects, status: "", needs: "easyeffects", act: () => System.setEasyEffects(!System.easyEffects) },
            warp: { label: "WARP", kind: "toggle", sizes: ["1x1", "2x1"], icon: "cloud", on: System.warp, status: "", needs: "warp-cli", act: () => System.setWarp(!System.warp) },
            keyboard: { label: "Keyboard", kind: "action", sizes: ["1x1", "2x1"], icon: "keyboard", act: () => Panels.show("keyboard") },
            screenshot: { label: "Screenshot", kind: "action", sizes: ["1x1", "2x1"], icon: "camera", act: () => Panels.show("screenTools") },
            colorPicker: { label: "Colour Picker", kind: "action", sizes: ["1x1", "2x1"], icon: "eyedropper", needs: "hyprpicker", act: () => { Panels.close(); System.run(["sh", "-c", "sleep 0.3; hyprpicker -a"]); } },
            musicRecognition: { label: "Identify Song", kind: "action", sizes: ["1x1", "2x1"], icon: "music-notes", needs: "songrec", act: () => { Panels.close(); System.identifySong(); } },
            clipboard: { label: "Clipboard", kind: "action", sizes: ["1x1", "2x1"], icon: "clipboard-text", act: () => Panels.show("clipboard") },
            calculator: { label: "Calculator", kind: "action", sizes: ["1x1", "2x1"], icon: "calculator", act: () => { Panels.close(); Launch.run(["sh", "-c", "gnome-calculator || qalculate-gtk || kcalc || galculator"]); } },
            timer: { label: "Timer", kind: "action", sizes: ["1x1", "2x1"], icon: "timer", act: () => { Panels.ncTab = 2; Panels.show("notificationCenter"); } },
            tileWindows: { label: "Tile Left & Right", kind: "action", sizes: ["2x1", "1x1"], icon: "square-split-horizontal", act: () => { Panels.close(); Tiling.arrange("leftRight"); } },
            appLibrary: { label: "App Library", kind: "action", sizes: ["1x1", "2x1"], icon: "squares-four", act: () => Panels.show("launchpad") },
            display: { label: "Display", kind: "slider", sizes: ["4x1"], icon: "sun", available: Brightness.available },
            sound: { label: "Sound", kind: "slider", sizes: ["4x1"], icon: Audio.icon, page: "sound" },
            nowPlaying: { label: "Now Playing", kind: "media", sizes: ["2x2", "4x1"], icon: "music-notes" },
            battery: { label: "Battery", kind: "info", sizes: ["2x1", "1x1"], icon: "battery-high", available: Island.hasBattery },
            resources: { label: "CPU & Memory", kind: "info", sizes: ["2x1", "2x2"], icon: "cpu" },
            weather: { label: "Weather", kind: "info", sizes: ["2x1", "2x2"], icon: Weather.icon }
        })

    readonly property var defaultLayout: ["wifi:2x1", "nowPlaying:2x2", "bluetooth:1x1", "nightLight:1x1", "focus:2x1", "stageManager:1x1", "darkMode:1x1", "display:4x1", "sound:4x1", "screenshot:1x1", "record:1x1", "keepAwake:1x1", "clipboard:1x1"]

    function usable(id) {
        const d = all[id];
        return !!d && d.available !== false && (!d.needs || Tools.has(d.needs));
    }

    // "id:WxH" -> { id, w, h }
    function parse(entry) {
        const [id, size] = entry.split(":");
        const [w, h] = (size ?? "1x1").split("x").map(Number);
        return { id: id, w: w || 1, h: h || 1 };
    }

    // First-fit into `cols` columns, row by row: [{ id, w, h, col, row }]
    function pack(entries, cols) {
        const taken = [];
        const free = (c, r, w, h) => {
            if (c + w > cols)
                return false;
            for (let y = r; y < r + h; y++)
                for (let x = c; x < c + w; x++)
                    if (taken[y]?.[x])
                        return false;
            return true;
        };
        const out = [];
        for (const e of entries) {
            const w = Math.min(e.w, cols);
            let placed = false;
            for (let r = 0; !placed && r < 64; r++)
                for (let c = 0; !placed && c < cols; c++)
                    if (free(c, r, w, e.h)) {
                        for (let y = r; y < r + e.h; y++) {
                            taken[y] = taken[y] ?? [];
                            for (let x = c; x < c + w; x++)
                                taken[y][x] = true;
                        }
                        out.push(Object.assign({}, e, { w: w, col: c, row: r }));
                        placed = true;
                    }
        }
        return out;
    }
}
