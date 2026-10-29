//@ pragma UseQApplication
//@ pragma Env QSG_DISTANCEFIELD_ANTIALIASING=gray
// iRice login screen for greetd: qs -p <shell dir>/greeter.qml, run by the
// greeter compositor (see hypr/greeter.lua). Minimal keyboard-first login.
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.modules.greeter

ShellRoot {
    GreeterAuth { id: greeterAuth }

    Variants {
        model: Quickshell.screens
        GreeterSurface {
            required property var modelData
            screen: modelData
            auth: greeterAuth
            // Keyboard and login controls on the focused (or first) screen
            primary: modelData.name === (Hyprland.focusedMonitor?.name ?? Quickshell.screens[0]?.name)
        }
    }
}
