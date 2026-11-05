import QtQuick
import Quickshell
import qs.common
import qs.services

// Control Center: connectivity, quick switches, display and sound, media.
// Each look draws it its own way: Glass's Control Center (ControlCenterGlass)
// or Material's Quick Settings (QuickSettings), with the same controls,
// actions and layout (ModuleDefs). Only the look's window is ever created.
// Drawn in Rust instead (rust.surfaces: controlcenter): both step aside
// and RustBridge tells tessera-ui what only this shell knows.
Scope {
    RustBridge {}

    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData

            PanelLoader {
                panel: "controlCenter"
                screen: perScreen.modelData
                active: alive && !Theme.material
                elsewhere: RustUi.drawing("controlcenter")

                ControlCenterGlass {
                    screen: perScreen.modelData
                }
            }
            PanelLoader {
                panel: "controlCenter"
                screen: perScreen.modelData
                active: alive && Theme.material
                elsewhere: RustUi.drawing("controlcenter")

                QuickSettings {
                    screen: perScreen.modelData
                }
            }
        }
    }
}
