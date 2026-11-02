import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// "About This Computer": OS, hardware and host name.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "about"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "about"
            contentWidth: 300

            Column {
                width: dialog.contentWidth
                spacing: 6

                IconImage {
                    anchors.horizontalCenter: parent.horizontalCenter
                    implicitSize: 72
                    source: IconLookup.path(SystemInfo.osLogo || "distributor-logo", "computer")
                }
                Item { width: 1; height: 6 }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: SystemInfo.osName
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.display; weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: SystemInfo.hostname
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
                Item { width: 1; height: 12 }

                Repeater {
                    model: [
                        ["Processor", SystemInfo.cpu],
                        ["Graphics", SystemInfo.gpu],
                        ["Memory", SystemInfo.memory],
                        ["Kernel", SystemInfo.kernel]
                    ]
                    Item {
                        required property var modelData
                        visible: modelData[1] !== ""
                        width: parent.width
                        height: 22
                        Text {
                            text: parent.modelData[0]
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        Text {
                            anchors.right: parent.right
                            width: parent.width - 90
                            horizontalAlignment: Text.AlignRight
                            elide: Text.ElideLeft
                            text: parent.modelData[1]
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                        }
                    }
                }
            }
        }
    }
}
