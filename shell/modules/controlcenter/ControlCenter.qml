import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.services

// Control Center: connectivity, quick switches, display and sound, media.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "controlCenter"
        screen: modelData

        Popover {
            id: cc
            readonly property var modelData: slot.modelData
            screen: modelData
            popoverName: "controlCenter"
            targetX: (screen?.width ?? 1920) - implicitWidth
            radius: Theme.radius.module + padding // concentric with the modules inside
            // Without the panel background (the default), every control is its
            // own glass chip, like Tahoe. Solid glass always has the panel.
            bare: Theme.glass.tier !== "solid" && !Config.options.controlCenter.background

            readonly property int gap: 10
            readonly property int half: (contentWidth - gap) / 2

            readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
            readonly property var wifiNetwork: wifiDevice?.networks.values.find(n => n.connected) ?? null
            readonly property var bt: Bluetooth.defaultAdapter
            readonly property var btConnected: Bluetooth.devices.values.filter(d => d.connected)
            property alias page: pageState.value
            QtObject {
                id: pageState
                property string value: Panels.ccPage
                onValueChanged: Panels.ccPage = value
            }

            // Detail pages (Wi-Fi, Bluetooth, Sound, Focus) open as a second
            // popover beside Control Center, level with the control that
            // opened them; the grid stays put. A press anywhere else or
            // Escape closes it.
            property real subY: 0 // window y of the control that opened it
            property string subFrom: "" // the module it belongs to
            function openSub(name, item) {
                subY = item ? item.mapToItem(null, 0, 0).y : panelRect.y;
                subFrom = item?.moduleId ?? "";
                page = name;
            }
            // Opened from elsewhere (e.g. the menu bar): belongs to the module with that page
            readonly property string focusedModule: page === "" ? "" : subFrom !== "" && moduleDefs.all[subFrom]?.page === page ? subFrom : (layout.find(e => moduleDefs.all[e.id]?.page === page)?.id ?? "")
            onPageChanged: if (page === "") subFrom = ""
            dim: page !== "" ? 1 : 0
            sideOpen: page !== ""
            onCloseSide: page = ""
            onPressedAt: p => {
                if (page !== "" && !(p.x >= sub.x && p.x < sub.x + sub.width && p.y >= sub.y && p.y < sub.y + sub.height))
                    page = "";
            }
            overlay: Item {
                id: sub
                readonly property int pad: 16
                readonly property bool open: cc.page !== ""
                width: 320 + 2 * pad
                height: pages.height + 2 * pad
                x: cc.panelRect.x - width - 10
                y: Math.max(cc.panelRect.y, Math.min(cc.subY - pad, (cc.screen?.height ?? 1080) - height - 12))
                visible: show > 0.01
                property real show: open ? 1 : 0
                Behavior on show { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.004 } }
                opacity: Math.min(1, show)
                transform: [
                    Translate { x: (1 - sub.show) * 12 },
                    Scale { origin.x: sub.width; origin.y: 24; xScale: 0.96 + 0.04 * sub.show; yScale: xScale }
                ]
                onXChanged: subGlass.updatePosition()
                onYChanged: subGlass.updatePosition()
                onHeightChanged: subGlass.updatePosition()

                Glass {
                    id: subGlass
                    anchors.fill: parent
                    anchors.margins: -shadowMargin
                    radius: cc.radius
                }
                // Clicks on the panel's empty parts stay here
                MouseArea { anchors.fill: parent }
                Item {
                    id: pages
                    x: sub.pad
                    y: sub.pad
                    width: 320
                    height: wifi.visible ? wifi.height : bt.visible ? bt.height : sound.visible ? sound.height : focusPage.visible ? focusPage.height : 0
                    Behavior on height { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 } }
                    clip: true
                    WifiPage {
                        id: wifi
                        width: parent.width
                        showBack: false
                        visible: cc.page === "wifi"
                    }
                    BluetoothPage {
                        id: bt
                        width: parent.width
                        showBack: false
                        visible: cc.page === "bluetooth"
                    }
                    SoundPage {
                        id: sound
                        width: parent.width
                        showBack: false
                        visible: cc.page === "sound"
                    }
                    FocusPage {
                        id: focusPage
                        width: parent.width
                        showBack: false
                        visible: cc.page === "focus"
                    }
                }
            }

            readonly property bool editing: Panels.ccEditing
            onShownChanged: if (!shown) { Panels.ccEditing = false; page = ""; }
            ModuleDefs { id: moduleDefs }

            // Saved layout ("id:WxH"), modules whose tool is missing left out
            readonly property var layout: (Config.options.controlCenter.layout.length > 0 ? Config.options.controlCenter.layout : moduleDefs.defaultLayout).map(e => moduleDefs.parse(e)).filter(e => moduleDefs.usable(e.id))
            function saveLayout(entries) {
                Config.options.controlCenter.layout = entries.map(e => e.id + ":" + e.w + "x" + e.h);
            }
            function removeModule(id) {
                saveLayout(layout.filter(e => e.id !== id));
            }
            function cycleSize(id) {
                saveLayout(layout.map(e => {
                    if (e.id !== id)
                        return e;
                    const sizes = moduleDefs.all[id].sizes;
                    const next = sizes[(sizes.indexOf(e.w + "x" + e.h) + 1) % sizes.length];
                    const [w, h] = next.split("x").map(Number);
                    return { id: id, w: w, h: h };
                }));
            }
            function addModule(id) {
                const [w, h] = moduleDefs.all[id].sizes[0].split("x").map(Number);
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

            contentWidth: editing ? 320 + 16 + 260 : 320

            // Without a panel, the add-controls list gets glass of its own
            Glass {
                visible: cc.bare && cc.editing
                x: -cc.padding - shadowMargin
                y: -cc.padding - shadowMargin
                width: addColumn.width + 2 * cc.padding + 2 * shadowMargin
                height: addColumn.height + 2 * cc.padding + 2 * shadowMargin
                shadowMargin: 16
                radius: cc.radius
            }
            Row {
                spacing: 16 + (cc.bare ? 2 * cc.padding : 0)

                // Gallery: everything that isn't in Control Center yet
                Column {
                    id: addColumn
                    visible: cc.editing
                    width: 260
                    spacing: 8
                    Text {
                        text: "Add controls"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        wrapMode: Text.Wrap
                        text: "Click a control to add it. In Control Center, click one to change its size, drag to move it, or use − to remove it."
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                    Flickable {
                        width: parent.width
                        height: Math.min(contentHeight, 440)
                        contentHeight: gallery.implicitHeight
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        Column {
                            id: gallery
                            width: parent.width
                            spacing: 2
                            Repeater {
                                model: Object.keys(moduleDefs.all).filter(id => moduleDefs.usable(id) && !cc.layout.some(e => e.id === id))
                                Rectangle {
                                    required property string modelData
                                    readonly property var def: moduleDefs.all[modelData]
                                    width: gallery.width
                                    height: 40
                                    radius: Theme.radius.control
                                    color: addMouse.containsMouse ? Theme.color.fill : "transparent"
                                    Rectangle {
                                        id: gIcon
                                        x: 6
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 28
                                        height: 28
                                        radius: 14
                                        color: Theme.color.platter
                                        Icon { anchors.centerIn: parent; name: parent.parent.def.icon; size: 14; color: Theme.color.label }
                                    }
                                    Text {
                                        anchors { left: gIcon.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                                        text: parent.def.label
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                                    }
                                    Text {
                                        anchors { right: plus.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                        text: parent.def.sizes.join(" · ").replace(/x/g, "×")
                                        color: Theme.color.tertiary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                                    }
                                    Icon {
                                        id: plus
                                        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                                        name: "plus-circle"
                                        fill: true
                                        size: 18
                                        color: Theme.color.success
                                    }
                                    MouseArea {
                                        id: addMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: cc.addModule(parent.modelData)
                                    }
                                }
                            }
                        }
                    }
                    Text {
                        text: "Reset to default"
                        color: Theme.accent
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Config.options.controlCenter.layout = []
                        }
                    }
                }

                Column {
                    width: 320
                    spacing: 12

                    // The module grid: four columns, packed in order
                    Item {
                        id: grid
                        width: parent.width
                        readonly property real unit: (width - 3 * cc.gap) / 4
                        readonly property var packed: moduleDefs.pack(cc.layout, 4)
                        readonly property int rowsUsed: packed.reduce((m, e) => Math.max(m, e.row + e.h), 0)
                        height: rowsUsed * unit + Math.max(0, rowsUsed - 1) * cc.gap

                        // Where a dragged module would land: the slot under its centre
                        function dropIndex(cx, cy) {
                            let best = 0, bestD = 1e9;
                            packed.forEach((e, i) => {
                                const ex = e.col * (unit + cc.gap) + (e.w * unit + (e.w - 1) * cc.gap) / 2;
                                const ey = e.row * (unit + cc.gap) + (e.h * unit + (e.h - 1) * cc.gap) / 2;
                                const d = (ex - cx) * (ex - cx) + (ey - cy) * (ey - cy);
                                if (d < bestD) {
                                    bestD = d;
                                    best = i;
                                }
                            });
                            return best;
                        }

                        Repeater {
                            model: grid.packed
                            ModuleView {
                                id: mod
                                required property var modelData
                                moduleId: modelData.id
                                cols: modelData.w
                                rows: modelData.h
                                defs: moduleDefs
                                editing: cc.editing
                                chip: cc.bare
                                focused: cc.focusedModule === modelData.id
                                dimmed: cc.page !== "" && !focused
                                readonly property real homeX: modelData.col * (grid.unit + cc.gap)
                                readonly property real homeY: modelData.row * (grid.unit + cc.gap)
                                x: drag.active ? x : homeX
                                y: drag.active ? y : homeY
                                z: drag.active ? 5 : focused ? 4 : 0
                                width: modelData.w * grid.unit + (modelData.w - 1) * cc.gap
                                height: modelData.h * grid.unit + (modelData.h - 1) * cc.gap
                                Behavior on x { enabled: !drag.active; SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 } }
                                Behavior on y { enabled: !drag.active; SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 } }
                                Behavior on width { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 } }
                                Behavior on height { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 } }
                                onOpenPage: page => cc.openSub(page, mod)
                                onRemoveRequested: cc.removeModule(modelData.id)
                                onResizeRequested: cc.cycleSize(modelData.id)

                                DragHandler {
                                    id: drag
                                    enabled: cc.editing
                                    onActiveChanged: if (!active) cc.moveModule(mod.modelData.id, grid.dropIndex(mod.x + mod.width / 2, mod.y + mod.height / 2))
                                }
                            }
                        }
                    }

                    // Edit Controls / Done
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        opacity: cc.page !== "" ? 0.45 : 1
                        Glass {
                            visible: cc.bare && !cc.editing
                            z: -1
                            anchors.fill: parent
                            anchors.margins: -shadowMargin
                            shadowMargin: 10
                            radius: parent.radius
                        }
                        Behavior on opacity { NumberAnimation { duration: 220 } }
                        width: editLabel.implicitWidth + 28
                        height: 28
                        radius: 14
                        color: cc.editing ? Theme.accent : editMouse.containsMouse ? Theme.color.fillHover : cc.bare ? "transparent" : Theme.color.platter
                        border.width: 1
                        border.color: Qt.alpha("white", Theme.dark ? 0.08 : 0.3)
                        Text {
                            id: editLabel
                            anchors.centerIn: parent
                            text: cc.editing ? "Done" : "Edit Controls"
                            color: cc.editing ? Theme.color.onAccent : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                        }
                        MouseArea {
                            id: editMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Panels.ccEditing = !Panels.ccEditing
                        }
                    }
                }
            }
        }
    }
}

