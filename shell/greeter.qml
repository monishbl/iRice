//@ pragma UseQApplication
//@ pragma Env QSG_DISTANCEFIELD_ANTIALIASING=gray
// iRice login screen for greetd: qs -p <shell dir>/greeter.qml, run by the
// greeter compositor (see hypr/greeter.lua). The wallpaper comes from
// IRICE_GREETER_WALLPAPER (the installer copies yours), else the bundled one.
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.modules.background
import qs.modules.greeter

ShellRoot {
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Fill.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Bold.ttf") }

    // The greeter user's own settings: no desktop widgets, the chosen wallpaper
    Connections {
        target: Config
        function onReadyChanged() {
            if (!Config.ready)
                return;
            Config.options.desktop.clock = false;
            Config.options.desktop.weather = false;
            Config.options.appearance.mode = "dark";
            const wp = Quickshell.env("IRICE_GREETER_WALLPAPER");
            if (wp)
                Config.options.wallpaper.path = wp;
        }
    }

    GreeterAuth { id: greeterAuth }

    Background {}

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
