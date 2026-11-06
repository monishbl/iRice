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

    // The windows (ControlCenterSlots.qml), loaded only while tessera-ui
    // isn't to draw it (RustUi.away); RustBridge above stays
    LazyLoader {
        active: !RustUi.away("controlcenter")
        source: Qt.resolvedUrl("ControlCenterSlots.qml")
    }
}
