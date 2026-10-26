import QtQuick
import Quickshell.Networking
import qs.common

// Wi-Fi networks: connected first, then known, then by signal. Secured
// networks that aren't known ask for a password inline.
DetailPage {
    id: root

    property var device: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    property var asking: null // network waiting for a password
    property var failed: null // last network that failed to connect

    readonly property var networks: (device?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 12)

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
    onVisibleChanged: if (!visible) asking = null

    function strengthIcon(s) {
        return s > 0.66 ? "wifi-high" : s > 0.33 ? "wifi-medium" : "wifi-low";
    }

    Text {
        visible: !Networking.wifiEnabled || root.networks.length === 0
        width: parent.width
        topPadding: 12
        bottomPadding: 12
        horizontalAlignment: Text.AlignHCenter
        text: !Networking.wifiEnabled ? "Wi-Fi is off" : "Looking for networks…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Repeater {
        model: Networking.wifiEnabled ? root.networks : []

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
                status: parent.modelData.connected ? "Connected" : parent.modelData.state === ConnectionState.Connecting ? "Connecting…" : root.failed === parent.modelData ? "Couldn't connect" : parent.modelData.known ? "Known network" : ""
                trailingIcon: parent.modelData.security !== WifiSecurityType.Open && parent.modelData.security !== WifiSecurityType.Unknown ? "lock-simple" : ""
                busy: parent.modelData.stateChanging
                onClicked: {
                    const n = parent.modelData;
                    if (n.connected)
                        n.disconnect();
                    else if (n.known || n.security === WifiSecurityType.Open)
                        n.connect();
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
                        onVisibleChanged: if (visible) { text = ""; forceActiveFocus(); }
                        onAccepted: join.clicked()
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
                    color: password.text.length >= 8 ? Theme.accent : Theme.color.toggleOff
                    Text {
                        anchors.centerIn: parent
                        text: "Join"
                        color: password.text.length >= 8 ? Theme.color.onAccent : Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    MouseArea { anchors.fill: parent; onClicked: join.clicked() }
                    onClicked: {
                        if (password.text.length < 8)
                            return;
                        root.asking.connectWithPsk(password.text);
                        root.asking = null;
                    }
                }
            }
        }
    }
}
