import QtQuick
import Quickshell
import qs.common

// Control Center: connectivity, quick switches, display and sound, media.
// Each look draws it its own way: Glass's Control Center (ControlCenterGlass)
// or Material's Quick Settings (QuickSettings), with the same controls,
// actions and layout (ModuleDefs). Only the look's window is ever created.
Variants {
    model: Quickshell.screens

    Scope {
        id: perScreen
        required property var modelData

        PanelLoader {
            panel: "controlCenter"
            screen: perScreen.modelData
            active: alive && !Theme.material

            ControlCenterGlass {
                screen: perScreen.modelData
            }
        }
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
