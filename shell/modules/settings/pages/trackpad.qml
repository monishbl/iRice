import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Trackpad: the gestures, each "fingers + direction → action", changed right
// in the row. Defaults come from hypr/irice/gestures.json; changes and your
// own gestures live in keybinds.json next to the keyboard shortcuts (see
// services/Keymap.qml) and apply as soon as they're saved.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property var undo: null // { text, snap }
    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: page.undo = null
    }
    function offerUndo(text, snap) {
        undo = { text: text, snap: snap };
        undoTimer.restart();
    }

    readonly property var fingerOptions: [3, 4, 5].map(n => ({ value: n, label: n + " fingers" }))
    readonly property var directionOptions: Keymap.directions.map(d => ({ value: d.name, label: d.label }))
    readonly property var actionOptions: Keymap.gestureActions.map(a => ({ value: a.name, label: a.label })).concat(Keymap.shellActions.map(a => ({ value: { shell: a.name }, label: a.label })))

    // A free spot for a new gesture
    function freeSpot() {
        for (const f of [3, 4, 5])
            for (const d of ["left", "right", "up", "down", "pinchin", "pinchout"])
                if (Keymap.gestureConflicts(f, d, "").length === 0)
                    return { fingers: f, direction: d };
        return { fingers: 5, direction: "up" };
    }

    function openMenu(anchor, g) {
        const items = [{
            text: g.disabled ? "Turn on" : "Turn off", icon: "power",
            action: () => {
                const snap = Keymap.snapshot();
                Keymap.disableGesture(g.id, !g.disabled);
                page.offerUndo((g.disabled ? "Turned on " : "Turned off ") + g.text, snap);
            }
        }];
        if (g.custom)
            items.push({
                text: "Remove", icon: "trash", danger: true,
                action: () => {
                    const snap = Keymap.snapshot();
                    Keymap.removeGesture(g.id);
                    page.offerUndo("Removed " + g.text, snap);
                }
            });
        else
            items.push({
                text: "Reset to default", icon: "arrow-counter-clockwise", enabled: g.changed,
                action: () => {
                    const snap = Keymap.snapshot();
                    Keymap.resetGesture(g.id);
                    page.offerUndo(g.label + " is back to its default", snap);
                }
            });
        pageMenu.open(anchor, items);
    }

    // ── A gesture: [fingers] [direction] → [action] ──────────────────────
    component GestureRow: Item {
        id: gr
        required property var gesture
        readonly property var others: Keymap.gestureConflicts(gesture.fingers, gesture.direction, gesture.id)
        readonly property var def: gesture.custom ? null : Keymap.gestureDefaults.find(d => d.id === gesture.id) ?? null
        width: parent?.width ?? 0
        height: grCol.implicitHeight + 20

        Column {
            id: grCol
            anchors { left: parent.left; leftMargin: 14; right: grMenu.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            spacing: 6
            Flow {
                width: parent.width
                spacing: 8
                opacity: gr.gesture.disabled ? 0.5 : 1
                Choice {
                    menu: pageMenu
                    width: 104
                    options: page.fingerOptions
                    value: gr.gesture.fingers
                    onChosen: v => Keymap.setGesture(gr.gesture.id, { fingers: v })
                }
                Choice {
                    menu: pageMenu
                    width: 136
                    options: page.directionOptions
                    value: gr.gesture.direction
                    onChosen: v => Keymap.setGesture(gr.gesture.id, { direction: v })
                }
                Icon {
                    height: 28
                    name: "arrow-right"
                    size: 14
                    color: Theme.color.tertiary
                }
                Choice {
                    menu: pageMenu
                    width: 200
                    options: page.actionOptions
                    value: gr.gesture.action
                    placeholder: Keymap.describeGesture(gr.gesture.action)
                    onChosen: v => Keymap.setGesture(gr.gesture.id, { action: v })
                }
            }
            Text {
                width: parent.width
                readonly property string what: gr.gesture.disabled ? "Off" : gr.others.length > 0 ? "Clashes with " + gr.others[0].text + " (" + gr.others[0].actionText + "). Change the fingers or direction." : gr.gesture.changed && gr.def ? "Changed · was " + gr.def.fingers + " fingers, " + Keymap.directionLabel(gr.def.direction).toLowerCase() + " → " + Keymap.describeGesture(gr.def.action) : ""
                visible: what !== ""
                text: what
                wrapMode: Text.Wrap
                color: gr.others.length > 0 && !gr.gesture.disabled ? Theme.color.warning : Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        IconButton {
            id: grMenu
            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            icon: "dots-three"
            iconColor: Theme.color.secondary
            tooltip: "Options for this gesture"
            onClicked: page.openMenu(grMenu, gr.gesture)
        }
    }

    // ── Pointing, clicking and scrolling (services/Mouse.qml applies) ───
    readonly property var tp: Config.options.trackpad
    component TpScale: Column {
        id: tpScale
        property alias from: tpSlider.from
        property alias to: tpSlider.to
        property alias value: tpSlider.value
        property string low
        property string high
        signal moved(real v)
        width: 240
        spacing: 2
        Slider {
            id: tpSlider
            width: parent.width
            onMoved: v => tpScale.moved(v)
        }
        Item {
            width: parent.width
            height: tpLow.implicitHeight
            Text { id: tpLow; text: tpScale.low; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
            Text { anchors.right: parent.right; text: tpScale.high; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
        }
    }

    Group {
        width: parent.width
        title: "Point and click"
        footer: Mouse.touchpads.length === 0 ? "No trackpad found. These settings apply when one is connected." : ""
        Row {
            label: "Tracking speed"
            TpScale {
                from: -1
                to: 1
                low: "Slow"
                high: "Fast"
                value: page.tp.speed
                onMoved: v => page.tp.speed = Math.round(v * 20) / 20
            }
        }
        Row {
            label: "Tap to click"
            detail: "Tap with one finger to click"
            Switch { checked: page.tp.tapToClick; onToggled: page.tp.tapToClick = !checked }
        }
        Row {
            label: "Tap to drag"
            detail: "Tap, then keep the finger down and move it to drag"
            enabled: page.tp.tapToClick
            opacity: enabled ? 1 : 0.5
            Switch { checked: page.tp.tapAndDrag; onToggled: page.tp.tapAndDrag = !checked }
        }
        Row {
            label: "Drag lock"
            detail: "A tap-drag keeps holding when you lift your finger, until you tap again"
            enabled: page.tp.tapToClick && page.tp.tapAndDrag
            opacity: enabled ? 1 : 0.5
            Switch { checked: page.tp.dragLock; onToggled: page.tp.dragLock = !checked }
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
            TpScale {
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
            detail: "Swipe and pinch with several fingers: spaces, Mission Control, apps"
            page: "trackpad/gestures"
        }
    }
}
