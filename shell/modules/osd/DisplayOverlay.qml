import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.settings
import qs.services

// Settings → Displays on every screen: "Identify" shows each screen's number
// and name in its corner, and after a change that could leave a screen dark,
// every screen asks "Keep these display settings?" with the countdown, so
// it can be answered from whichever screen still shows a picture.
Scope {
    Variants {
        model: Displays.identifying || Displays.confirming ? Quickshell.screens.filter(s => !/^(HEADLESS|IRICE-)/.test(s.name)) : []

        Scope {
            id: perScreen
            required property var modelData

            // Number and name, top left
            LazyLoader {
                active: Displays.identifying

                GlassWindow {
                    readonly property int m: 24
                    name: "display-identify"
                    screen: perScreen.modelData
                    WlrLayershell.layer: WlrLayer.Overlay
                    anchors { top: true; left: true }
                    margins { top: 48 - m; left: 32 - m }
                    implicitWidth: Math.max(220, info.implicitWidth + 48) + 2 * m
                    implicitHeight: 132 + 2 * m
                    mask: Region {}

                    Glass {
                        anchors.fill: parent
                        radius: 26
                        bevel: 18
                        shadowMargin: parent.m
                    }
                    QQ.Row {
                        id: info
                        anchors.centerIn: parent
                        spacing: 18
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: Displays.numberOf(perScreen.modelData.name) || "?"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: 72; weight: Font.DemiBold }
                        }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2
                            Text {
                                text: Displays.monitors.find(m => m.name === perScreen.modelData.name)?.label ?? perScreen.modelData.name
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.DemiBold }
                            }
                            Text {
                                text: perScreen.modelData.name + " · " + perScreen.modelData.width + " × " + perScreen.modelData.height
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                        }
                    }
                }
            }

            // Keep or go back, centred
            LazyLoader {
                active: Displays.confirming

                GlassWindow {
                    readonly property int m: 24
                    name: "display-confirm"
                    screen: perScreen.modelData
                    WlrLayershell.layer: WlrLayer.Overlay
                    implicitWidth: 360 + 2 * m
                    implicitHeight: 150 + 2 * m

                    Glass {
                        anchors.fill: parent
                        radius: 22
                        bevel: 16
                        shadowMargin: parent.m
                    }
                    Column {
                        x: parent.m + 20
                        y: parent.m + 18
                        width: 320
                        spacing: 6
                        Text {
                            text: "Keep these display settings?"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Going back to the previous settings in " + Displays.confirmLeft + (Displays.confirmLeft === 1 ? " second." : " seconds.")
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body; features: ({ "tnum": 1 }) }
                        }
                    }
                    QQ.Row {
                        anchors { right: parent.right; bottom: parent.bottom; rightMargin: parent.m + 20; bottomMargin: parent.m + 18 }
                        spacing: 8
                        SettingsButton { text: "Go back"; onClicked: Displays.revert() }
                        SettingsButton { text: "Keep settings"; primary: true; onClicked: Displays.keep() }
                    }
                }
            }
        }
    }
}
