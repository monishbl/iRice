import QtQuick
import Quickshell
import Quickshell.Bluetooth
import qs.common

// The Bluetooth devices, for the Bluetooth menu and Control Center
// (BluetoothPage) and Settings → Bluetooth: connected first, then paired,
// then by name. Click to connect, disconnect or pair; right-click (or press
// and hold) for the device's menu. Discovery runs while the list shows. The
// list scrolls once it's taller than `maxListHeight`.
//   `which`: "all" | "paired" (My devices) | "nearby" (not paired yet)
Column {
    id: root

    width: parent?.width ?? 0
    spacing: 2
    property real maxListHeight: 360 // the menu's cap; Settings: none (its page scrolls)
    property bool inSettings: false // (Device settings… would be this page)
    property string which: "all"
    property bool discover: true // look for new devices while showing

    readonly property var adapter: Bluetooth.defaultAdapter
    // The pointer is over the list (or its menu is open): rows hold still
    readonly property bool hovered: listHover.hovered || menu.shown
    HoverHandler { id: listHover }

    function wanted(d) {
        return which === "paired" ? d.paired : which === "nearby" ? !d.paired && !!d.name : d.paired || !!d.name;
    }
    function sortedNow() {
        return (adapter?.devices.values ?? []).filter(d => wanted(d)).slice().sort((a, b) => (b.connected - a.connected) || (b.paired - a.paired) || (a.name ?? "").localeCompare(b.name ?? "")).slice(0, 40);
    }
    // The rows shown: a snapshot (see WifiList), refreshed every few seconds
    // unless the pointer is over the list, and with the refresh button
    property var devices: []
    function refresh(force) {
        const next = sortedNow();
        if (!force && next.length === devices.length && next.every((d, i) => d === devices[i]))
            return;
        devices = next;
    }
    // Look again (the refresh button)
    function rescan() {
        if (adapter?.enabled) {
            adapter.discovering = false;
            adapter.discovering = true;
        }
        refresh(true);
    }
    Component.onCompleted: refresh(true) // (a list created already showing gets no visibility change)
    onVisibleChanged: {
        if (visible)
            refresh(true);
        else
            menu.close();
    }
    Timer {
        running: root.visible
        interval: 4000
        repeat: true
        onTriggered: if (!root.hovered) root.refresh(false)
    }
    Connections {
        target: root.adapter?.devices ?? null
        ignoreUnknownSignals: true
        function onValuesChanged() {
            const live = root.adapter?.devices.values ?? [];
            if (root.devices.length === 0 || root.devices.some(d => !d || !live.includes(d)))
                root.refresh(false);
        }
    }

    Binding {
        target: root.adapter
        property: "discovering"
        value: true
        when: root.discover && root.visible && (root.adapter?.enabled ?? false)
        restoreMode: Binding.RestoreBindingOrValue
    }

    function deviceIcon(d) {
        const i = d.icon ?? "";
        return i.includes("headset") || i.includes("headphone") ? "headphones" : i.includes("phone") ? "device-mobile" : i.includes("keyboard") ? "keyboard" : i.includes("mouse") ? "mouse" : i.includes("audio") ? "speaker-hifi" : "bluetooth";
    }
    function activate(d) {
        if (d.connected)
            d.disconnect();
        else if (d.paired)
            d.connect();
        else
            d.pair();
    }

    // ── Right-click menu ────────────────────────────────────────────────
    PointerMenu { id: menu }
    function openMenu(d) {
        const items = [];
        if (d.connected)
            items.push({ text: "Disconnect", action: () => d.disconnect() });
        else if (d.paired)
            items.push({ text: "Connect", action: () => d.connect() });
        if (d.pairing)
            items.push({ text: "Stop pairing", action: () => d.cancelPair() });
        else if (!d.paired)
            items.push({ text: "Pair", action: () => d.pair() });
        if (d.paired)
            items.push(d.trusted
                ? { text: "Don't connect automatically", action: () => d.trusted = false }
                : { text: "Connect automatically", action: () => d.trusted = true });
        items.push({ separator: true });
        items.push({ text: "Copy address", action: () => Quickshell.execDetached(["wl-copy", "--", d.address]) });
        if (d.paired)
            items.push({ text: "Forget device", danger: true, action: () => {
                    d.forget();
                    root.refresh(true);
                } });
        if (!root.inSettings) {
            items.push({ separator: true });
            items.push({ text: "Bluetooth settings…", action: () => Panels.openSettings("bluetooth") });
        }
        menu.open(items);
    }

    function statusOf(d) {
        if (d.connected)
            return d.batteryAvailable ? "Connected · " + Math.round(d.battery * 100) + "%" : "Connected";
        if (d.pairing)
            return "Pairing…";
        if (d.paired)
            return "Not connected";
        return "";
    }

    Item {
        width: parent.width
        height: Math.min(listCol.implicitHeight, root.maxListHeight)
        Flickable {
            id: listFlick
            anchors.fill: parent
            contentHeight: listCol.implicitHeight
            interactive: contentHeight > height + 1
            boundsBehavior: Flickable.StopAtBounds
            clip: interactive
            Column {
                id: listCol
                width: listFlick.width
                spacing: 2

                Text {
                    visible: !root.adapter?.enabled || root.devices.length === 0
                    width: parent.width
                    topPadding: 12
                    bottomPadding: 12
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: !root.adapter ? "No Bluetooth adapter. If this computer has one, check that it isn't switched off in its settings or with a key."
                        : !root.adapter.enabled ? "Bluetooth is off"
                        : root.which === "paired" ? "No devices yet. Pair one from Nearby devices."
                        : root.discover ? "Looking for devices…" : "No devices"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }

                Repeater {
                    model: root.adapter?.enabled ? root.devices : []

                    ListRow {
                        required property var modelData
                        icon: root.deviceIcon(modelData)
                        highlighted: modelData.connected
                        title: modelData.name || modelData.address
                        status: root.statusOf(modelData)
                        busy: modelData.state === BluetoothDeviceState.Connecting || modelData.state === BluetoothDeviceState.Disconnecting || modelData.pairing
                        onClicked: root.activate(modelData)
                        onMenuRequested: root.openMenu(modelData)
                    }
                }
            }
        }
        ScrollIndicator {
            target: listFlick
            anchors { right: listFlick.right; top: listFlick.top; bottom: listFlick.bottom }
        }
    }
}
