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
        // tessera-ui draws it with the power menu (rust.surfaces: session):
        // this says when it opens, where and what it shows, and hears when it
        // closed itself (the first screen's loader speaks for all)
        elsewhere: RustUi.drawing("session")
        readonly property bool speaks: modelData === Quickshell.screens[0]
        function tellRust() {
            const open = Panels.open === "about";
            RustUi.send({ cmd: "about", surface: "session", open: open, screen: open ? (Panels.screen?.name ?? "") : "", info: {
                    osName: SystemInfo.osName, hostname: SystemInfo.hostname, cpu: SystemInfo.cpu, gpu: SystemInfo.gpu,
                    memory: SystemInfo.memory, kernel: SystemInfo.kernel,
                    logo: IconLookup.path(SystemInfo.osLogo || "distributor-logo", "computer")
                } });
        }
        property Connections rustOpen: Connections {
            target: Panels
            enabled: slot.speaks && RustUi.drawing("session")
            function onOpenChanged() {
                Qt.callLater(slot.tellRust);
            }
        }
        property Connections rustClosed: Connections {
            target: RustUi
            enabled: slot.speaks
            function onMessage(e) {
                if (e.kind === "session" && e.close === "about" && Panels.open === "about")
                    Panels.close();
            }
        }

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "about"
            contentWidth: 300
            padding: Theme.material ? 24 : 20

            Column {
                width: dialog.contentWidth
                spacing: 6

                // (Material: the logo on a shape, a hero moment)
                Item {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Theme.material ? 104 : 72
                    height: width
                    Loader {
                        anchors.fill: parent
                        active: Theme.material
                        sourceComponent: MaterialShape {
                            shape: "cookie12"
                            color: Theme.m3.primaryContainer
                        }
                    }
                    IconImage {
                        anchors.centerIn: parent
                        implicitSize: Theme.material ? 64 : 72
                        source: IconLookup.path(SystemInfo.osLogo || "distributor-logo", "computer")
                    }
                }
                Item { width: 1; height: 6 }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: SystemInfo.osName
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.display; weight: Theme.material ? Font.Normal : Font.DemiBold }
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
