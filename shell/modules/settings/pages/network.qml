import QtQuick
import QtQuick as QQ
import Quickshell.Networking
import qs.common
import qs.modules.controlcenter
import qs.modules.network
import qs.modules.settings
import qs.services

// Settings → Network: Wi-Fi (the same network list as the Wi-Fi menu, with
// its join form and right-click menu, and the networks you've saved), the
// speed test (with its history), which server it uses, and the details of
// the connection you're on.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property string pageTitle: "Network"
    readonly property var link: SpeedTest.link

    Component.onCompleted: {
        SpeedTest.probe();
        SpeedTest.readLink();
        WifiJoin.loadKnown();
    }
    // The link changes as you move around: read it again now and then
    Timer {
        running: page.visible
        interval: 5000
        repeat: true
        onTriggered: SpeedTest.readLink()
    }

    component Value: Text {
        color: Theme.color.secondary
        horizontalAlignment: Text.AlignRight
        font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
    }

    Group {
        width: parent.width
        title: "Wi-Fi"
        footer: Backends.wifiUnmanaged ? "" : "Right-click a network to connect, disconnect, forget it or copy its name."
        Row {
            visible: !Backends.wifiUnmanaged // (the list below says who manages it instead)
            label: "Wi-Fi"
            detail: !wifiList.device ? "No Wi-Fi adapter found" : Networking.wifiEnabled ? (wifiList.networks.find(n => n.connected)?.name ? "Connected to " + wifiList.networks.find(n => n.connected).name : "On, not connected") : "Off"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: Networking.wifiEnabled && !Backends.wifiUnmanaged
                    text: "Scan again"
                    icon: "arrow-clockwise"
                    onClicked: wifiList.rescan()
                }
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !Backends.wifiUnmanaged
                    checked: Networking.wifiEnabled
                    onToggled: Networking.wifiEnabled = !Networking.wifiEnabled
                }
            }
        }
        Item {
            visible: Networking.wifiEnabled || Backends.wifiUnmanaged
            width: parent.width
            height: wifiList.implicitHeight + 16
            WifiList {
                id: wifiList
                x: 8
                y: 8
                width: parent.width - 16
                maxListHeight: 100000 // (the page scrolls)
                inSettings: true
            }
        }
    }

    Group {
        visible: !Backends.wifiUnmanaged // (NetworkManager's saved networks)
        width: parent.width
        title: "Known networks"
        footer: WifiJoin.forgetError !== "" ? WifiJoin.forgetError : "Networks this computer joins by itself when they're in range. Forget one to stop that and remove its saved password."
        Row {
            visible: WifiJoin.knownNetworks.length === 0
            label: "No saved networks"
            labelColor: Theme.color.secondary
        }
        Repeater {
            model: WifiJoin.knownNetworks
            Row {
                required property var modelData
                label: modelData.ssid
                detail: modelData.name !== modelData.ssid ? "Saved as " + modelData.name : ""
                SettingsButton {
                    text: "Forget"
                    danger: true
                    onClicked: {
                        const live = (wifiList.device?.networks.values ?? []).find(n => n.name === modelData.ssid && n.known);
                        if (live && typeof live.forget === "function") {
                            live.forget();
                            forgetLater.restart();
                        } else {
                            WifiJoin.forget(modelData.ssid);
                        }
                    }
                }
            }
        }
    }
    // (after Quickshell's own forget, read the saved list again)
    Timer {
        id: forgetLater
        interval: 800
        onTriggered: WifiJoin.loadKnown()
    }

    Group {
        width: parent.width
        footer: "Runs only when you click. A test takes about 20 seconds and uses up to " + (SpeedTest.backend === "ookla" ? "a few hundred MB" : "about 370 MB on a fast line, less on a slower one") + ". Under load is your ping while the line is busy: if it's much higher than the idle ping, calls and games lag while something downloads."
        Item {
            width: parent.width
            height: card.implicitHeight + 28
            SpeedTestCard {
                id: card
                x: 14
                y: 14
                width: parent.width - 28
                settingsLink: false
            }
        }
    }

    Group {
        width: parent.width
        title: "Server"
        footer: SpeedTest.backend === "ookla" && !SpeedTest.ooklaReady ? SpeedTest.ooklaInstall
            : SpeedTest.backend === "ookla" ? "Ookla tests against a nearby speedtest.net server, picked automatically." + (SpeedTest.hasOokla ? "" : " Using speedtest-cli; Ookla's official speedtest (paru -S ookla-speedtest-bin) measures more, including latency under load.")
            : "Cloudflare tests against its nearest data centre, using curl. Nothing to install."
        Row {
            label: "Test with"
            detail: SpeedTest.ooklaReady ? (SpeedTest.hasOokla ? "Ookla's speedtest is installed" : "speedtest-cli is installed") : "Ookla needs its speedtest app"
            Segmented {
                width: 220
                enabled: !SpeedTest.running
                readonly property var values: ["cloudflare", "ookla"]
                options: ["Cloudflare", "Ookla"]
                current: Math.max(0, values.indexOf(SpeedTest.backend))
                onSelected: i => Config.options.network.speedTest = values[i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Connection"
        footer: page.link.kind === "wifi" ? "Signal above −67 dBm is good for calls and video; below −75 dBm, move closer to the router or switch to 5 GHz." : ""
        Row {
            visible: !page.link.iface
            label: "Not connected"
            detail: "Connect to Wi-Fi or plug in a cable to see its details"
        }
        Row {
            visible: page.link.kind === "wifi"
            label: "Network"
            Value { text: page.link.ssid ?? "" }
        }
        Row {
            visible: page.link.kind === "wifi" && page.link.signal !== undefined
            label: "Signal"
            QQ.Row {
                spacing: 8
                // Four bars, filled by quality
                QQ.Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Repeater {
                        model: 4
                        Rectangle {
                            required property int index
                            anchors.bottom: parent.bottom
                            width: 4
                            height: 5 + index * 3
                            radius: 1
                            color: (page.link.quality ?? 0) > index / 4 ? Theme.color.label : Theme.color.fill
                        }
                    }
                }
                Value { text: (page.link.signal ?? 0) + " dBm · " + SpeedTest.signalWords(page.link.signal ?? -100) }
            }
        }
        Row {
            visible: page.link.kind === "wifi" && !!page.link.band
            label: "Band"
            Value { text: [page.link.standard, page.link.band, page.link.channel ? "channel " + page.link.channel : "", page.link.width ? page.link.width + " MHz wide" : ""].filter(x => x).join(" · ") }
        }
        Row {
            visible: page.link.kind === "wifi" && (page.link.rx !== undefined || page.link.tx !== undefined)
            label: "Link rate"
            detail: "The most the Wi-Fi link carries right now; your internet is usually slower"
            Value { text: page.link.tx ? "↓ " + Math.round(page.link.rx ?? 0) + " · ↑ " + Math.round(page.link.tx) + " Mbit/s" : Math.round(page.link.rx ?? 0) + " Mbit/s" }
        }
        Row {
            visible: page.link.kind === "wifi" && !!page.link.security
            label: "Security"
            Value { text: page.link.security ?? "" }
        }
        Row {
            visible: page.link.kind === "wired"
            label: "Ethernet"
            Value { text: +(page.link.speed ?? 0) > 0 ? (+page.link.speed >= 1000 ? (+page.link.speed / 1000) + " Gbit/s" : page.link.speed + " Mbit/s") : "Connected" }
        }
        Row {
            visible: !!page.link.iface
            label: "Interface"
            Value { text: page.link.iface ?? "" }
        }
        Row {
            visible: !!(SpeedTest.client.ip || SpeedTest.last?.ip)
            label: "Public IP"
            detail: SpeedTest.client.isp || SpeedTest.last?.isp || ""
            Value { text: SpeedTest.client.ip || SpeedTest.last?.ip || "" }
        }
    }

    Group {
        width: parent.width
        title: "History"
        footer: SpeedTest.history.length > 0 ? "The last 10 tests." : ""
        Row {
            visible: SpeedTest.history.length === 0
            label: "No tests yet"
            detail: "Results appear here after you test your speed"
        }
        Repeater {
            model: SpeedTest.history
            Row {
                required property var modelData
                label: new Date(modelData.at).toLocaleString(Qt.locale(), "d MMM, HH:mm") + (modelData.network ? " · " + modelData.network : "")
                detail: [modelData.backend === "ookla" ? "Ookla" : "Cloudflare", modelData.server.replace(/^Cloudflare /, ""), modelData.ping >= 0 ? "ping " + SpeedTest.formatMs(modelData.ping) : "", modelData.loadedDown >= 0 ? "under load " + SpeedTest.formatMs(modelData.loadedDown) : ""].filter(x => x).join(" · ")
                Value { text: "↓ " + SpeedTest.format(modelData.download) + "   ↑ " + SpeedTest.format(modelData.upload) }
            }
        }
        Row {
            visible: SpeedTest.history.length > 0
            label: "Remove all results"
            SettingsButton {
                text: "Clear history"
                danger: true
                onClicked: SpeedTest.clearHistory()
            }
        }
    }
}
