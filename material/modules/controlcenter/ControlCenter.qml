import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.services
import qs.modules.network

// Control Center: connectivity, quick switches, display and sound, media.
// In the Material look the same place, shortcuts and controls open as
// Android's Quick Settings instead (QuickSettings.qml): only one of the two
// windows is ever created.
Variants {
    model: Quickshell.screens

    Scope {
        id: perScreen
        required property var modelData


        PanelLoader {
            panel: "controlCenter"
            screen: perScreen.modelData
            active: alive && Theme.material

            QuickSettings {
                screen: perScreen.modelData
            }
        }
    }
}
