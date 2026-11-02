//@ pragma UseQApplication
//@ pragma Env QSG_DISTANCEFIELD_ANTIALIASING=gray
// iRice login screen for greetd: qs -p <shell dir>/greeter.qml, run by the
// greeter compositor (see hypr/greeter.lua). Minimal keyboard-first login,
// in the Glass look or, when /usr/share/irice/look.json says so, the
// Material one (GreeterLook: the greeter can't read your settings).
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.modules.greeter

ShellRoot {
    GreeterAuth { id: greeterAuth }
    GreeterLook { id: greeterLook }

    // The Material look's type and icons (only when it's chosen)
    FontLoader { source: greeterLook.material ? Qt.resolvedUrl("assets/fonts/GoogleSansFlex.ttf") : "" }
    FontLoader { source: greeterLook.material ? Qt.resolvedUrl("assets/fonts/MaterialSymbolsRounded.ttf") : "" }

    Variants {
        model: greeterLook.material ? [] : Quickshell.screens
        GreeterSurface {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            // Keyboard and login controls on the focused (or first) screen
            primary: modelData.name === (Hyprland.focusedMonitor?.name ?? Quickshell.screens[0]?.name)
        }
    }
    Variants {
        model: greeterLook.material ? Quickshell.screens : []
        GreeterMaterial {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            appearance: greeterLook
            primary: modelData.name === (Hyprland.focusedMonitor?.name ?? Quickshell.screens[0]?.name)
        }
    }
}
