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
    // (not while a typed password is checked: the last message can still
    // be about the finger)
    readonly property bool fingerWait: previewFinger || (flow !== null && !waiting && !(flow?.isResponseRequired ?? true)
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

    // The finger (fingerWait): a pulse each time the reader asks, a shake
    // and "Not recognised" for a real miss, a check when it matched. A "no
    // match" sooner than a touch could be isn't one (some readers answer at
    // once for a finger still resting on them)
    property real fingerAskedAt: 0
    property bool fingerMissed: false
    readonly property string fingerPhase: previewFinger ? "waiting" : !fingerWait ? "" : fingerMissed ? "miss" : "waiting"
    Timer {
        id: fingerMissShown
        interval: 1600
        onTriggered: root.fingerMissed = false
    }
    Connections {
        target: root.flow
        function onSupplementaryMessageChanged() {
            const m = root.flow?.supplementaryMessage ?? "";
            if (/fail|no match|not match/i.test(m)) {
                if (Date.now() - root.fingerAskedAt >= 700) {
                    root.fingerMissed = true;
                    fingerMissShown.restart();
                }
            } else if (/place|swipe|finger/i.test(m)) {
                root.fingerAskedAt = Date.now();
            }
        }
        function onAuthenticationFailed() {
            // (only a typed password is "incorrect": a finger that didn't
            // match isn't, and PAM asks for the password next anyway)
            const typed = root.waiting;
            root.waiting = false;
            root.error = typed ? "Incorrect password. Try again." : "";
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
                color: Theme.material ? Qt.alpha(Theme.m3.scrim, 0.32 * win.progress) : Qt.rgba(0, 0, 0, 0.3 * win.progress)
                MouseArea { anchors.fill: parent }
            }

            Item {
                id: body
                // (Escape while the field is away, waiting for a finger)
                focus: true
                Keys.onEscapePressed: root.cancel()
                // (Material: a basic dialog, 24 padding)
                readonly property int pad: Theme.material ? 24 : 20
                width: 340 + 2 * win.m
                height: content.implicitHeight + 2 * pad + 2 * win.m
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
                // Material: a dialog is surfaceContainerHigh, a step above
                // the surfaces it covers
                Rectangle {
                    anchors.fill: glass.shape
                    visible: Theme.material
                    radius: 28
                    color: Theme.material ? Theme.m3.surfaceContainerHigh : "transparent"
                }

                Column {
                    id: content
                    x: win.m + body.pad
                    y: win.m + body.pad
                    width: 340 - 2 * body.pad
                    spacing: Theme.material ? 12 : 10

                    Squircle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Theme.material ? 64 : 56
                        height: width
                        radius: 16
                        color: Theme.material ? "transparent" : Theme.color.toggleOff
                        // Material: the hero icon on a shape, a cookie for the
                        // password, a softer one while waiting for a finger,
                        // and a burst in the error colours for a miss
                        Loader {
                            anchors.fill: parent
                            active: Theme.material
                            sourceComponent: MaterialShape {
                                shape: root.fingerWait ? (root.fingerMissed ? "softBurst" : "clover8") : root.error !== "" ? "softBurst" : "cookie9"
                                color: (root.fingerWait && root.fingerMissed) || root.error !== "" ? Theme.m3.errorContainer : Theme.m3.secondaryContainer
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                            }
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: !root.fingerWait
                            name: "lock-key"
                            fill: true
                            size: 28
                            color: !Theme.material ? Theme.color.label : root.error !== "" ? Theme.m3.onErrorContainer : Theme.m3.onSecondaryContainer
                        }
                        FingerGlyph {
                            anchors.centerIn: parent
                            visible: root.fingerWait
                            phase: root.fingerPhase
                            size: 30
                            color: !Theme.material ? Theme.accent : root.fingerMissed ? Theme.m3.onErrorContainer : Theme.m3.onSecondaryContainer
                        }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.message
                        color: Theme.color.label
                        // (Material: the dialog's headline, headline small)
                        font { family: Theme.font.family; pixelSize: Theme.material ? 24 : Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.fingerWait && root.fingerMissed ? "Not recognised. Try again, or wait for the password field."
                            : root.fingerWait ? "Touch the fingerprint reader to allow this. The password field comes back in a few seconds."
                            : "Enter the password for " + (Quickshell.env("USER") ?? "your account") + " to allow this."
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small }
                    }

                    Squircle {
                        width: parent.width
                        height: Theme.material ? 52 : 36
                        visible: !root.fingerWait
                        onVisibleChanged: visible ? password.forceActiveFocus() : body.forceActiveFocus() // (back after the finger timed out)
                        radius: Theme.radius.control
                        color: Theme.material ? "transparent" : Theme.color.fill
                        border.width: Theme.material ? 0 : 1
                        border.color: root.error !== "" ? Theme.color.danger : password.activeFocus ? Theme.accent : Theme.color.separator
                        // Material: a filled text field (top corners 4, an
                        // indicator line that thickens in primary, error red)
                        Loader {
                            anchors.fill: parent
                            active: Theme.material
                            sourceComponent: Rectangle {
                                topLeftRadius: 4
                                topRightRadius: 4
                                color: fieldHover.hovered ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHighest
                                HoverHandler {
                                    id: fieldHover
                                    cursorShape: Qt.IBeamCursor
                                }
                                Rectangle {
                                    anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                                    height: password.activeFocus || root.error !== "" ? 2 : 1
                                    color: root.error !== "" ? Theme.m3.error : password.activeFocus ? Theme.m3.primary : Theme.m3.onSurfaceVariant
                                }
                            }
                        }
                        TextInput {
                            id: password
                            anchors { fill: parent; leftMargin: Theme.material ? 16 : 12; rightMargin: Theme.material ? 16 : 12 }
                            verticalAlignment: TextInput.AlignVCenter
                            echoMode: root.secret ? TextInput.Password : TextInput.Normal
                            passwordCharacter: "●"
                            enabled: !root.waiting
                            color: Theme.color.label
                            clip: true
                            font { family: Theme.font.family; pixelSize: Theme.material ? 16 : Theme.font.body }
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
                        spacing: Theme.material ? 8 : 10
                        topPadding: Theme.material ? 12 : 6
                        Repeater {
                            model: root.fingerWait ? [{ text: "Cancel", primary: false }]
                                : [{ text: "Cancel", primary: false }, { text: root.waiting ? "Checking…" : "Allow", primary: true }]
                            Rectangle {
                                id: btn
                                required property var modelData
                                // Material: a filled primary pill for Allow, a
                                // tonal one for Cancel; state layers, and the
                                // corners tighten while pressed. Allow is
                                // greyed (12% / 38% of onSurface) until there's
                                // a password, rather than faded.
                                readonly property bool off: modelData.primary && password.text === ""
                                readonly property color ink: !Theme.material ? (modelData.primary ? Theme.color.onAccent : Theme.color.label)
                                    : off ? Qt.alpha(Theme.m3.onSurface, 0.38) : modelData.primary ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
                                readonly property color base: !Theme.material ? "transparent" : off ? Qt.alpha(Theme.m3.onSurface, 0.12) : modelData.primary ? Theme.m3.primary : Theme.m3.secondaryContainer
                                width: root.fingerWait ? parent.width : (parent.width - parent.spacing) / 2
                                height: Theme.material ? 40 : 34
                                radius: Theme.material ? (btnMouse.pressed && !off ? 8 : height / 2) : 17
                                Behavior on radius {
                                    enabled: Theme.material
                                    SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.25 }
                                }
                                color: Theme.material ? (off ? base : Qt.tint(base, Qt.alpha(ink, btnMouse.pressed ? 0.1 : btnMouse.containsMouse ? 0.08 : 0)))
                                    : modelData.primary ? Theme.accent : btnMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                opacity: !Theme.material && off ? 0.5 : 1
                                Text {
                                    anchors.centerIn: parent
                                    text: parent.modelData.text
                                    color: btn.ink
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: Theme.material ? Font.Medium : Font.DemiBold }
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
