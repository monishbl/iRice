import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services

// Adding a finger (Settings → Lock Screen → Fingerprint), in a popup over
// everything (its own window, like a password prompt). The fingerprint is
// drawn in 11 sections: the centre, four around it, then six around the
// edges. The section to touch next is lit and named, so the whole print gets
// recorded rather than the same middle eleven times; each touch the reader
// takes fills its section, a retry shakes the print and flashes the section
// red with the reader's reason, and the end turns every ridge green. Between
// the centre and the edges it says so, as a phone does. Nothing loops: it
// moves only on a touch.
//   open(finger)  shows it and starts reading
Scope {
    id: root

    property bool shown: false
    property string finger: ""
    signal addAnother

    function open(f) {
        finger = f;
        shown = true;
        Fingerprint.enroll(f);
    }
    function close() {
        if (Fingerprint.enrolling)
            Fingerprint.cancelEnroll();
        shown = false;
    }

    // ── Where the reading is ────────────────────────────────────────────
    readonly property int sections: 11
    readonly property bool done: Fingerprint.enrollStage === "done"
    readonly property bool failed: Fingerprint.enrollStage === "error"
    // sections taken: the reader's stages spread over the 11 (or one each,
    // if it doesn't say how many it wants)
    readonly property int taken: done ? sections
        : Fingerprint.enrollStages > 0 ? Math.min(sections - 1, Math.floor(sections * Fingerprint.enrollCount / Fingerprint.enrollStages))
        : Math.min(sections - 1, Fingerprint.enrollCount)
    readonly property int target: done || failed ? -1 : taken
    readonly property bool edges: target >= 5
    readonly property string fingerName: (Fingerprint.fingerNames[finger] ?? "finger").toLowerCase()
    // where to touch, section by section
    readonly property var steps: [
        "Rest the middle of your finger flat on the reader, then lift it.",
        "Again, a little higher up your finger.",
        "Again, with your finger a little to the right.",
        "Again, a little lower, toward the crease.",
        "Again, with your finger a little to the left.",
        "Touch with the tip of your finger.",
        "Roll onto the upper right edge.",
        "Now the lower right edge.",
        "Now the bottom of the print, near the crease.",
        "Now the lower left edge.",
        "And last, the upper left edge."
    ]
    // the reader's reason for a retry, shown for a moment
    property string retryText: ""
    Timer {
        id: retryShown
        interval: 2200
        onTriggered: root.retryText = ""
    }

    // ── The popup (its window exists while shown, plus time to fade out) ─
    property bool alive: false
    onShownChanged: shown ? (alive = true, unload.stop()) : unload.restart()
    Timer { id: unload; interval: 400; onTriggered: root.alive = false }

    LazyLoader {
        active: root.alive

        PanelWindow {
            id: win
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => root.shown ? 1 : 0)
            Behavior on progress { NumberAnimation { duration: root.shown ? Theme.motion.panel : Math.round(Theme.motion.panel * 0.66); easing.type: root.shown ? Easing.OutCubic : Easing.InCubic } }
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            color: "transparent"
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "irice:fingerprint"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: root.shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            Connections {
                target: Fingerprint
                function onScanned(result) {
                    if (result === "enroll-stage-passed" || result === "enroll-completed") {
                        root.retryText = "";
                        art.took();
                    } else if (result.startsWith("enroll-") && result !== "enroll-failed") {
                        root.retryText = Fingerprint.hints[result] ?? "Try that one again.";
                        retryShown.restart();
                        art.missed();
                    }
                }
            }
            Connections {
                target: root
                function onTargetChanged() {
                    if (root.target >= 0)
                        art.lightTarget();
                }
            }

            // Modal: blocks what's behind, doesn't dismiss
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, (Theme.dark ? 0.5 : 0.3) * win.progress)
                MouseArea { anchors.fill: parent; hoverEnabled: true }
            }
            Item {
                anchors.fill: parent
                focus: true
                Keys.onEscapePressed: root.close()
            }

            Squircle {
                id: card
                anchors.centerIn: parent
                width: 420
                height: body.implicitHeight + 48
                radius: 20
                color: Theme.dark ? "#232327" : "white"
                border.width: 1
                border.color: Theme.color.separator
                opacity: win.progress
                scale: 0.96 + 0.04 * win.progress

                Column {
                    id: body
                    x: 24
                    y: 24
                    width: parent.width - 48
                    spacing: 8

                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        text: root.done ? "Your " + root.fingerName + " is added"
                            : root.failed ? "That didn't work"
                            : root.edges ? "Now the edges"
                            : "Add your " + root.fingerName
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: 20; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        height: 40
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.done ? "It unlocks the lock screen now, and password prompts where you've turned that on."
                            : root.failed ? Fingerprint.enrollError
                            : root.retryText !== "" ? root.retryText
                            : Fingerprint.enrollStage === "starting" ? "Getting the reader ready…"
                            : root.steps[root.target] ?? ""
                        color: root.retryText !== "" && !root.done && !root.failed ? Theme.color.danger : Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: 14 }
                    }

                    FingerprintArt {
                        id: art
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 200
                        height: 248
                        taken: root.taken
                        target: root.target
                        done: root.done
                    }

                    // the 11 sections, as dots: taken, the next one, still to come
                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 6
                        topPadding: 4
                        Repeater {
                            model: root.sections
                            Rectangle {
                                required property int index
                                width: index === root.target ? 16 : 6
                                height: 6
                                radius: 3
                                color: root.done ? Theme.color.success : index < root.taken ? Theme.accent : index === root.target ? Qt.alpha(Theme.accent, 0.5) : Theme.color.toggleOff
                                Behavior on width { NumberAnimation { duration: Theme.motion.hover } }
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                            }
                        }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        text: root.done ? "All 11 sections recorded" : root.failed ? "" : "Section " + (root.target + 1) + " of 11 · " + (root.edges ? "the edges" : "the centre")
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: 12; features: { "tnum": 1 } }
                    }

                    Item { width: 1; height: 8 }
                    Row {
                        anchors.right: parent.right
                        spacing: 8
                        SettingsButton {
                            visible: root.done && Object.keys(Fingerprint.fingerNames).some(f => !Fingerprint.enrolled.includes(f) && f !== root.finger)
                            text: "Add Another Finger"
                            onClicked: root.addAnother()
                        }
                        SettingsButton {
                            visible: root.failed
                            text: "Try Again"
                            onClicked: Fingerprint.enroll(root.finger)
                        }
                        SettingsButton {
                            text: root.done ? "Done" : "Cancel"
                            primary: root.done
                            onClicked: root.close()
                        }
                    }
                }
            }
        }
    }
}
