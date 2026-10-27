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

    // ── Problems ─────────────────────────────────────────────────────────
    Repeater {
        model: [Keymap.userError, Keymap.saveError].filter(e => e !== "")
        Group {
            id: problem
            required property string modelData
            width: page.width
            Row {
                label: "Something's wrong with the gestures file"
                detail: problem.modelData
                Icon { name: "warning-circle"; size: 20; color: Theme.color.warning }
            }
        }
    }

    Item {
        width: parent.width
        height: 28
        Text {
            anchors { left: parent.left; leftMargin: 4; right: addButton.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            text: "Swipe or pinch with several fingers. Changes apply right away."
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        SettingsButton {
            id: addButton
            anchors.right: parent.right
            primary: true
            icon: "plus"
            text: "Add gesture"
            enabled: !draft.visible && Keymap.userError === ""
            onClicked: {
                const s = page.freeSpot();
                draft.fingers = s.fingers;
                draft.direction = s.direction;
                draft.action = { shell: "missionControl" };
                draft.visible = true;
            }
        }
    }

    Group {
        visible: page.undo !== null
        width: parent.width
        Row {
            label: page.undo?.text ?? ""
            SettingsButton {
                text: "Undo"
                icon: "arrow-counter-clockwise"
                onClicked: {
                    Keymap.restore(page.undo.snap);
                    page.undo = null;
                    undoTimer.stop();
                }
            }
        }
    }

    // A new gesture, before it's added
    Group {
        id: draft
        visible: false
        width: parent.width
        title: "New gesture"
        property int fingers: 3
        property string direction: "up"
        property var action: ({ shell: "missionControl" })
        readonly property var others: visible ? Keymap.gestureConflicts(fingers, direction, "") : []
        Item {
            width: parent.width
            height: draftCol.implicitHeight + 24
            Column {
                id: draftCol
                x: 14
                y: 12
                width: parent.width - 28
                spacing: 8
                Flow {
                    width: parent.width
                    spacing: 8
                    Choice { menu: pageMenu; width: 104; options: page.fingerOptions; value: draft.fingers; onChosen: v => draft.fingers = v }
                    Choice { menu: pageMenu; width: 136; options: page.directionOptions; value: draft.direction; onChosen: v => draft.direction = v }
                    Icon { height: 28; name: "arrow-right"; size: 14; color: Theme.color.tertiary }
                    Choice { menu: pageMenu; width: 200; options: page.actionOptions; value: draft.action; onChosen: v => draft.action = v }
                }
                Text {
                    visible: draft.others.length > 0
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "Clashes with " + (draft.others[0]?.text ?? "") + " (" + (draft.others[0]?.actionText ?? "") + "). Choose other fingers or another direction."
                    color: Theme.color.warning
                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
                }
                Flow {
                    spacing: 8
                    width: parent.width
                    layoutDirection: Qt.RightToLeft
                    SettingsButton {
                        primary: true
                        text: "Add gesture"
                        enabled: draft.others.length === 0
                        onClicked: {
                            const snap = Keymap.snapshot();
                            Keymap.addGesture({ fingers: draft.fingers, direction: draft.direction, action: draft.action });
                            draft.visible = false;
                            page.offerUndo("Added " + draft.fingers + " fingers, " + Keymap.directionLabel(draft.direction).toLowerCase(), snap);
                        }
                    }
                    SettingsButton { text: "Cancel"; onClicked: draft.visible = false }
                }
            }
        }
    }

    Text {
        visible: !Keymap.ready
        leftPadding: 4
        text: "Loading gestures…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }

    Group {
        visible: Keymap.ready
        width: parent.width
        title: "Gestures"
        footer: "“Left or right” covers both directions, “Any direction” every swipe, “Pinch or spread” both pinches."
        Row {
            visible: Keymap.gestures.length === 0
            label: "No gestures"
            detail: Keymap.gestureDefaults.length === 0 ? "iRice's default gestures weren't found (" + Keymap.hyprDir + "/gestures.json). Reinstall iRice to get them back." : "Add one to switch spaces, open Overview and more."
        }
        Repeater {
            model: Keymap.gestures
            GestureRow {
                required property var modelData
                gesture: modelData
            }
        }
    }

    ImportExport {
        visible: Keymap.ready
        width: parent.width
        sections: ["gestures"]
        title: "Import and export gestures"
    }

    ActionMenu {
        id: pageMenu
    }
}
