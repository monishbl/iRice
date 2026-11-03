import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.settings
import qs.services

// Sharing with an iPhone (or any phone's browser) through a QR code
// (services/LinkShare): the code in the middle of the screen you're using,
// what to do with it, how it's going, and Done. Escape closes it; closing
// stops the sharing. One card for both looks (a dialog's surface in
// Material).
Scope {
    id: root

    readonly property bool sending: LinkShare.mode === "send"
    readonly property string title: sending ? (LinkShare.names.length === 1 ? "Share “" + LinkShare.names[0] + "”" : "Share " + LinkShare.names.length + " files") : "Receive from a phone"
    readonly property string help: sending ? "Open the Camera on the iPhone and point it at the code, then tap Save for each file."
        : "Open the Camera on the iPhone and point it at the code, then choose what to send."
    readonly property string status: {
        if (LinkShare.busy)
            return (sending ? "Sending… " : "Receiving… ") + Math.round(100 * LinkShare.done / LinkShare.total) + "%";
        if (LinkShare.count > 0)
            return sending ? "Saved " + LinkShare.count + (LinkShare.count === 1 ? " time" : " times") : "Received " + LinkShare.count + (LinkShare.count === 1 ? " file" : " files");
        return "The phone needs the same Wi-Fi as this computer.";
    }

    // (qrencode just installed: make the code again)
    Connections {
        target: Needs
        function onInstallingChanged() {
            if (!Needs.installing && LinkShare.stage === "error" && LinkShare.error.includes("qrencode"))
                LinkShare.mode === "send" ? LinkShare.close() : LinkShare.receive();
        }
    }

    LazyLoader {
        active: LinkShare.mode !== ""

        GlassWindow {
            id: win
            name: "share-link"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            implicitWidth: card.width + 2 * glass.shadowMargin
            implicitHeight: card.height + 2 * glass.shadowMargin
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand

            property real shown: 0
            Component.onCompleted: shown = 1
            Behavior on shown { NumberAnimation { duration: Math.round(300 * CoolDown.motionScale); easing.type: Easing.OutCubic } }

            Glass {
                id: glass
                anchors.fill: parent
                visible: Theme.material !== true
                radius: Theme.radius.panel
                opacity: win.shown
                scale: 0.96 + 0.04 * win.shown
            }
            Rectangle {
                visible: Theme.material === true
                anchors.fill: card
                radius: 28
                color: Theme.m3?.surfaceContainerHigh ?? "transparent"
                opacity: win.shown
                scale: 0.96 + 0.04 * win.shown
            }

            Item {
                id: card
                x: glass.shadowMargin
                y: glass.shadowMargin
                width: 320
                height: body.implicitHeight + 48
                opacity: win.shown
                scale: 0.96 + 0.04 * win.shown
                focus: true
                Keys.onEscapePressed: LinkShare.close()
                Keys.onReturnPressed: LinkShare.close()

                Column {
                    id: body
                    x: 24
                    y: 24
                    width: parent.width - 48
                    spacing: 8

                    Text {
                        width: parent.width
                        text: root.title
                        elide: Text.ElideMiddle
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: 18; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        visible: LinkShare.stage !== "error"
                        text: root.help
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }

                    Item { width: 1; height: 8 }

                    // The code, on white (cameras read dark on light), or what's happening
                    Item {
                        width: parent.width
                        height: LinkShare.stage === "error" ? errorCol.implicitHeight : 208
                        Rectangle {
                            anchors.centerIn: parent
                            width: 200
                            height: 200
                            radius: 14
                            color: "white"
                            visible: LinkShare.stage === "ready"
                            Image {
                                anchors { fill: parent; margins: 10 }
                                source: LinkShare.qr !== "" ? "file://" + LinkShare.qr : ""
                                sourceSize: Qt.size(360, 360)
                                fillMode: Image.PreserveAspectFit
                                smooth: false
                                cache: false
                                Accessible.role: Accessible.Graphic
                                Accessible.name: "QR code with the link to " + (root.sending ? "the shared files" : "send files here")
                            }
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: LinkShare.stage === "loading"
                            name: "circle-notch"
                            size: 24
                            color: Theme.color.secondary
                            RotationAnimation on rotation { running: LinkShare.stage === "loading"; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
                        }
                        Column {
                            id: errorCol
                            width: parent.width
                            visible: LinkShare.stage === "error"
                            spacing: 12
                            Text {
                                width: parent.width
                                text: LinkShare.error
                                wrapMode: Text.WordWrap
                                horizontalAlignment: Text.AlignHCenter
                                color: Theme.color.danger
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            SettingsButton {
                                anchors.horizontalCenter: parent.horizontalCenter
                                visible: LinkShare.error.includes("qrencode")
                                text: Needs.installing ? "Installing…" : "Install qrencode"
                                busy: Needs.installing
                                enabled: !Updates.running
                                onClicked: Needs.install(["qrencode"])
                            }
                        }
                    }

                    Text {
                        width: parent.width
                        visible: LinkShare.stage === "ready"
                        text: root.status
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                        color: LinkShare.busy || LinkShare.count > 0 ? Theme.color.label : Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                    }
                    // (plain HTTP on the local network: said plainly)
                    Text {
                        width: parent.width
                        visible: LinkShare.stage === "ready"
                        text: "Works for 10 minutes, on this network only. Others on it could see what's shared, so use it on networks you trust."
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: 12 }
                    }

                    Item { width: 1; height: 8 }

                    Squircle {
                        id: btn
                        width: parent.width
                        height: 40
                        radius: 20
                        color: btnMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent
                        scale: btnMouse.pressed ? 0.97 : 1
                        Behavior on scale { NumberAnimation { duration: 100 } }
                        activeFocusOnTab: true
                        Keys.onPressed: event => {
                            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                                LinkShare.close();
                                event.accepted = true;
                            }
                        }
                        Accessible.role: Accessible.Button
                        Accessible.name: LinkShare.busy ? "Stop sharing" : "Done"
                        Accessible.onPressAction: LinkShare.close()
                        FocusRing { radius: 24 }
                        Text {
                            anchors.centerIn: parent
                            text: LinkShare.busy ? "Stop Sharing" : "Done"
                            color: Theme.color.onAccent
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                        }
                        MouseArea {
                            id: btnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: LinkShare.close()
                        }
                    }
                }
            }
        }
    }
}
