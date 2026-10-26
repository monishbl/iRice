import QtQuick
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Repeater {
        model: Keybinds.groups
        Group {
            id: group
            required property var modelData
            width: parent.width
            title: modelData.name
            Repeater {
                model: group.modelData.binds
                Row {
                    id: bindRow
                    required property var modelData
                    label: modelData.description
                    KeyCombo { keys: bindRow.modelData.keys }
                }
            }
        }
    }
}
