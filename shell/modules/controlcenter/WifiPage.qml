import QtQuick
import Quickshell.Networking
import qs.common
import qs.services

// Wi-Fi networks: connected first, then known, then by signal. Secured
// networks that aren't known ask for a password inline.
DetailPage {
    id: root

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
    // A menu with a form in it stays open while the pointer is elsewhere
    // (picking a certificate file opens a window over it)
    readonly property bool holding: visible && (form !== null || asking !== null)
    onHoldingChanged: Panels.holdMenus += holding ? 1 : -1
    Component.onDestruction: if (holding) Panels.holdMenus -= 1

    function sortedNow() {
        return (device?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 12);
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
    hasRefresh: true
    onRefreshRequested: {
        if (device) { // a fresh scan
            device.scannerEnabled = false;
            device.scannerEnabled = true;
        }
        refresh(true);
    }
    onAskingChanged: {
        typed = "";
        hint = "";
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

    title: "Wi-Fi"
    hasSwitch: true
    switchOn: Networking.wifiEnabled
    onSwitchToggled: Networking.wifiEnabled = !Networking.wifiEnabled

    // Scan only while this page is open
    Binding {
        target: root.device
        property: "scannerEnabled"
        value: true
        when: root.visible && root.device !== null
        restoreMode: Binding.RestoreBindingOrValue
    }
    Component.onCompleted: refresh(true) // (a page created already showing gets no visibility change)
    onVisibleChanged: {
        if (visible) {
            refresh(true);
        } else {
            asking = null;
            form = null;
        }
    }

    function strengthIcon(s) {
        return s > 0.66 ? "wifi-high" : s > 0.33 ? "wifi-medium" : "wifi-low";
    }

    Text {
        visible: root.form === null && (!Networking.wifiEnabled || root.networks.length === 0)
        width: parent.width
        topPadding: 12
        bottomPadding: 12
        horizontalAlignment: Text.AlignHCenter
        text: !Networking.wifiEnabled ? "Wi-Fi is off" : "Looking for networks…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Repeater {
        model: Networking.wifiEnabled && root.form === null ? root.networks : []

        Column {
            id: entry
            required property var modelData
            width: parent.width

            Connections {
                target: entry.modelData
                ignoreUnknownSignals: true // connectionFailed is newer than Quickshell 0.2
                function onConnectionFailed() { root.failed = entry.modelData; }
                function onConnectedChanged() { if (entry.modelData.connected && root.failed === entry.modelData) root.failed = null; }
            }

            ListRow {
                icon: root.strengthIcon(parent.modelData.signalStrength)
                highlighted: parent.modelData.connected
                title: parent.modelData.name
                status: parent.modelData.connected ? "Connected" : parent.modelData.state === ConnectionState.Connecting || WifiJoin.joining(parent.modelData.name) ? "Connecting…" : root.failed === parent.modelData || WifiJoin.failedFor(parent.modelData.name) !== "" ? "Couldn't connect" : parent.modelData.known ? "Known network" : ""
                trailingIcon: parent.modelData.security !== WifiSecurityType.Open && parent.modelData.security !== WifiSecurityType.Unknown ? "lock-simple" : ""
                busy: parent.modelData.stateChanging || WifiJoin.joining(parent.modelData.name)
                onClicked: {
                    const n = parent.modelData;
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
                visible: root.asking === parent.modelData
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
                visible: root.asking === parent.modelData && root.hint !== ""
                x: 44
                bottomPadding: 6
                text: root.hint
                color: Theme.color.danger
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
            // A network that needs more than a password (a username, a
            // certificate): the full form
            Text {
                visible: root.asking === parent.modelData
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
                visible: text !== "" && root.asking !== parent.modelData
                x: 44
                width: parent.width - 52
                bottomPadding: 6
                wrapMode: Text.Wrap
                text: WifiJoin.failedFor(parent.modelData.name)
                color: Theme.color.danger
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
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
