import QtQuick
import Quickshell
import Quickshell.Networking
import qs.common
import qs.services

// The Wi-Fi networks, for the Wi-Fi menu and Control Center (WifiPage) and
// Settings → Network: connected first, then known, then by signal. Secured
// networks that aren't known ask for a password inline; enterprise and
// hidden ones get the full form (WifiJoinForm). Right-click (or press and
// hold) a network for its menu: connect, disconnect, forget, details. The
// list scrolls once it's taller than `maxListHeight`.
Column {
    id: root

    width: parent?.width ?? 0
    spacing: 2
    property real maxListHeight: 360 // the menu's cap; Settings: none (its page scrolls)
    property bool inSettings: false // (Network details… would be this page)

    property var device: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    property var asking: null // network waiting for a password
    property var failed: null // last network that failed to connect
    // The full join form (WifiJoinForm), in place of the list: an enterprise
    // network, "Other options…" under the password, or "Join other network…"
    // { ssid, hidden, security }; null = the list
    property var form: null
    function enterprise(n) {
        return [WifiSecurityType.Wpa2Eap, WifiSecurityType.WpaEap, WifiSecurityType.Wpa3SuiteB192, WifiSecurityType.Leap, WifiSecurityType.DynamicWep].includes(n?.security);
    }
    function secured(n) {
        return n.security !== WifiSecurityType.Open && n.security !== WifiSecurityType.Unknown;
    }
    // A menu with a form in it stays open while the pointer is elsewhere
    // (picking a certificate file opens a window over it)
    readonly property bool holding: visible && (form !== null || asking !== null)
    onHoldingChanged: Panels.holdMenus += holding ? 1 : -1
    Component.onDestruction: if (holding) Panels.holdMenus -= 1
    // The pointer is over the list (or its menu is open): rows hold still
    readonly property bool hovered: listHover.hovered || menu.shown
    HoverHandler { id: listHover }

    function sortedNow() {
        return (device?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 30);
    }
    // The rows shown: a snapshot, not a live sort. Re-sorting on every signal
    // change rebuilt every row under the pointer (clicks landed on rows that
    // were gone, and a password being typed was wiped). It's refreshed every
    // few seconds, not while the pointer is over the list or a password is
    // being typed, and with the refresh button; a network that disappears
    // is dropped at once.
    property var networks: []
    function refresh(force) {
        const next = sortedNow();
        if (!force && next.length === networks.length && next.every((n, i) => n === networks[i]))
            return; // same rows, same order: nothing rebuilds
        networks = next;
    }
    // A fresh scan (the refresh button)
    function rescan() {
        if (device) {
            device.scannerEnabled = false;
            device.scannerEnabled = true;
        }
        refresh(true);
    }
    Timer {
        running: root.visible
        interval: 4000
        repeat: true
        onTriggered: if (!root.hovered && !root.asking && !root.form) root.refresh(false)
    }
    Connections {
        target: root.device?.networks ?? null
        ignoreUnknownSignals: true
        function onValuesChanged() {
            const live = root.device?.networks.values ?? [];
            // First results, or a shown network went away: don't wait. But not
            // while a password is being typed: a phone hotspot drops out of the
            // scan and comes back every few seconds, and rebuilding the rows
            // under the pointer swallowed the click on Join
            if (root.asking || root.form)
                return;
            if (root.networks.length === 0 || root.networks.some(n => !n || !live.includes(n)))
                root.refresh(false);
        }
    }
    onAskingChanged: {
        typed = "";
        hint = "";
        Qt.callLater(revealAsking);
    }
    property string hint: "" // why Join didn't go ahead
    property string typed: "" // the password being typed (outlives a row rebuild)
    // The network to join, as the scan has it now: one that dropped out and
    // came back (hotspots do) is a new object with the same name
    function live(n) {
        const all = device?.networks.values ?? [];
        return all.includes(n) ? n : all.find(x => x.name === n?.name) ?? n;
    }
    // WPA needs 8 to 63 characters; WEP keys are 5 or 13 (or 10 or 26 in hex)
    function passwordOk(t) {
        return t.length >= 8 && t.length <= 63 || [5, 10, 13, 26].includes(t.length);
    }
    // The password prompt that just opened scrolls into view
    function revealAsking() {
        const row = asking ? rows.itemAt(networks.indexOf(asking)) : null;
        if (!row)
            return;
        const y = row.mapToItem(listCol, 0, 0).y;
        if (y < listFlick.contentY)
            listFlick.contentY = y;
        else if (y + row.height > listFlick.contentY + listFlick.height)
            listFlick.contentY = Math.min(listFlick.contentHeight - listFlick.height, y + row.height - listFlick.height);
    }

    // Scan only while the list shows
    Binding {
        target: root.device
        property: "scannerEnabled"
        value: true
        when: root.visible && root.device !== null
        restoreMode: Binding.RestoreBindingOrValue
    }
    Component.onCompleted: refresh(true) // (a list created already showing gets no visibility change)
    onVisibleChanged: {
        if (visible) {
            refresh(true);
        } else {
            asking = null;
            form = null;
            menu.close();
        }
    }

    function strengthIcon(s) {
        return s > 0.66 ? "wifi-high" : s > 0.33 ? "wifi-medium" : "wifi-low";
    }

    // ── Right-click menu ────────────────────────────────────────────────
    PointerMenu { id: menu }
    function openMenu(n) {
        const items = [];
        if (n.connected)
            items.push({ text: "Disconnect", action: () => n.disconnect() });
        else if (n.known || n.security === WifiSecurityType.Open)
            items.push({ text: "Connect", action: () => root.live(n).connect() });
        if (!n.connected && secured(n))
            items.push({ text: enterprise(n) ? "Connect with sign-in…" : "Connect with password…", action: () => {
                    if (root.enterprise(n))
                        root.form = { ssid: n.name, hidden: false, security: "enterprise" };
                    else
                        root.asking = n;
                } });
        items.push({ separator: true });
        items.push({ text: "Copy name", action: () => Quickshell.execDetached(["wl-copy", "--", n.name]) });
        if (n.known)
            items.push({ text: "Forget network", danger: true, action: () => root.forget(n) });
        if (!root.inSettings) {
            items.push({ separator: true });
            items.push({ text: "Network details…", action: () => Panels.openSettings("network") });
        }
        menu.open(items);
    }
    // Forget: Quickshell's own when it has one, else every saved profile for
    // that name through NetworkManager
    function forget(n) {
        if (typeof n.forget === "function")
            n.forget();
        else
            WifiJoin.forget(n.name);
        WifiJoin.loadKnown();
        refresh(true);
    }

    // ── The networks, scrolling once there are more than fit ───────────
    Item {
        width: parent.width
        height: Math.min(listCol.implicitHeight, root.maxListHeight)
        visible: root.form === null
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
                    visible: !Networking.wifiEnabled || root.networks.length === 0
                    width: parent.width
                    topPadding: 12
                    bottomPadding: 12
                    horizontalAlignment: Text.AlignHCenter
                    text: !root.device ? "No Wi-Fi adapter" : !Networking.wifiEnabled ? "Wi-Fi is off" : "Looking for networks…"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }

                Repeater {
                    id: rows
                    model: Networking.wifiEnabled && root.form === null ? root.networks : []

                    Column {
                        id: entry
                        required property var modelData
                        width: listCol.width

                        Connections {
                            target: entry.modelData
                            ignoreUnknownSignals: true // connectionFailed is newer than Quickshell 0.2
                            function onConnectionFailed() { root.failed = entry.modelData; }
                            function onConnectedChanged() { if (entry.modelData.connected && root.failed === entry.modelData) root.failed = null; }
                        }

                        ListRow {
                            icon: root.strengthIcon(entry.modelData.signalStrength)
                            highlighted: entry.modelData.connected
                            title: entry.modelData.name
                            status: entry.modelData.connected ? "Connected" : entry.modelData.state === ConnectionState.Connecting || WifiJoin.joining(entry.modelData.name) ? "Connecting…" : root.failed === entry.modelData || WifiJoin.failedFor(entry.modelData.name) !== "" ? "Couldn't connect" : entry.modelData.known ? "Known network" : ""
                            trailingIcon: root.secured(entry.modelData) ? "lock-simple" : ""
                            busy: entry.modelData.stateChanging || WifiJoin.joining(entry.modelData.name)
                            onMenuRequested: root.openMenu(entry.modelData)
                            onClicked: {
                                const n = entry.modelData;
                                if (n.connected)
                                    n.disconnect();
                                else if (n.known || n.security === WifiSecurityType.Open)
                                    n.connect();
                                else if (root.enterprise(n))
                                    root.form = { ssid: n.name, hidden: false, security: "enterprise" };
                                else
                                    root.asking = root.asking === n ? null : n;
                            }
                        }

                        // Inline password prompt
                        Row {
                            visible: root.asking === entry.modelData
                            x: 44
                            spacing: 8
                            height: visible ? 40 : 0

                            Rectangle {
                                width: 180
                                height: 30
                                anchors.verticalCenter: parent.verticalCenter
                                radius: Theme.radius.control
                                color: Theme.color.fill
                                border.width: 1
                                border.color: password.activeFocus ? Theme.accent : Theme.color.separator

                                TextInput {
                                    id: password
                                    anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
                                    verticalAlignment: TextInput.AlignVCenter
                                    echoMode: TextInput.Password
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                                    text: root.typed
                                    onTextChanged: root.typed = text
                                    onVisibleChanged: if (visible) forceActiveFocus()
                                    Component.onCompleted: if (visible) forceActiveFocus()
                                    onAccepted: join.clicked()
                                    Keys.onEscapePressed: root.asking = null
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        visible: password.text === ""
                                        text: "Password"
                                        color: Theme.color.tertiary
                                        font: password.font
                                    }
                                }
                            }
                            Rectangle {
                                id: join
                                signal clicked
                                width: 56
                                height: 30
                                anchors.verticalCenter: parent.verticalCenter
                                radius: Theme.radius.control
                                readonly property bool ready: root.passwordOk(password.text)
                                color: !ready ? Theme.color.toggleOff : joinMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent
                                Text {
                                    anchors.centerIn: parent
                                    text: "Join"
                                    color: join.ready ? Theme.color.onAccent : Theme.color.tertiary
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: joinMouse
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: join.clicked()
                                }
                                onClicked: {
                                    if (!ready) {
                                        root.hint = password.text === "" ? "Type the password first." : "Wi-Fi passwords are at least 8 characters.";
                                        return;
                                    }
                                    const n = root.live(root.asking);
                                    root.failed = null;
                                    n.connectWithPsk(password.text);
                                    root.asking = null;
                                }
                            }
                        }
                        Text {
                            visible: root.asking === entry.modelData && root.hint !== ""
                            x: 44
                            bottomPadding: 6
                            text: root.hint
                            color: Theme.color.danger
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        // A network that needs more than a password (a username, a
                        // certificate): the full form
                        Text {
                            visible: root.asking === entry.modelData
                            x: 44
                            bottomPadding: 8
                            text: "Other options…"
                            color: otherMouse.containsMouse ? Theme.accentInk : Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small; underline: otherMouse.containsMouse }
                            MouseArea {
                                id: otherMouse
                                anchors { fill: parent; margins: -4 }
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    const n = root.asking;
                                    root.asking = null;
                                    root.form = { ssid: n.name, hidden: false, security: "enterprise" };
                                }
                            }
                        }
                        // Joining through WifiJoin went wrong: why, under the row
                        Text {
                            visible: text !== "" && root.asking !== entry.modelData
                            x: 44
                            width: parent.width - 52
                            bottomPadding: 6
                            wrapMode: Text.Wrap
                            text: WifiJoin.failedFor(entry.modelData.name)
                            color: Theme.color.danger
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }
            }
        }
        ScrollIndicator {
            target: listFlick
            anchors { right: listFlick.right; top: listFlick.top; bottom: listFlick.bottom }
        }
    }

    // A network that hides its name
    ListRow {
        visible: Networking.wifiEnabled && root.form === null
        icon: "plus"
        title: "Join other network…"
        onClicked: {
            root.asking = null;
            root.form = { ssid: "", hidden: true, security: "psk" };
        }
    }

    Loader {
        active: root.form !== null
        visible: active
        width: parent.width
        sourceComponent: WifiJoinForm {
            ssid: root.form?.ssid ?? ""
            hidden: root.form?.hidden ?? false
            security: root.form?.security ?? "enterprise"
            onDone: {
                root.form = null;
                root.refresh(true);
            }
        }
    }
}
