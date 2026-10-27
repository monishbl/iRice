import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Power options. Arrow keys move, Enter confirms, Esc cancels.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "session"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "session"
            contentWidth: 5 * 80 + 4 * 12

            property int current: 4
            onShownChanged: current = 4

            readonly property var options: [
                { id: "lock", text: "Lock", icon: "lock" },
                { id: "sleep", text: "Sleep", icon: "moon" },
                { id: "logout", text: "Log Out", icon: "sign-out" },
                { id: "restart", text: "Restart", icon: "arrow-clockwise" },
                { id: "shutdown", text: "Shut Down", icon: "power" }
            ]

            function choose(i) {
                const opt = options[i];
                Panels.close();
                System.power(opt.id);
            }

            Column {
                width: dialog.contentWidth
                spacing: 20

                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "What would you like to do?"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.DemiBold }
                }

                Row {
                    spacing: 12
                    focus: dialog.shown
                    Keys.onLeftPressed: dialog.current = Math.max(0, dialog.current - 1)
                    Keys.onRightPressed: dialog.current = Math.min(dialog.options.length - 1, dialog.current + 1)
                    Keys.onReturnPressed: dialog.choose(dialog.current)
                    Keys.onEnterPressed: dialog.choose(dialog.current)
                    Keys.onEscapePressed: Panels.close()

                    Repeater {
                        model: dialog.options

                        Column {
                            id: option
                            required property var modelData
                            required property int index
                            readonly property bool hot: dialog.current === index
                            width: 80
                            spacing: 8

                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: 64
                                height: 64
                                radius: 32
                                color: option.hot ? (option.modelData.id === "shutdown" || option.modelData.id === "restart" ? Theme.color.danger : Theme.accent) : Theme.color.toggleOff
                                scale: mouse.pressed ? 0.94 : 1
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }

                                Icon {
                                    anchors.centerIn: parent
                                    name: option.modelData.icon
                                    bold: true
                                    size: 26
                                    color: option.hot ? Theme.color.onAccent : Theme.color.label
                                }
                                MouseArea {
                                    id: mouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onEntered: dialog.current = option.index
                                    onClicked: dialog.choose(option.index)
                                }
                            }
                            Text {
                                width: parent.width
                                horizontalAlignment: Text.AlignHCenter
                                text: option.modelData.text
                                color: option.hot ? Theme.color.label : Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                            }
                        }
                    }
                }
            }
        }
    }
}
