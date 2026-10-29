pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

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
        if (cfg.focus === "hover")
            lua += "hl.config({ input = { follow_mouse_threshold = 0 }, cursor = { warp_on_change_workspace = 0, warp_on_monitor_change = 0 } })\n";
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
    function applyBoundaries() {
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

    property string openedWindow: ""
    Timer {
        id: focusLater
        interval: 180
        onTriggered: {
            const address = root.openedWindow;
            root.openedWindow = "";
            if (!Config.ready || root.cfg.focus !== "hover" || System.locked || Panels.open !== "" || ScreenTools.selecting !== "")
                return;
            if (address === "" && root.boundaryAvailable) {
                Quickshell.execDetached(["hyprctl", "irice-glass", "pointer-refocus"]);
                return;
            }
            const script = decodeURIComponent(Qt.resolvedUrl("../assets/scripts/pointer-focus.lua").toString().replace("file://", ""));
            Quickshell.execDetached(["hyprctl", "eval", "local focus = dofile(" + JSON.stringify(script) + "); focus(" + JSON.stringify(address) + ")"]);
        }
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded")
                applyLater.restart();
            if (root.cfg.focus !== "hover")
                return;
            if (event.name === "openwindow") {
                const address = event.data.split(",")[0];
                if (/^(0x)?[0-9a-f]+$/i.test(address)) {
                    root.openedWindow = address.startsWith("0x") ? address : "0x" + address;
                    focusLater.restart();
                }
            } else if (["workspace", "workspacev2", "activespecial", "activespecialv2", "closewindow", "movewindowv2", "changefloatingmode"].includes(event.name)) {
                focusLater.restart();
            }
        }
    }
}
