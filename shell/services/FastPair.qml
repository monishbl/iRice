pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import qs.common

// Earbuds nearby (assets/scripts/fastpair.py): Google Fast Pair earbuds and
// headphones in pairing mode get a card in the island with Connect; with the
// case open, their batteries (left, right, case). Looks only while Bluetooth
// is on, lightly (continuously on the charger). Settings → Bluetooth turns
// it off.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/fastpair.py"
    readonly property bool wanted: Config.options.fastPair.enabled && (Bluetooth.defaultAdapter?.enabled ?? false)

    property var devices: [] // [{ address, name, model, pairing, rssi, battery }]
    KeptProcess {
        id: watcher
        wanted: root.wanted
        command: [Tools.watcher, "python3", "-I", root.script, "watch"]
        stdinEnabled: true // (Tools.watcher: it ends with the shell, and scanning stops)
        stdout: SplitParser {
            onRead: line => {
                try {
                    // (LE adverts carry no name: an address isn't one)
                    root.devices = (JSON.parse(line).devices ?? []).map(d => Object.assign({}, d, { name: /^[0-9A-F]{2}([-:][0-9A-F]{2}){5}$/i.test(d.name ?? "") ? "" : d.name }));
                } catch (e) {}
            }
        }
    }

    // Said once per device for a while (a card that keeps coming back would nag)
    property var offered: ({}) // address -> time
    property var batteryShown: ({}) // address -> time last seen with batteries
    onDevicesChanged: {
        const now = Date.now();
        for (const d of devices) {
            if (d.pairing && d.rssi > -70 && now - (offered[d.address] ?? 0) > 600000 && !paired(d)) {
                offered[d.address] = now;
                Island.post({
                    kind: "custom", icon: "headphones", title: d.name || "Earbuds nearby", subtitle: "Ready to pair with this computer",
                    color: Theme.accent, action: "Connect", onAction: () => root.connect(d)
                }, 15000);
            } else if (d.battery && d.showUi !== false && d.rssi > -60 && ownAudioConnected) {
                // the case was opened (heard again after a quiet spell). Only
                // with your own earbuds connected, and these right here: the
                // advert can't say whose they are (that needs Fast Pair's
                // account keys, which come with Google's pairing)
                if (now - (batteryShown[d.address] ?? 0) > 60000)
                    Island.post({ kind: "custom", icon: "headphones", title: d.name || "Earbuds", subtitle: batteryText(d.battery), color: Theme.accent }, 6000);
                batteryShown[d.address] = now;
            }
        }
    }
    readonly property bool ownAudioConnected: (Bluetooth.devices?.values ?? []).some(b => b.paired && b.connected && /audio|headset|headphone|earbud/i.test(String(b.icon ?? "") + " " + String(b.deviceName ?? b.name ?? "")))
    function paired(d) {
        return (Bluetooth.devices?.values ?? []).some(b => b.paired && b.name && b.name === d.name);
    }
    function batteryText(b) {
        const part = (label, x) => x ? label + " " + x.level + "%" + (x.charging ? " ⚡" : "") : "";
        return [part("L", b.left), part("R", b.right), part("Case", b.case)].filter(x => x).join(" · ");
    }

    // Pair, trust, connect (one at a time)
    property string connecting: ""
    function connect(d) {
        if (connector.running)
            return;
        connecting = d.name || "the earbuds";
        connector.command = ["python3", "-I", script, "connect", d.address];
        connector.running = true;
        Island.post({ kind: "custom", icon: "headphones", title: "Connecting " + connecting + "…", subtitle: "Keep the case open near the computer", color: Theme.accent }, 8000);
    }
    Process {
        id: connector
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                Island.post(r.ok ? { kind: "custom", icon: "headphones", title: root.connecting + " connected", subtitle: "Sound plays there now", color: Theme.accents.green }
                    : { kind: "custom", icon: "headphones", title: "Couldn't connect " + root.connecting, subtitle: r.error || "Try again from Bluetooth settings.", color: Theme.color.danger }, 6000);
                root.connecting = "";
            }
        }
    }
}
