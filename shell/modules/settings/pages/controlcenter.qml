import QtQuick
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        title: "Layout"
        footer: "Open Control Center and choose Edit Controls to add, remove, resize and move controls. Controls that need a missing tool stay hidden until it's installed. Game Mode turns off animations, blur, shadows and gaps; Anti-Flash softens very bright screens."
        Row {
            label: "Edit Controls"
            detail: Config.options.controlCenter.layout.length > 0 ? Config.options.controlCenter.layout.length + " controls, arranged by you" : "Tahoe's default arrangement"
            Rectangle {
                width: editLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.accent
                Text { id: editLabel; anchors.centerIn: parent; text: "Edit…"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        Panels.settingsOpen = false;
                        Panels.ccEditing = true;
                        Panels.show("controlCenter");
                    }
                }
            }
        }
        Row {
            label: "Panel background"
            detail: Theme.glass.tier === "solid" ? "Always on with Solid glass" : Config.options.controlCenter.background ? "Controls sit on one glass panel" : "Each control is its own glass chip, like Tahoe"
            Switch {
                enabled: Theme.glass.tier !== "solid"
                checked: Config.options.controlCenter.background || Theme.glass.tier === "solid"
                onToggled: Config.options.controlCenter.background = !checked
            }
        }
        Row {
            visible: Config.options.controlCenter.layout.length > 0
            label: "Reset to the default layout"
            Text {
                text: "Reset"
                color: Theme.accentInk
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Config.options.controlCenter.layout = []
                }
            }
        }
    }
}
