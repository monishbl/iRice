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
            // (without NetworkManager there's nothing for it to switch: Backends)
            wifi: { label: "Wi-Fi", kind: "toggle", sizes: ["2x1", "1x1"], available: !Backends.wifiUnmanaged, reason: Backends.wifiShort, icon: !Networking.wifiEnabled ? "wifi-slash" : "wifi-high", on: Networking.wifiEnabled, status: !Networking.wifiEnabled ? "Off" : wifiNetwork?.name ?? "Not connected", page: "wifi", act: () => Networking.wifiEnabled = !Networking.wifiEnabled },
            bluetooth: { label: "Bluetooth", kind: "toggle", sizes: ["1x1", "2x1"], icon: "bluetooth", on: bt?.enabled ?? false, status: Backends.btShort !== "" ? Backends.btShort : !(bt?.enabled) ? "Off" : btConnected.length > 0 ? btConnected[0].name : "On", page: "bluetooth", act: () => { if (Backends.btFix === "unblock") Backends.fixBluetooth(); else if (bt) bt.enabled = !bt.enabled; } },
            focus: { label: Focus.on ? Focus.mode.name : "Focus", kind: "toggle", sizes: ["2x1", "1x1"], icon: Focus.mode?.icon ?? "moon", on: Focus.on, status: Focus.on && Focus.until > 0 ? Focus.statusText : "", page: "focus", act: () => Focus.on ? Focus.deactivate() : Focus.activate("dnd", 0, false) },
            nightLight: { label: "Night Light", kind: "toggle", sizes: ["1x1", "2x1"], icon: "moon-stars", on: System.nightLight, status: System.nightLight ? System.nightLightTemp + "K" : "Off", needs: "hyprsunset", act: () => System.nightLight = !System.nightLight },
            darkMode: { label: Theme.dark ? "Dark Mode" : "Light Mode", kind: "toggle", sizes: ["1x1", "2x1"], icon: "circle-half", on: Theme.dark, status: Config.options.appearance.mode === "auto" ? "Auto" : "", act: () => System.toggleDarkMode() },
            // Visible to nearby devices, LocalSend and Quick Share together; on = for 10 minutes
            // The circle toggles off ↔ 10 min; the rest (or right-click) offers
            // the three choices, like AirDrop on iOS
            sharing: { label: "Sharing", kind: "toggle", sizes: ["1x1", "2x1"], icon: "share-network", on: Sharing.on, page: "sharing",
                status: Sharing.mixed && Sharing.on ? "Custom" : Sharing.level === "off" ? "No one" : Sharing.level === "10min" ? "Everyone · 10 min" : "Everyone",
                act: () => Sharing.setLevel(Sharing.on ? "off" : "10min"),
                menu: () => [
                    { text: "Send files…", action: () => Panels.pickShare() },
                    { separator: true },
                    { text: "No one", checked: Sharing.level === "off", action: () => Sharing.setLevel("off") },
                    { text: "Everyone for 10 minutes", checked: Sharing.level === "10min" && !Sharing.mixed, action: () => Sharing.setLevel("10min") },
                    { text: "Everyone", checked: Sharing.level === "everyone" && !Sharing.mixed, action: () => Sharing.setLevel("everyone") },
                    { separator: true },
                    { text: "Sharing settings…", openPage: "sharing" }
                ] },
            mirroring: { label: Mirroring.active ? Mirroring.targetName : "Screen Mirroring", kind: "toggle", sizes: ["2x1", "1x1"], icon: "screencast", on: Mirroring.active, status: Mirroring.active ? (Mirroring.state_ === "streaming" ? "Mirroring" : "Connecting…") : "", available: Mirroring.available, reason: "Needs GNOME Network Displays", page: "mirroring", act: () => Mirroring.active ? Mirroring.stop() : Panels.ccPage = "mirroring" },
            captions: { label: "Live Captions", kind: "toggle", sizes: ["1x1", "2x1"], icon: "closed-captioning", on: Captions.enabled, status: Captions.enabled ? "On" : "", act: () => Captions.ready || Captions.enabled ? Captions.toggle() : Panels.openSettings("accessibility") },
            keepAwake: { label: "Caffeine", kind: "toggle", sizes: ["1x1", "2x1"], icon: "coffee", on: System.keepAwake, status: System.keepAwakeStatus, page: "caffeine", act: () => System.setKeepAwake(!System.keepAwake, 0) },
            stageManager: { label: "Stages", kind: "toggle", sizes: ["1x1", "2x1"], icon: "squares-four", on: Config.options.stageManager.enabled, status: "", act: () => Config.options.stageManager.enabled = !Config.options.stageManager.enabled },
            mic: { label: "Microphone", kind: "toggle", sizes: ["1x1", "2x1"], icon: Audio.micMuted ? "microphone-slash" : "microphone", on: !Audio.micMuted, status: Audio.micMuted ? "Muted" : "On", act: () => Audio.toggleMic(true) },
            lowPower: { label: "Low Power", kind: "toggle", sizes: ["1x1", "2x1"], available: Backends.ppd, reason: Backends.powerShort, icon: "leaf", on: PowerProfiles.profile === PowerProfile.PowerSaver, status: "", act: () => Power.setMode(PowerProfiles.profile === PowerProfile.PowerSaver ? "balanced" : "power-saver") },
            coolDown: { label: "Cool-down", kind: "toggle", sizes: ["1x1", "2x1"], icon: "snowflake", on: CoolDown.active, status: CoolDown.why, act: () => CoolDown.toggle() },
            gameMode: { label: "Game Mode", kind: "toggle", sizes: ["1x1", "2x1"], icon: "game-controller", on: System.gameMode, status: "", act: () => System.gameMode = !System.gameMode },
            antiFlashbang: { label: "Anti-Flash", kind: "toggle", sizes: ["1x1", "2x1"], icon: "sun-dim", on: System.antiFlashbang, status: "", act: () => System.antiFlashbang = !System.antiFlashbang },
            record: { label: "Record", kind: "toggle", sizes: ["1x1", "2x1"], icon: "record", on: ScreenTools.recording, status: ScreenTools.recording ? "Recording" : "", available: Recorder.available, act: () => Recorder.openToolbar() }, // the recording toolbar (again: stop)
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
            appLibrary: { label: "App Drawer", kind: "action", sizes: ["1x1", "2x1"], icon: "squares-four", act: () => Panels.show("launchpad") },
            display: { label: "Display", kind: "slider", sizes: ["4x1", "1x2"], icon: "sun", available: Brightness.available, reason: "No adjustable display brightness" },
            sound: { label: "Sound", kind: "slider", sizes: ["4x1", "1x2"], icon: Audio.icon, page: "sound" },
            // (only in Edit Controls to begin with: not in the default layout)
            micLevel: { label: "Microphone level", kind: "slider", sizes: ["4x1", "1x2"], icon: Audio.micMuted ? "microphone-slash" : "microphone", page: "mic", available: AudioStreams.inputs.length > 0, reason: "No microphone found" },
            nowPlaying: { label: "Now Playing", kind: "media", sizes: ["2x2", "4x1"], icon: "music-notes" },
            battery: { label: "Battery", kind: "info", sizes: ["2x1", "1x1"], icon: "battery-high", available: Island.hasBattery, reason: "No battery" },
            resources: { label: "CPU & Memory", kind: "info", sizes: ["2x1", "2x2"], icon: "cpu", page: "resources" },
            weather: { label: "Weather", kind: "info", sizes: ["2x1", "2x2"], icon: Weather.icon }
        })

    readonly property var defaultLayout: ["wifi:2x1", "nowPlaying:2x2", "bluetooth:1x1", "nightLight:1x1", "focus:2x1", "stageManager:1x1", "darkMode:1x1", "display:4x1", "sound:4x1", "screenshot:1x1", "record:1x1", "keepAwake:1x1", "clipboard:1x1", "mirroring:2x1"]

    // ── The saved layout, shared by both looks' editors ─────────────────
    // ("id:WxH"), modules whose tool is missing left out
    readonly property var layout: (Config.options.controlCenter.layout.length > 0 ? Config.options.controlCenter.layout : defaultLayout).map(e => parse(e)).filter(e => usable(e.id))
    function saveLayout(entries) {
        // Keep modules that are only unavailable right now (no display
        // brightness while undocked, a tool not installed yet)
        const saved = Config.options.controlCenter.layout.length > 0 ? Config.options.controlCenter.layout : defaultLayout;
        const resting = saved.filter(e => !usable(parse(e).id));
        Config.options.controlCenter.layout = entries.map(e => e.id + ":" + e.w + "x" + e.h).concat(resting);
    }
    function removeModule(id) {
        saveLayout(layout.filter(e => e.id !== id));
    }
    function setSize(id, size) {
        const [w, h] = size.split("x").map(Number);
        saveLayout(layout.map(e => e.id !== id ? e : { id: id, w: w, h: h }));
    }
    function cycleSize(id) {
        const e = layout.find(e => e.id === id);
        if (!e)
            return;
        const sizes = all[id].sizes;
        setSize(id, sizes[(sizes.indexOf(e.w + "x" + e.h) + 1) % sizes.length]);
    }
    function addModule(id) {
        const [w, h] = all[id].sizes[0].split("x").map(Number);
        saveLayout(layout.concat([{ id: id, w: w, h: h }]));
    }
    function moveModule(id, toIndex) {
        const list = layout.slice();
        const from = list.findIndex(e => e.id === id);
        if (from < 0 || from === toIndex)
            return;
        const [e] = list.splice(from, 1);
        list.splice(Math.max(0, Math.min(list.length, toIndex)), 0, e);
        saveLayout(list);
    }
    // The editor's gallery: everything not in the layout yet, then the ones
    // this computer can't use (greyed, with why)
    readonly property var galleryIds: {
        const ids = Object.keys(all).filter(id => !layout.some(e => e.id === id));
        return ids.filter(id => usable(id)).concat(ids.filter(id => !usable(id)));
    }
    // Where a dragged control lands: the index of the packed cell whose centre
    // is nearest (cx, cy), cells cellW × cellH apart by gap; -1 when empty
    function nearestSlot(packed, cx, cy, cellW, cellH, gap) {
        let best = -1, bestD = 1e9;
        packed.forEach((e, i) => {
            const ex = e.col * (cellW + gap) + (e.w * cellW + (e.w - 1) * gap) / 2;
            const ey = e.row * (cellH + gap) + (e.h * cellH + (e.h - 1) * gap) / 2;
            const d = (ex - cx) * (ex - cx) + (ey - cy) * (ey - cy);
            if (d < bestD) {
                bestD = d;
                best = i;
            }
        });
        return best;
    }

    // ── What the controls do, the same in both looks ────────────────────
    // A control's choices (def.menu) for a PointerMenu; an item with
    // `openPage` opens the control's page
    function menuItems(id, openPage) {
        return all[id].menu().map(it => it.openPage ? Object.assign({}, it, { action: () => openPage(it.openPage) }) : it);
    }
    // The sliders' values: brightness, the microphone, volume
    function sliderValue(id) {
        return id === "display" ? Brightness.value : id === "micLevel" ? (Audio.micMuted ? 0 : Audio.micVolume) : (Audio.muted ? 0 : Audio.volume);
    }
    function setSlider(id, v) {
        if (id === "display")
            Brightness.set(v, true);
        else if (id === "micLevel")
            Audio.setMicVolume(v, true);
        else
            Audio.setVolume(v, true);
    }
    // Arrow keys between controls: the nearest sibling of `from` whose centre
    // lies that way (rows first for up and down, columns for left and right)
    function nearestToward(from, dx, dy) {
        const cx = from.x + from.width / 2, cy = from.y + from.height / 2;
        let best = null, bestD = Infinity;
        for (const m of from.parent?.children ?? []) {
            if (m === from || m.moduleId === undefined || !m.visible)
                continue;
            const mx = m.x + m.width / 2 - cx, my = m.y + m.height / 2 - cy;
            if ((dx !== 0 && Math.sign(mx) !== dx) || (dy !== 0 && Math.sign(my) !== dy))
                continue;
            const d = dx !== 0 ? Math.abs(mx) + 2 * Math.abs(my) : Math.abs(my) + 2 * Math.abs(mx);
            if (d < bestD) {
                bestD = d;
                best = m;
            }
        }
        return best;
    }

    function usable(id) {
        const d = all[id];
        return !!d && d.available !== false && (!d.needs || Tools.has(d.needs));
    }
    // Why a module can't be added here (the gallery shows it greyed out)
    function whyNot(id) {
        const d = all[id];
        if (!d || usable(id))
            return "";
        if (d.available === false)
            return d.reason || "Not available on this computer";
        return "Needs " + d.needs;
    }

    // "id:WxH" -> { id, w, h }
    function parse(entry) {
        const [id, size] = entry.split(":");
        let [w, h] = (size ?? "1x1").split("x").map(Number);
        w = w || 1;
        h = h || 1;
        // A size the module no longer offers (Sharing's 2×2 and 4×1, saved
        // before it became a pill): the largest one it offers that fits
        // inside, else its first
        const sizes = all[id]?.sizes;
        if (sizes && !sizes.includes(w + "x" + h)) {
            const fits = sizes.map(z => z.split("x").map(Number)).filter(z => z[0] <= w && z[1] <= h).sort((a, b) => b[0] * b[1] - a[0] * a[1]);
            [w, h] = fits[0] ?? sizes[0].split("x").map(Number);
        }
        return { id: id, w: w, h: h };
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
