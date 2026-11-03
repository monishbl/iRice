import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Mouse: speed, acceleration, scrolling and buttons for mice (services/
// Mouse.qml applies them per device, so trackpads keep their own).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var cfg: Config.options.mouse

    component Choice: Segmented {
        required property string key
        required property var values
        width: 260
        current: Math.max(0, values.indexOf(page.cfg[key]))
        onSelected: i => page.cfg[key] = values[i]
    }

    Group {
        width: parent.width
        footer: Mouse.mice.length === 0 ? "No mouse is connected. These settings apply when one is." : Mouse.mice.length === 1 ? "Applies to " + Mouse.mice[0] + "." : "Applies to " + Mouse.mice.length + " mice. Trackpads have their own settings."
        Row {
            label: "Tracking speed"
            WordSlider {
                from: -1
                to: 1
                low: "Slow"
                high: "Fast"
                value: page.cfg.speed
                onMoved: v => page.cfg.speed = Math.round(v * 20) / 20
            }
        }
        Row {
            label: "Shake to find the pointer"
            detail: "Move the mouse quickly back and forth: the pointer grows for a moment, or a ring closes in on it. Works with a trackpad too."
            Segmented {
                width: 280
                // (one choice: off, or how it shows; "both" from before shows as Ring)
                readonly property string now: !Config.options.findPointer.enabled || Config.options.findPointer.style === "off" ? "off" : Config.options.findPointer.style === "grow" ? "grow" : "ring"
                readonly property var values: ["off", "grow", "ring"]
                options: ["Off", "Bigger", "Ring"]
                current: Math.max(0, values.indexOf(now))
                onSelected: i => {
                    Config.options.findPointer.enabled = values[i] !== "off";
                    if (values[i] !== "off")
                        Config.options.findPointer.style = values[i];
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Scrolling"
        Row {
            label: "Natural scrolling"
            detail: "Content moves the way the wheel turns"
            Switch { checked: page.cfg.naturalScroll; onToggled: page.cfg.naturalScroll = !checked }
        }
        Row {
            label: "Scroll speed"
            WordSlider {
                from: 0.2
                to: 3
                low: "Slow"
                high: "Fast"
                value: page.cfg.scrollSpeed
                onMoved: v => page.cfg.scrollSpeed = Math.round(v * 10) / 10
            }
        }
    }

    Group {
        width: parent.width
        title: "Buttons"
        Row {
            label: "Primary button"
            detail: "Which button clicks; the other opens menus"
            Segmented {
                width: 160
                options: ["Left", "Right"]
                current: page.cfg.leftHanded ? 1 : 0
                onSelected: i => page.cfg.leftHanded = i === 1
            }
        }
    }

    Group {
        width: parent.width
        title: "Windows"
        footer: "Also counts for trackpads."
        Row {
            label: "Focus"
            detail: page.cfg.focus === "click" ? "A window takes the keyboard when you click it, as on a Mac" : "Workspace changes focus under the pointer. New windows take focus and bring the pointer if needed."
            Choice { key: "focus"; values: ["hover", "click"]; options: ["Follows pointer", "On click"]; width: 260 }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Restore defaults"
            SettingsButton {
                text: "Restore Defaults"
                onClicked: {
                    page.cfg.speed = 0;
                    page.cfg.accelProfile = "adaptive";
                    page.cfg.naturalScroll = false;
                    page.cfg.scrollSpeed = 1;
                    page.cfg.leftHanded = false;
                    page.cfg.middleEmulation = false;
                    page.cfg.focus = "hover";
                    page.cfg.boundaryMode = "corners";
                    page.cfg.cornerSize = 24;
                }
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            Row {
                label: "Pointer acceleration"
                detail: page.cfg.accelProfile === "flat" ? "The pointer moves the same distance however fast you move the mouse. Good for games and precise work." : "Faster movements carry the pointer further"
                Choice { key: "accelProfile"; values: ["adaptive", "flat"]; options: ["On", "Off"]; width: 160 }
            }
        }
        Group {
            width: parent.width
            title: "Buttons"
            Row {
                label: "Middle click with both buttons"
                detail: "Press left and right together, for mice without a wheel button"
                Switch { checked: page.cfg.middleEmulation; onToggled: page.cfg.middleEmulation = !checked }
            }
        }
        Group {
            width: parent.width
            title: "Between displays"
            footer: Mouse.boundaryAvailable ? "Applies to mice and trackpads. Deliberate focus warps still cross displays. During region selection, the pointer stays on the starting screen." : "Pointer boundaries need the updated iRice compositor plugin. Screenshot bounds are protected even without it."
            Row {
                label: "Pointer boundaries"
                Dropdown {
                    width: 260
                    options: [{value: "corners", label: "Protect shared corners"}, {value: "resistance", label: "Edge resistance / push again"}, {value: "capture", label: "Only during screen selection"}, {value: "off", label: "Off"}]
                    value: page.cfg.boundaryMode
                    onChosen: v => page.cfg.boundaryMode = v
                }
            }
            Row {
                visible: page.cfg.boundaryMode === "corners"
                label: "Corner reach"
                detail: page.cfg.cornerSize + " logical pixels from each corner"
                WordSlider { from: 4; to: 100; low: "Small"; high: "Large"; value: page.cfg.cornerSize; onMoved: v => page.cfg.cornerSize = Math.round(v) }
            }
        }
    }

    // ── Parts: other pages shown here (SettingsMap.parts) ──────────────
    PagePart {
        title: "Trackpad"
        source: Qt.resolvedUrl("trackpad.qml")
    }
}
