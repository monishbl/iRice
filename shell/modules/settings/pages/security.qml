import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

// Privacy & Security: fingerprint and face unlock (biometrics.qml, a part),
// what shows when your screen is shared, and what may run without your
// password: iRice's root helper, Quick Share's direct link, the firewall's
// ports for sharing (SharingPorts)
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    // Which firewall is on, and whether iRice's root helper is there
    // (sharing-check.py, as on Phone & Sharing)
    property var system: ({}) // { firewall: { kind, open, why, error }, passwordless }
    readonly property string checkScript: decodeURIComponent(Qt.resolvedUrl("../../../assets/scripts/sharing-check.py").toString().replace("file://", ""))
    function checkSystem() {
        checkProc.command = ["python3", "-I", checkScript, "status"];
        checkProc.running = true;
    }
    Process {
        id: checkProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    page.system = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Component.onCompleted: checkSystem()
    // (the ports opened or closed while the page shows)
    Connections {
        target: SharingPorts
        function onBusyChanged() {
            if (!SharingPorts.busy)
                page.checkSystem();
        }
    }

    // Fingerprint and Face (both shown only with a reader or an IR camera)
    PagePart {
        source: Qt.resolvedUrl("biometrics.qml")
    }

    Group {
        width: parent.width
        title: "Privacy"
        Row {
            label: "Screen sharing shows"
            detail: !Config.options.island.enabled ? "The island is off, so a share shows as the purple dot." : Config.options.privacy.screenShareIndicator === "island" ? "In the island while an app shares your screen, with a Stop button; no dot meanwhile." : Config.options.privacy.screenShareIndicator === "dot" ? "A purple dot with the other privacy dots; click it to see what's shared or stop it." : "In the island and as a purple dot."
            Segmented {
                width: 240
                readonly property var values: ["island", "dot", "both"]
                options: ["Island", "Dot", "Both"]
                current: Math.max(0, values.indexOf(Config.options.privacy.screenShareIndicator))
                onSelected: i => Config.options.privacy.screenShareIndicator = values[i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Without your password"
        footer: "Only for an administrator at this computer, in the session on screen. Everything else iRice does as an administrator asks for your password."
        Row {
            label: "iRice's helper"
            detail: page.system.passwordless ? "Installed: the login screen's wallpaper and look, and the fixes below, don't ask for your password"
                : "Not installed yet: the login screen's wallpaper and the fixes below ask for your password. The next update installs it."
        }
        Row {
            visible: Config.options.sharing.quickShareHotspot !== false
            label: "Fast transfers without asking"
            detail: QuickShare.trustError !== "" ? QuickShare.trustError
                : QuickShare.trusted ? "Quick Share's direct Wi-Fi link is set up without your password. Any app running as you could set one up too; turn this off to be asked again."
                : "Off: your password is asked each time Quick Share sets up a direct Wi-Fi link. Changing this asks for it once."
            labelColor: QuickShare.trustError !== "" ? Theme.color.danger : Theme.color.label
            Switch {
                enabled: !QuickShare.trustBusy
                checked: QuickShare.trusted
                onToggled: QuickShare.setTrusted(!checked)
            }
        }
        // Sharing's ports: open only while you're visible to others or
        // sharing, closed otherwise (SharingPorts)
        Row {
            readonly property string kind: page.system.firewall?.kind ?? ""
            readonly property var fw: page.system.firewall ?? ({})
            label: "Firewall"
            labelColor: SharingPorts.error !== "" ? Theme.color.danger : Theme.color.label
            detail: SharingPorts.error !== "" ? SharingPorts.error
                : kind === "" ? "No firewall stands in sharing's way."
                : (fw.open ?? []).length > 0 ? "Your firewall (" + kind + ") lets Quick Share and LocalSend through now, while you're " + (fw.why || "sharing") + ", and closes again after."
                : "Your firewall (" + kind + ") lets Quick Share and LocalSend through only while you're visible to others or sharing." + (fw.asked ? " Closed now." : "")
            SettingsButton {
                visible: SharingPorts.error !== ""
                text: SharingPorts.failedWith === 64 ? "Open Software Update" : "Try again"
                icon: SharingPorts.failedWith === 64 ? "arrow-circle-up" : "shield-check"
                busy: SharingPorts.busy
                onClicked: SharingPorts.failedWith === 64 ? Panels.openSettings("update") : SharingPorts.retry()
            }
        }
    }
}
