import QtQuick
import Quickshell
import qs.common
import qs.services

// The emoji picker's window on each screen (EmojiPicker.qml loads this only
// while the shell draws the picker itself, not tessera-ui)
Variants {
    id: slots
    model: Quickshell.screens
    // (tessera-ui draws it: rust.surfaces lists "emoji")
    readonly property bool rust: RustUi.drawing("emoji")

    PanelLoader {
        id: slot
        required property var modelData
        panel: "emoji"
        screen: modelData
        elsewhere: slots.rust
        // Kept for a while after closing: a new window draws every emoji
        // glyph again (Qt rasterises each colour emoji on first use), so
        // reopening within five minutes is instant
        linger: 300000

        PickerWindow {
            screen: slot.modelData
        }
    }
}
