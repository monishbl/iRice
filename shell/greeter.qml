//@ pragma UseQApplication
//@ pragma Env QSG_DISTANCEFIELD_ANTIALIASING=gray
// Tessera login screen for greetd: qs -p <shell dir>/greeter.qml, started by
// assets/scripts/greeter-start.sh in the greeter compositor (hypr/greeter.lua).
// Its style (TESSERA_GREETER_STYLE; Settings → Lock Screen → Login screen):
//   look   this root's look: Glass here, Material in the Material root
//          (modules/greeter/GreeterSurface.qml, each look's own)
//   text   ly's: a box of monospace fields, keyboard first (GreeterText)
//   plain  black, your picture, the field and the fingerprint (GreeterPlain)
// Settings' Preview runs it with TESSERA_GREETER_MOCK=1 (password "tessera").
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.modules.greeter

ShellRoot {
    id: root

    readonly property string style: Quickshell.env("TESSERA_GREETER_STYLE") || "look"

    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Fill.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Bold.ttf") }
    // (the Material look's, in its root)
    readonly property bool materialRoot: Quickshell.shellDir.includes("material")
    FontLoader { source: root.materialRoot ? Qt.resolvedUrl("assets/fonts/GoogleSansFlex.ttf") : "" }
    FontLoader { source: root.materialRoot ? Qt.resolvedUrl("assets/fonts/MaterialSymbolsRounded.ttf") : "" }

    GreeterAuth { id: greeterAuth }

    // Keyboard and login controls on the focused (or first) screen; the
    // others show the same picture, quietly
    function isPrimary(s) {
        return s.name === (Hyprland.focusedMonitor?.name ?? Quickshell.screens[0]?.name);
    }
    Variants {
        model: root.style === "look" ? Quickshell.screens : []
        GreeterSurface {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            primary: root.isPrimary(modelData)
        }
    }
    Variants {
        model: root.style === "text" ? Quickshell.screens : []
        GreeterText {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            primary: root.isPrimary(modelData)
        }
    }
    Variants {
        model: root.style === "plain" ? Quickshell.screens : []
        GreeterPlain {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            primary: root.isPrimary(modelData)
        }
    }
}
