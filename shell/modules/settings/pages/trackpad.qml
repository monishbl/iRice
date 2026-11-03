import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Trackpad: pointing, clicking and scrolling (services/Mouse.qml applies
// them). The gestures are a page of their own (mouse-gestures.qml).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    // ── Pointing, clicking and scrolling (services/Mouse.qml applies) ───
    readonly property var tp: Config.options.trackpad

    Group {
        width: parent.width
        title: "Point and click"
        footer: Mouse.touchpads.length === 0 ? "No trackpad found. These settings apply when one is connected." : ""
        Row {
            label: "Tracking speed"
            WordSlider {
                from: -1
                to: 1
                low: "Slow"
                high: "Fast"
                value: page.tp.speed
                onMoved: v => page.tp.speed = Math.round(v * 20) / 20
            }
        }
        // One choice: off, tap to click, or tap to click and drag
        Row {
            label: "Tap to click"
            detail: !page.tp.tapToClick ? "Press the trackpad down to click" : page.tp.tapAndDrag ? "Tap with one finger to click; tap, then keep the finger down and move it to drag" : "Tap with one finger to click"
            Segmented {
                width: 260
                options: ["Off", "Click", "Click and drag"]
                current: !page.tp.tapToClick ? 0 : page.tp.tapAndDrag ? 2 : 1
                onSelected: i => {
                    if (i > 0)
                        page.tp.tapAndDrag = i === 2;
                    page.tp.tapToClick = i > 0;
                }
            }
        }
        Row {
            label: "Secondary click"
            detail: page.tp.rightClick === "corner" ? "Click in the bottom-right corner" : "Click or tap with two fingers"
            Segmented {
                width: 240
                readonly property var values: ["twoFingers", "corner"]
                options: ["Two fingers", "Bottom right"]
                current: Math.max(0, values.indexOf(page.tp.rightClick))
                onSelected: i => page.tp.rightClick = values[i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Scrolling"
        Row {
            label: "Natural scrolling"
            detail: "Content follows your fingers"
            Switch { checked: page.tp.naturalScroll; onToggled: page.tp.naturalScroll = !checked }
        }
        Row {
            label: "Scroll speed"
            WordSlider {
                from: 0.2
                to: 3
                low: "Slow"
                high: "Fast"
                value: page.tp.scrollSpeed
                onMoved: v => page.tp.scrollSpeed = Math.round(v * 20) / 20
            }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Three-finger drag"
            detail: Keymap.threeFingerDrag ? "Three fingers move the window under them; swipe with four to change spaces" : "Three fingers swipe between spaces"
            Switch {
                checked: Keymap.threeFingerDrag
                onToggled: Keymap.setThreeFingerDrag(!checked)
            }
        }
    }

    Group {
        width: parent.width
        SubpageLink {
            icon: "hand-swipe-right"
            label: "Gestures"
            detail: "Swipe and pinch with several fingers: spaces, All Windows, apps"
            page: "mouse/gestures"
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            title: "Point and click"
            Row {
                label: "Drag lock"
                detail: "A tap-drag keeps holding when you lift your finger, until you tap again"
                enabled: page.tp.tapToClick && page.tp.tapAndDrag
                opacity: enabled ? 1 : 0.5
                Switch { checked: page.tp.dragLock; onToggled: page.tp.dragLock = !checked }
            }
            Row {
                label: "Middle click"
                detail: page.tp.rightClick === "corner" ? "Click both bottom corners together" : "Click or tap with three fingers"
                Switch { checked: page.tp.middleClick; onToggled: page.tp.middleClick = !checked }
            }
            Row {
                label: "Ignore the trackpad while typing"
                detail: "Stops a resting palm from moving the pointer"
                Switch { checked: page.tp.disableWhileTyping; onToggled: page.tp.disableWhileTyping = !checked }
            }
        }
    }
}
