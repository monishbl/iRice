import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Polkit
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Polkit authentication agent: a modal glass dialog asking for the password
// when an app needs admin rights. Not registered in the dev sandbox (it would
// act on the real system's polkit); use `qs ipc call polkit preview` there.
Scope {
    id: root

    readonly property var flow: agent.flow ?? null
    property bool preview: false
    readonly property bool shown: (agent.isActive && flow !== null) || preview
    property string error: ""
    property bool waiting: false

    readonly property string message: preview ? "Authentication is required to install software." : (flow?.message ?? "").replace(/\.$/, "")
    readonly property string prompt: preview ? "Password" : ((flow?.inputPrompt ?? "").trim().replace(/:$/, "") || "Password")
    readonly property bool secret: preview || !(flow?.responseVisible ?? false)
    // A finger first (fprintd in the PAM stack: Settings → Lock Screen →
    // Fingerprint): PAM asks for no text while it waits for one, then for the
    // password once that times out
    property bool previewFinger: false
    readonly property bool fingerWait: previewFinger || (flow !== null && !(flow?.isResponseRequired ?? true)
        && /finger|reader|swipe/i.test(flow?.supplementaryMessage ?? ""))

    function submit(text) {
        if (preview) {
            error = "Incorrect password. Try again.";
            return;
        }
        if (!flow || text === "")
            return;
        waiting = true;
        error = "";
        flow.submit(text);
    }

    function cancel() {
        preview = false;
        previewFinger = false;
        error = "";
        waiting = false;
        flow?.cancelAuthenticationRequest();
    }

    // Stand-in while the real agent isn't loaded
    QtObject {
        id: agentStub
        property bool isActive: false
        property var flow: null
    }
    property var agent: agentLoader.item ?? agentStub
    // For System health: only the agent knows whether polkit accepted it
    Binding {
        target: Health
        property: "polkit"
        value: System.sandboxed ? "off" : (root.agent.isRegistered ?? false) ? "yes" : "no"
    }
    LazyLoader {
        id: agentLoader
        active: !System.sandboxed
        component: PolkitAgent {}
    }

    Connections {
        target: root.flow
        function onAuthenticationFailed() {
            root.waiting = false;
            root.error = "Incorrect password. Try again.";
        }
        function onIsCompletedChanged() {
            root.waiting = false;
            root.error = "";
        }
    }

    IpcHandler {
        target: "polkit"
        function preview(): void {
            root.preview = true;
        }
        // (the fingerprint step, as it looks while PAM waits for a finger)
        function previewFinger(): void {
            root.preview = true;
            root.previewFinger = true;
        }
    }

    // Exists only while visible (plus time for the exit animation)
    property bool windowAlive: false
    readonly property bool windowWanted: root.shown
    onWindowWantedChanged: windowWanted ? (windowAlive = true, windowUnload.stop()) : windowUnload.restart()
    Timer { id: windowUnload; interval: 600; onTriggered: root.windowAlive = false }

    LazyLoader {
        active: root.windowAlive

        GlassWindow {
            id: win
            readonly property int m: 24
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => root.shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.panel; easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            name: "polkit"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            visible: progress > 0
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: root.shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            onVisibleChanged: if (visible) Qt.callLater(() => password.forceActiveFocus())

            // Modal scrim: blocks the desktop, does not dismiss
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.3 * win.progress)
                MouseArea { anchors.fill: parent }
            }

            Item {
                id: body
                width: 340 + 2 * win.m
                height: content.implicitHeight + 40 + 2 * win.m
                x: (parent.width - width) / 2 + shake
                y: (parent.height - height) / 2 - 40
                opacity: win.progress
                scale: 0.96 + 0.04 * win.progress
                onXChanged: glass.updatePosition()

                property real shake: 0
                SequentialAnimation {
                    id: shakeAnim
                    NumberAnimation { target: body; property: "shake"; to: 10; duration: 50 }
                    NumberAnimation { target: body; property: "shake"; to: -10; duration: 80 }
                    NumberAnimation { target: body; property: "shake"; to: 6; duration: 70 }
                    NumberAnimation { target: body; property: "shake"; to: 0; duration: 60 }
                }
                Connections {
                    target: root
                    function onErrorChanged() { if (root.error !== "") shakeAnim.restart(); }
                }

                Glass {
                    id: glass
                    anchors.fill: parent
                    radius: 28
                    bevel: 22
                    shadowMargin: win.m
                }

                Column {
                    id: content
                    x: win.m + 20
                    y: win.m + 20
                    width: 340 - 40
                    spacing: 10

                    Squircle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 56
                        height: 56
                        radius: 16
                        color: Theme.color.toggleOff
                        Icon { anchors.centerIn: parent; name: root.fingerWait ? "fingerprint" : "lock-key"; fill: !root.fingerWait; size: 28 }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.message
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.fingerWait ? "Touch the fingerprint reader to allow this. The password field comes back in a few seconds."
                            : "Enter the password for " + (Quickshell.env("USER") ?? "your account") + " to allow this."
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }

                    Squircle {
                        width: parent.width
                        height: 36
                        visible: !root.fingerWait
                        onVisibleChanged: if (visible) password.forceActiveFocus() // (back after the finger timed out)
                        radius: Theme.radius.control
                        color: Theme.color.fill
                        border.width: 1
                        border.color: root.error !== "" ? Theme.color.danger : password.activeFocus ? Theme.accent : Theme.color.separator
                        TextInput {
                            id: password
                            anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
                            verticalAlignment: TextInput.AlignVCenter
                            echoMode: root.secret ? TextInput.Password : TextInput.Normal
                            passwordCharacter: "●"
                            enabled: !root.waiting
                            color: Theme.color.label
                            clip: true
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                            onAccepted: root.submit(text)
                            Keys.onEscapePressed: root.cancel()
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: password.text === ""
                                text: root.prompt
                                color: Theme.color.tertiary
                                font: password.font
                            }
                        }
                    }
                    Text {
                        width: parent.width
                        visible: root.error !== ""
                        horizontalAlignment: Text.AlignHCenter
                        text: root.error
                        color: Theme.color.danger
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }

                    Row {
                        width: parent.width
                        spacing: 10
                        topPadding: 6
                        Repeater {
                            model: root.fingerWait ? [{ text: "Cancel", primary: false }]
                                : [{ text: "Cancel", primary: false }, { text: root.waiting ? "Checking…" : "Allow", primary: true }]
                            Rectangle {
                                required property var modelData
                                width: root.fingerWait ? parent.width : (parent.width - 10) / 2
                                height: 34
                                radius: 17
                                color: modelData.primary ? Theme.accent : btnMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                opacity: modelData.primary && password.text === "" ? 0.5 : 1
                                Text {
                                    anchors.centerIn: parent
                                    text: parent.modelData.text
                                    color: parent.modelData.primary ? Theme.color.onAccent : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: btnMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: parent.modelData.primary ? root.submit(password.text) : root.cancel()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
