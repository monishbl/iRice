import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

// Privacy & Security: fingerprint and face unlock (biometrics.qml, a part),
// what shows when your screen is shared, and what may run without your
// password: iRice's root helper, Quick Share's direct link, the sharing
// firewall's fix
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    // What on this computer stands in sharing's way, and whether iRice's
    // root helper is there (sharing-check.py, as on Phone & Sharing)
    property var system: ({}) // { firewall: { kind, allowed, blocks, hotspot }, passwordless }
    property string fixing: ""
    property string fixError: ""
    readonly property string checkScript: decodeURIComponent(Qt.resolvedUrl("../../../assets/scripts/sharing-check.py").toString().replace("file://", ""))
    function checkSystem() {
        checkProc.command = ["python3", "-I", checkScript, "status"];
        checkProc.running = true;
    }
    function fixSystem(what) {
        fixing = what;
        fixError = "";
        fixProc.command = ["python3", "-I", checkScript, "fix", what];
        fixProc.running = true;
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
    Process {
        id: fixProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const r = JSON.parse(text);
                    page.fixError = r.ok ? "" : r.error;
                } catch (e) {
                    page.fixError = "It didn't work; try again.";
                }
                page.fixing = "";
                page.checkSystem();
            }
        }
    }
    Component.onCompleted: checkSystem()

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
        Row {
            id: firewallRow
            readonly property bool hotspotOnly: !(page.system.firewall?.blocks ?? false) && page.system.firewall?.hotspot === false
            readonly property bool trouble: (page.system.firewall?.blocks ?? false) || hotspotOnly
            label: "Firewall"
            labelColor: trouble && !hotspotOnly ? Theme.color.danger : Theme.color.label
            detail: !trouble ? (page.system.firewall?.kind ? "Your firewall (" + page.system.firewall.kind + ") lets Quick Share and LocalSend through." : "No firewall stands in sharing's way.")
                : (hotspotOnly ? "Your firewall (" + page.system.firewall.kind + ") keeps phones from joining Quick Share's hotspot, so transfers without a shared Wi-Fi stay slow. Allowing DHCP (port 67) fixes it"
                : (page.system.firewall?.allowed === false ? "Your firewall (" + page.system.firewall.kind + ") blocks Quick Share and LocalSend"
                : "Your firewall (" + (page.system.firewall?.kind ?? "") + ") may block Quick Share and LocalSend")
                + ": phones can't connect to this computer. Allowing ports 53317–53320, mDNS and DHCP fixes it")
                + (page.system.passwordless ? "." : "; it asks for your password.")
                + (page.fixError !== "" && page.fixing === "" ? " " + page.fixError : "")
            SettingsButton {
                visible: firewallRow.trouble
                text: "Allow through firewall"
                icon: "shield-check"
                busy: page.fixing === "firewall"
                enabled: page.fixing === ""
                onClicked: page.fixSystem("firewall")
            }
        }
    }
}
