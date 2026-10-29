pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import "FocusDecision.js" as FocusDecision

// Mouse and trackpad (Settings → Mouse, Trackpad). Mice: applied to each mouse as a Hyprland device
// config (`hl.device`), so trackpads keep their own speed, acceleration and
// scrolling. Window focus (click or hover) is Hyprland's input.follow_mouse,
// which counts for every pointer. Re-applied after a config reload and when
// the device list changes.
Singleton {
    id: root

    readonly property var cfg: Config.options.mouse
    readonly property var tp: Config.options.trackpad
    property var mice: [] // device names from hyprctl -j devices
    property var touchpads: []

    // Trackpads (and their pointer-stick halves) are not mice here
    function isMouse(name) {
        return !/touchpad|trackpad|ydotool|virtual|wacom|stylus|pen/i.test(name);
    }

    function deviceSpec(name) {
        const sens = Math.max(-1, Math.min(1, cfg.speed));
        return "hl.device({ name = \"" + name.replace(/\\/g, "\\\\").replace(/"/g, "\\\"") + "\""
            + ", accel_profile = \"" + (cfg.accelProfile === "flat" ? "flat" : "adaptive") + "\""
            + ", sensitivity = " + sens.toFixed(2)
            + ", natural_scroll = " + (cfg.naturalScroll ? "true" : "false")
            + ", scroll_factor = " + Math.max(0.2, Math.min(3, cfg.scrollSpeed)).toFixed(2)
            + ", left_handed = " + (cfg.leftHanded ? "true" : "false")
            + ", middle_button_emulation = " + (cfg.middleEmulation ? "true" : "false")
            + " })";
    }

    // Trackpads (Settings → Trackpad): Hyprland's input.touchpad for how they
    // click and scroll, plus each trackpad's own speed
    function touchpadSpec() {
        return "hl.config({ input = { touchpad = {"
            + " tap_to_click = " + (tp.tapToClick ? "true" : "false")
            + ", tap_and_drag = " + (tp.tapAndDrag ? "true" : "false")
            + ", drag_lock = " + (tp.dragLock ? 1 : 0)
            + ", natural_scroll = " + (tp.naturalScroll ? "true" : "false")
            + ", scroll_factor = " + Math.max(0.2, Math.min(3, tp.scrollSpeed)).toFixed(2)
            + ", disable_while_typing = " + (tp.disableWhileTyping ? "true" : "false")
            + ", clickfinger_behavior = " + (tp.rightClick === "corner" ? "false" : "true")
            + ", middle_button_emulation = " + (tp.middleClick ? "true" : "false")
            + " } } })";
    }
    function touchpadDevice(name) {
        return "hl.device({ name = \"" + name.replace(/\\/g, "\\\\").replace(/"/g, "\\\"") + "\", sensitivity = " + Math.max(-1, Math.min(1, tp.speed)).toFixed(2) + " })";
    }

    function apply() {
        if (!Config.ready)
            return;
        let lua = "hl.config({ input = { follow_mouse = " + (cfg.focus === "hover" ? 1 : 2) + ", mouse_refocus = true, focus_on_close = " + (cfg.focus === "hover" ? 1 : 0) + " } })\n";
        for (const m of mice)
            lua += deviceSpec(m) + "\n";
        lua += touchpadSpec() + "\n";
        for (const t of touchpads)
            lua += touchpadDevice(t) + "\n";
        Quickshell.execDetached(["hyprctl", "eval", lua]);
        applyBoundaries();
    }
    Timer {
        id: applyLater
        interval: 150 // a slider being dragged applies once it rests
        onTriggered: root.apply()
    }

    // Any setting change
    readonly property string signature: Config.ready ? [cfg.accelProfile, cfg.speed, cfg.naturalScroll, cfg.scrollSpeed, cfg.leftHanded, cfg.middleEmulation, cfg.focus, tp.speed, tp.tapToClick, tp.tapAndDrag, tp.dragLock, tp.naturalScroll, tp.scrollSpeed, tp.disableWhileTyping, tp.rightClick, tp.middleClick].join("|") : ""
    onSignatureChanged: applyLater.restart()

    Process {
        id: reader
        command: ["hyprctl", "-j", "devices"]
        stdout: StdioCollector {
            onStreamFinished: {
                let d = null;
                try {
                    d = JSON.parse(text);
                } catch (e) {
                    return;
                }
                const names = (d.mice ?? []).map(m => m.name).filter(n => root.isMouse(n));
                const pads = (d.mice ?? []).map(m => m.name).filter(n => /touchpad|trackpad/i.test(n));
                if (names.join("\n") !== root.mice.join("\n") || pads.join("\n") !== root.touchpads.join("\n")) {
                    root.mice = names;
                    root.touchpads = pads;
                    applyLater.restart();
                }
            }
        }
    }
    // Mice come and go (Bluetooth, receivers): look again now and then
    Timer {
        interval: 15000
        running: Config.ready
        repeat: true
        triggeredOnStart: true
        onTriggered: reader.running = true
    }

    readonly property bool boundaryAvailable: GlassPlugin.available && GlassPlugin.version >= 3
    // (4: the plugin moves the pointer onto windows brought forward, and
    // knows when the user last clicked or typed)
    readonly property bool followAvailable: GlassPlugin.available && GlassPlugin.version >= 4
    function applyBoundaries() {
        if (followAvailable && Config.ready)
            GlassPlugin.command("pointer-follow", "irice-glass pointer-follow " + (cfg.focus === "hover" ? 1 : 0));
        if (!boundaryAvailable || !Config.ready)
            return;
        const mode = cfg.boundaryMode === "corners" ? 1 : cfg.boundaryMode === "resistance" ? 2 : 0;
        GlassPlugin.command("pointer-policy", "irice-glass pointer-policy " + mode + " " + Math.max(4, Math.min(100, cfg.cornerSize)));
        confineSelection();
    }
    function confineSelection() {
        if (!boundaryAvailable)
            return;
        const name = cfg.boundaryMode !== "off" && !System.locked && ScreenTools.selecting !== "" ? ScreenTools.selectingScreen?.name : "";
        GlassPlugin.command("pointer-confine", "irice-glass pointer-confine " + (name || "-"));
    }
    readonly property string boundarySignature: cfg.boundaryMode + "|" + cfg.cornerSize
    onBoundarySignatureChanged: applyBoundaries()
    Connections { target: GlassPlugin; function onReady() { root.applyBoundaries(); } }
    Connections { target: ScreenTools; function onSelectingChanged() { root.confineSelection(); } }
    Connections { target: System; function onLockedChanged() { root.confineSelection(); } }
    Timer { // A short lease releases confinement if the shell exits during selection.
        interval: 500; repeat: true
        running: root.boundaryAvailable && ScreenTools.selecting !== "" && root.cfg.boundaryMode !== "off"
        onTriggered: root.confineSelection()
    }

    // ── Focus that follows the pointer ──────────────────────────────────
    // A window opening, a space switching, a window closing: the pointer
    // didn't move, so Hyprland leaves focus where it was. Events are
    // collected for a moment (Hyprland sends several for one change, in no
    // fixed order), then decided once (FocusDecision.afterEvents). A window
    // brought forward by an app or the dock is the plugin's (pointer-follow):
    // the pointer is already on it by then, so a refocus keeps it.
    property var pending: []
    Timer {
        id: focusLater
        interval: 180
        onTriggered: {
            const act = FocusDecision.afterEvents(root.pending);
            root.pending = [];
            if (act.kind === "none" || !Config.ready || root.cfg.focus !== "hover" || System.locked || Panels.open !== "" || ScreenTools.selecting !== "")
                return;
            if (act.kind === "refocus" && root.boundaryAvailable) {
                Quickshell.execDetached(["hyprctl", "irice-glass", "pointer-refocus"]);
                return;
            }
            const script = decodeURIComponent(Qt.resolvedUrl("../assets/scripts/pointer-focus.lua").toString().replace("file://", ""));
            Quickshell.execDetached(["hyprctl", "eval", "local focus = dofile(" + JSON.stringify(script) + "); focus(" + JSON.stringify(act.kind === "warp" ? act.address : "") + ")"]);
        }
    }

    // ── Apps asking to come forward ─────────────────────────────────────
    // Hyprland's "urgent": a window asked to be activated without the right
    // to (an app opened from a link, "Open in VS Code", a sign-in finishing,
    // or a chat app pinging). Right after the user clicked or typed, it's
    // their own request: the window comes forward. Otherwise its dock icon
    // bounces and the island says it wants attention, with Show.
    // When the user last clicked or typed: the plugin knows; without it, any
    // input in the last 3 s (pointer movement counts too).
    IdleMonitor {
        id: recentInput
        enabled: !root.followAvailable
        timeout: 3
        respectInhibitors: false
    }
    property string urgentWindow: ""
    // A window opening asks for attention before Hyprland announces it
    // (urgent, then openwindow): decide a moment later, once it's known
    Timer {
        id: urgentSettle
        interval: 250
        onTriggered: {
            if (root.urgentWindow === "")
                return;
            if (root.followAvailable)
                inputAge.running = true;
            else
                root.urgentDecided(recentInput.isIdle ? Infinity : 0);
        }
    }
    // Classes of windows as they open (from Hyprland's openwindow event): a
    // window asking for attention right as it opens isn't known to the IPC
    // yet, so its class comes from here
    property var openedClass: ({})
    Process {
        id: inputAge
        command: ["hyprctl", "irice-glass", "input-age"]
        stdout: StdioCollector {
            onStreamFinished: root.urgentDecided(parseInt(text.trim()))
        }
    }
    function urgentDecided(age) {
        const address = urgentWindow;
        urgentWindow = "";
        const t = Hyprland.toplevels.values.find(w => "0x" + w.address.replace(/^0x/, "") === address);
        if (!t)
            return;
        // iRice's own windows (Settings, pickers it opened) are only ever
        // opened on purpose, from a click, a shortcut or a command: they
        // come forward, never as an "wants your attention" alert
        // (the Wayland app id: a window that has just opened has no class
        // from Hyprland's IPC yet)
        // A window asking the moment it opens (within 2 s) is just a new
        // window wanting to be seen: it comes forward too
        const opened = openedClass[address];
        const fresh = !!opened && Date.now() - opened.at < 2000;
        if (fresh || (opened?.cls || t.wayland?.appId || t.lastIpcObject?.class || "") === "org.quickshell") {
            if (Hyprland.activeToplevel !== t)
                bringForward(address);
            return;
        }
        const act = FocusDecision.onUrgent(Number.isFinite(age) ? age : Infinity, Hyprland.activeToplevel === t);
        if (act === "bring") {
            bringForward(address);
        } else if (act === "notify") {
            const cls = t.lastIpcObject?.class ?? "";
            const name = Apps.entryFor(cls)?.name || t.title || cls || "An app";
            Island.post({ kind: "custom", icon: "bell-ringing", title: name, subtitle: "Wants your attention", color: Theme.accent, action: "Show", onAction: () => root.bringForward(address) }, 6000);
        }
    }
    // Focus it (switching to its space) and, with hover focus, put the
    // pointer on it so focus stays there
    function bringForward(address) {
        let lua = "hl.dispatch(hl.dsp.focus({ window = " + JSON.stringify("address:" + address) + " }))\n";
        if (cfg.focus === "hover") {
            const script = decodeURIComponent(Qt.resolvedUrl("../assets/scripts/pointer-focus.lua").toString().replace("file://", ""));
            lua += "dofile(" + JSON.stringify(script) + ")(" + JSON.stringify(address) + ")\n";
        }
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded")
                applyLater.restart();
            if (event.name === "openwindow") {
                const f = event.data.split(",");
                const c = Object.assign({}, root.openedClass);
                c["0x" + f[0].replace(/^0x/, "")] = { cls: f[2] ?? "", at: Date.now() };
                const keys = Object.keys(c);
                if (keys.length > 64)
                    delete c[keys[0]];
                root.openedClass = c;
            }
            if (event.name === "urgent") {
                const address = event.data.replace(/^0x/, "");
                if (!/^[0-9a-f]+$/i.test(address) || System.locked)
                    return;
                root.urgentWindow = "0x" + address;
                urgentSettle.restart();
                return;
            }
            if (root.cfg.focus !== "hover")
                return;
            if (event.name === "openwindow" || ["workspace", "workspacev2", "activespecial", "activespecialv2", "closewindow", "movewindowv2", "changefloatingmode"].includes(event.name)) {
                root.pending = root.pending.concat([{ name: event.name, data: event.data }]);
                focusLater.restart();
            }
        }
    }
}
