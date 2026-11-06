import QtQuick
import Quickshell
import qs.common
import qs.services

// Control Center's window on each screen, in the look's drawing
// (ControlCenter.qml loads this only while the shell draws it itself)
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
