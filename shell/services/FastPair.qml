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

    property var devices: [] // [{ address, name, model, pairing, rssi, battery, known: { address, name, connected } }]
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
        // (keyed by Bluetooth address, which earbuds change every ~15
        // minutes: entries past their 10 minutes go, or they'd pile up)
        for (const a in offered)
            if (now - offered[a] > 600000)
                delete offered[a];
        for (const a in batteryShown)
            if (now - batteryShown[a] > 600000)
                delete batteryShown[a];
        for (const d of devices) {
            if (d.pairing && d.rssi > -70 && now - (offered[d.address] ?? 0) > 600000 && !paired(d)) {
                offered[d.address] = now;
                // (`known`: paired with this computer before, from another
                // address: they're connected, not paired again)
                Island.post({
                    kind: "custom", icon: "headphones", title: d.known?.name || d.name || "Earbuds nearby",
                    subtitle: d.known ? "Paired with this computer, not connected" : "Ready to pair with this computer",
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
        return d.known?.connected || (Bluetooth.devices?.values ?? []).some(b => b.paired && b.connected && b.name && (b.name === d.name || b.name === d.known?.name));
    }
    function batteryText(b) {
        const part = (label, x) => x ? label + " " + x.level + "%" + (x.charging ? " ⚡" : "") : "";
        return [part("L", b.left), part("R", b.right), part("Case", b.case)].filter(x => x).join(" · ");
    }

    // Pair, trust, connect (one at a time). "Connecting…" stays up until it's
    // done (pairing can take a minute): then the result takes its place
    property string connecting: ""
    property int connectingUid: 0
    property var connectingDevice: null
    function connect(d) {
        run(d, ["connect", d.address, d.known?.name || d.name || ""]);
    }
    // Paired before, but they won't take the old pairing (reset earbuds):
    // forget it and pair afresh
    function repair(d, known) {
        run(d, ["repair", d.address, known, d.known?.name || d.name || ""]);
    }
    function run(d, args) {
        if (connector.running)
            return;
        connecting = d.known?.name || d.name || "the earbuds";
        connectingDevice = d;
        connector.command = ["python3", "-I", script].concat(args);
        connector.running = true;
        connectingUid = Island.post({ kind: "custom", icon: "headphones", title: "Connecting " + connecting + "…", subtitle: "Keep the case open near the computer", color: Theme.accent }, 120000) || 0;
    }
    Process {
        id: connector
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                if (root.connectingUid)
                    Island.remove(root.connectingUid);
                const d = root.connectingDevice;
                Island.post(r.ok ? { kind: "custom", icon: "headphones", title: root.connecting + " connected", subtitle: "Sound plays there now", color: Theme.accents.green }
                    : Object.assign({ kind: "custom", icon: "headphones", title: "Couldn't connect " + root.connecting, subtitle: r.error || "Try again from Bluetooth settings.", color: Theme.color.danger },
                        r.known && d ? { action: "Pair again", onAction: () => root.repair(d, r.known) } : {}), r.known ? 15000 : 6000);
                root.connecting = "";
                root.connectingUid = 0;
                root.connectingDevice = null;
            }
        }
    }
}
