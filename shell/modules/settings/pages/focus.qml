import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Focus modes: what each one lets through, what it changes, and when it
// turns on by itself.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property string selectedId: Focus.modes[0]?.id ?? ""
    readonly property var mode: Focus.modeById(selectedId) ?? Focus.modes[0] ?? null
    property var removed: null // last deleted mode, for Undo
    readonly property var icons: ["moon", "briefcase", "user", "bed", "book-open", "barbell", "car", "game-controller", "heart", "music-notes", "code", "coffee"]
    readonly property var dayNames: ["S", "M", "T", "W", "T", "F", "S"]

    function patch(p) {
        if (mode)
            Focus.setMode(mode.id, p);
    }
    function patchSchedule(p) {
        if (mode)
            Focus.setMode(mode.id, { schedule: Object.assign({}, mode.schedule, p) });
    }

    Group {
        width: parent.width
        footer: "Click a mode to edit it. Turn modes on from Control Center or the menu bar."
        Repeater {
            model: Focus.modes
            Row {
                id: modeRow
                required property var modelData
                label: modelData.name
                detail: (Focus.active === modelData.id ? Focus.statusText + " · " : "") + (modelData.schedule?.enabled ? "Scheduled " + Focus.fmt(modelData.schedule.from) + "–" + Focus.fmt(modelData.schedule.to) : "Manual")
                Icon {
                    name: modelData.icon
                    fill: page.selectedId === modelData.id
                    size: 18
                    color: page.selectedId === modelData.id ? Theme.accent : Theme.color.secondary
                }
                MouseArea {
                    parent: modeRow
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: page.selectedId = modeRow.modelData.id
                }
            }
        }
        Row {
            label: "Add a mode"
            TextField {
                width: 200
                placeholder: "Name, then Enter"
                commitOnBlur: false
                onCommitted: t => {
                    if (t.trim() === "")
                        return;
                    page.selectedId = Focus.addMode(t.trim());
                    text = "";
                }
            }
        }
    }

    // Undo for a deleted mode
    Row {
        visible: page.removed !== null
        label: "Deleted “" + (page.removed?.name ?? "") + "”"
        Text {
            text: "Undo"
            color: Theme.accent
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    Focus.modes = Focus.modes.concat([page.removed]);
                    Focus.save();
                    page.selectedId = page.removed.id;
                    page.removed = null;
                }
            }
        }
        Timer {
            running: page.removed !== null
            interval: 8000
            onTriggered: page.removed = null
        }
    }

    Group {
        visible: page.mode !== null
        width: parent.width
        title: page.mode?.name ?? ""
        Row {
            label: "Name"
            TextField {
                width: 200
                text: page.mode?.name ?? ""
                onCommitted: t => { if (t.trim() !== "") page.patch({ name: t.trim() }); }
            }
        }
        Row {
            label: "Icon"
            Flow {
                width: 6 * 30
                spacing: 2
                Repeater {
                    model: page.icons
                    IconButton {
                        required property string modelData
                        icon: modelData
                        fill: page.mode?.icon === modelData
                        iconColor: page.mode?.icon === modelData ? Theme.accent : Theme.color.secondary
                        onClicked: page.patch({ icon: modelData })
                    }
                }
            }
        }
        Row {
            label: "Allowed apps"
            detail: "Their banners still show. Separate names with commas, e.g. Calendar, Slack."
            TextField {
                width: 220
                text: (page.mode?.allowApps ?? []).join(", ")
                placeholder: "None"
                onCommitted: t => page.patch({ allowApps: t.split(",").map(x => x.trim()).filter(x => x !== "") })
            }
        }
        Row {
            label: "Wallpaper"
            detail: "Shown while the mode is on; yours comes back afterwards."
            TextField {
                width: 220
                text: page.mode?.wallpaper ?? ""
                placeholder: "No change"
                onCommitted: t => page.patch({ wallpaper: t.trim().replace(/^~/, Quickshell.env("HOME")) })
            }
        }
        Row {
            label: "Appearance"
            Segmented {
                width: 240
                readonly property var values: ["", "light", "dark"]
                options: ["No change", "Light", "Dark"]
                current: Math.max(0, values.indexOf(page.mode?.appearance ?? ""))
                onSelected: i => page.patch({ appearance: values[i] })
            }
        }
        Row {
            label: "Schedule"
            detail: "Turns on and off by itself at these times."
            Switch {
                checked: page.mode?.schedule?.enabled ?? false
                onToggled: page.patchSchedule({ enabled: !checked })
            }
        }
        Row {
            visible: page.mode?.schedule?.enabled ?? false
            label: "From"
            detail: "24-hour time, e.g. 22:30"
            TextField {
                width: 90
                text: page.mode?.schedule?.from ?? ""
                onCommitted: t => { if (/^\d{1,2}:\d{2}$/.test(t.trim())) page.patchSchedule({ from: t.trim() }); }
            }
        }
        Row {
            visible: page.mode?.schedule?.enabled ?? false
            label: "To"
            TextField {
                width: 90
                text: page.mode?.schedule?.to ?? ""
                onCommitted: t => { if (/^\d{1,2}:\d{2}$/.test(t.trim())) page.patchSchedule({ to: t.trim() }); }
            }
        }
        Row {
            visible: page.mode?.schedule?.enabled ?? false
            label: "Days"
            Flow {
                width: 7 * 30 + 6 * 4
                spacing: 4
                Repeater {
                    model: 7
                    Rectangle {
                        required property int index
                        readonly property bool on: (page.mode?.schedule?.days ?? []).includes(index)
                        width: 30
                        height: 28
                        radius: Theme.radius.control - 2
                        color: on ? Theme.accent : dayMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                        Text {
                            anchors.centerIn: parent
                            text: page.dayNames[parent.index]
                            color: parent.on ? Theme.color.onAccent : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                        }
                        MouseArea {
                            id: dayMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                const days = page.mode.schedule.days ?? [];
                                page.patchSchedule({ days: parent.on ? days.filter(d => d !== parent.index) : days.concat([parent.index]).sort() });
                            }
                        }
                    }
                }
            }
        }
        Row {
            visible: page.mode?.id !== "dnd"
            label: "Delete this mode"
            Text {
                text: "Delete"
                color: Theme.color.danger
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        page.removed = page.mode;
                        Focus.removeMode(page.mode.id);
                        page.selectedId = Focus.modes[0]?.id ?? "";
                    }
                }
            }
        }
    }
}
