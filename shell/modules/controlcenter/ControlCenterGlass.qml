import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.services
import qs.modules.network

// Control Center in the Glass look: the module grid (Tahoe), detail pages in
// a second popover beside it, a footer (you, the battery, Settings, Power,
// Edit Controls), and Edit Controls with the gallery beside the grid. The controls, what they do and the layout are ModuleDefs'.
Popover {
    id: cc
    popoverName: "controlCenter"
    targetX: (screen?.width ?? 1920) - implicitWidth
    radius: Theme.radius.tile + padding // concentric with the modules inside
    // Without the panel background (the default), every control is its
    // own glass chip, like Tahoe. Solid glass always has the panel.
    bare: Theme.glass.tier !== "solid" && !Config.options.controlCenter.background

    readonly property int gap: 10
    readonly property int half: (contentWidth - gap) / 2

    readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wifiNetwork: wifiDevice?.networks.values.find(n => n.connected) ?? null
    readonly property var bt: Bluetooth.defaultAdapter
    readonly property var btConnected: Bluetooth.devices.values.filter(d => d.connected)
    // The open detail page, kept in step with Panels.ccPage both ways
    // (without a binding that the first local change would break)
    property string page: Panels.ccPage
    Connections {
        target: Panels
        function onCcPageChanged() {
            if (Panels.screen === cc.screen)
                cc.page = Panels.ccPage;
        }
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
    onPageChanged: {
        if (page === "")
            subFrom = "";
        if (Panels.screen === cc.screen && Panels.ccPage !== page)
            Panels.ccPage = page;
    }
    dim: page !== "" ? 1 : 0
    // Escape leaves a page or the editor first (as Quick Settings does)
    sideOpen: page !== "" || Panels.ccEditing
    onCloseSide: {
        if (page !== "")
            page = "";
        else
            Panels.ccEditing = false;
    }
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
            height: wifi.visible ? wifi.height : bt.visible ? bt.height : sound.visible ? sound.height : micPage.visible ? micPage.height : focusPage.visible ? focusPage.height : caffeinePage.visible ? caffeinePage.height : resourcesPage.visible ? resourcesPage.height : sharingPage.visible ? sharingPage.height : castPage.visible ? castPage.height : mirroringPage.visible ? mirroringPage.height : 0
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
            MicPage {
                id: micPage
                width: parent.width
                showBack: false
                visible: cc.page === "mic"
            }
            FocusPage {
                id: focusPage
                width: parent.width
                showBack: false
                visible: cc.page === "focus"
            }
            CaffeinePage {
                id: caffeinePage
                width: parent.width
                showBack: false
                visible: cc.page === "caffeine"
            }
            ResourcesPage {
                id: resourcesPage
                width: parent.width
                showBack: false
                visible: cc.page === "resources"
                networkExtra: Component { SpeedTestCard { compact: true; settingsLink: true } }
            }
            SharingPage {
                id: sharingPage
                width: parent.width
                showBack: false
                visible: cc.page === "sharing"
            }
            MirroringPage {
                id: mirroringPage
                width: parent.width
                showBack: false
                visible: cc.page === "mirroring"
            }
            CastPage {
                id: castPage
                width: parent.width
                showBack: false
                visible: cc.page === "cast"
            }
        }
    }

    readonly property bool editing: Panels.ccEditing
    // Only the screen that closed resets the shared state
    onShownChanged: if (!shown && Panels.screen === cc.screen) { Panels.ccEditing = false; page = ""; }
    ModuleDefs { id: moduleDefs }

    // The saved layout and its editing are ModuleDefs' (shared with
    // Material's Quick Settings)
    readonly property var layout: moduleDefs.layout

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
                text: "Click a control to add it. In Quick Settings, click one to change its size, drag to move it, or use − to remove it."
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
            Flickable {
                Momentum {}
                width: parent.width
                height: Math.min(contentHeight, 440)
                contentHeight: gallery.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Column {
                    id: gallery
                    width: parent.width
                    spacing: 2
                    // (then the ones this computer can't use, greyed, with why)
                    Repeater {
                        model: ScriptModel {
                            values: moduleDefs.galleryIds
                        }
                        Squircle {
                            required property string modelData
                            readonly property var def: moduleDefs.all[modelData]
                            readonly property string whyNot: moduleDefs.whyNot(modelData)
                            width: gallery.width
                            height: 40
                            radius: Theme.radius.control
                            color: addMouse.containsMouse && whyNot === "" ? Theme.color.fill : "transparent"
                            Accessible.role: Accessible.Button
                            Accessible.name: def.label + (whyNot !== "" ? ", " + whyNot : "")
                            Rectangle {
                                id: gIcon
                                x: 6
                                anchors.verticalCenter: parent.verticalCenter
                                width: 28
                                height: 28
                                radius: 14
                                color: Theme.color.platter
                                Icon { anchors.centerIn: parent; name: parent.parent.def.icon; size: 14; color: parent.parent.whyNot !== "" ? Theme.color.tertiary : Theme.color.label }
                            }
                            Column {
                                anchors { left: gIcon.right; leftMargin: 10; right: gSizes.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: parent.parent.def.label
                                    color: parent.parent.whyNot !== "" ? Theme.color.secondary : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                                }
                                Text {
                                    width: parent.width
                                    visible: text !== ""
                                    elide: Text.ElideRight
                                    text: parent.parent.whyNot
                                    color: Theme.color.tertiary
                                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
                                }
                            }
                            Text {
                                id: gSizes
                                anchors { right: plus.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                visible: parent.whyNot === ""
                                width: visible ? implicitWidth : 0
                                text: parent.def.sizes.join(" · ").replace(/x/g, "×")
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.caption }
                            }
                            Icon {
                                id: plus
                                anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                                visible: parent.whyNot === ""
                                name: "plus-circle"
                                fill: true
                                size: 18
                                color: Theme.color.success
                            }
                            MouseArea {
                                id: addMouse
                                anchors.fill: parent
                                enabled: parent.whyNot === ""
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: moduleDefs.addModule(parent.modelData)
                            }
                        }
                    }
                }
            }
            Text {
                text: "Reset to default"
                color: Theme.accentInk
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
                    return moduleDefs.nearestSlot(packed, cx, cy, unit, unit, cc.gap);
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
                        unit: grid.unit
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
                        onRemoveRequested: moduleDefs.removeModule(modelData.id)
                        onResizeRequested: moduleDefs.cycleSize(modelData.id)

                        DragHandler {
                            id: drag
                            enabled: cc.editing
                            onActiveChanged: if (!active) moduleDefs.moveModule(mod.modelData.id, grid.dropIndex(mod.x + mod.width / 2, mod.y + mod.height / 2))
                        }
                    }
                }
            }

            // You, the battery, Settings, Power and Edit Controls (Done
            // while editing)
            ControlCenterFooter {
                width: parent.width
                screen: cc.screen
                chip: cc.bare
                editing: cc.editing
                opacity: cc.page !== "" ? 0.45 : 1
                Behavior on opacity { NumberAnimation { duration: 220 } }
            }
        }
    }
}
