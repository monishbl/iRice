pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Mouse (Settings → Mouse): applied to each mouse as a Hyprland device
// config (`hl.device`), so trackpads keep their own speed, acceleration and
// scrolling. Window focus (click or hover) is Hyprland's input.follow_mouse,
// which counts for every pointer. Re-applied after a config reload and when
// the device list changes.
Singleton {
    id: root

    readonly property var cfg: Config.options.mouse
    property var mice: [] // device names from hyprctl -j devices

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

    function apply() {
        if (!Config.ready)
            return;
        let lua = "hl.config({ input = { follow_mouse = " + (cfg.focus === "hover" ? 1 : 2) + " } })\n";
        for (const m of mice)
            lua += deviceSpec(m) + "\n";
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }
    Timer {
        id: applyLater
        interval: 150 // a slider being dragged applies once it rests
        onTriggered: root.apply()
    }

    // Any setting change
    readonly property string signature: Config.ready ? [cfg.accelProfile, cfg.speed, cfg.naturalScroll, cfg.scrollSpeed, cfg.leftHanded, cfg.middleEmulation, cfg.focus].join("|") : ""
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
                if (names.join("\n") !== root.mice.join("\n")) {
                    root.mice = names;
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

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded")
                applyLater.restart();
        }
    }
}
