import QtQuick

// Keycaps for a shortcut, e.g. ["Super", "Shift", "S"], written in the
// chosen style (Settings → Appearance → Shortcut symbols; see KeyNames).
Row {
    id: root

    property var keys: []
    spacing: 4
    Accessible.role: Accessible.StaticText
    Accessible.name: KeyNames.keys(root.keys).join(" ")

    Repeater {
        model: KeyNames.keys(root.keys)
        Rectangle {
            required property string modelData
            width: Math.max(24, label.implicitWidth + 12)
            height: 22
            radius: 6
            color: Theme.color.fill
            border.width: 1
            border.color: Theme.color.separator
            Text {
                id: label
                anchors.centerIn: parent
                text: parent.modelData
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
        }
    }
}
