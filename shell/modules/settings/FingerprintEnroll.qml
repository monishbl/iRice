import QtQuick
import qs.common
import qs.services

// Adding a finger (Settings → Lock Screen → Fingerprint), as on a phone: a
// large fingerprint whose ridges fill from the bottom as each touch is
// taken, a ripple for every touch that counts, a shake when the reader asks
// for another try, and a check when it's done. Under it, the finger being
// added and how far along it is. Nothing loops: it moves only on a touch.
Item {
    id: root

    readonly property bool done: Fingerprint.enrollStage === "done"
    readonly property real progress: done ? 1 : Fingerprint.enrollStages > 0 ? Math.min(1, Fingerprint.enrollCount / Fingerprint.enrollStages) : 0
    readonly property string finger: Fingerprint.fingerNames[Fingerprint.enrollFinger] ?? ""
    property bool retrying: false

    width: parent?.width ?? 400
    implicitHeight: 220

    Connections {
        target: Fingerprint
        function onScanned(result) {
            if (CoolDown.motionScale <= 0)
                return;
            if (result === "enroll-stage-passed" || result === "enroll-completed") {
                ripple.restart();
                tap.restart();
            } else if (result.startsWith("enroll-") && result !== "enroll-failed") {
                root.retrying = true;
                retryShown.restart();
                shake.restart();
            }
        }
    }
    Timer {
        id: retryShown
        interval: 900
        onTriggered: root.retrying = false
    }

    Item {
        id: printBox
        anchors.horizontalCenter: parent.horizontalCenter
        y: 24
        width: 112
        height: 112

        // the ring each counted touch sends out
        Rectangle {
            id: ring
            anchors.centerIn: parent
            width: 112
            height: 112
            radius: width / 2
            color: "transparent"
            border.width: 2
            border.color: Theme.accent
            opacity: 0
        }
        ParallelAnimation {
            id: ripple
            NumberAnimation { target: ring; property: "scale"; from: 0.85; to: 1.35; duration: Math.round(520 * CoolDown.motionScale); easing.type: Easing.OutCubic }
            NumberAnimation { target: ring; property: "opacity"; from: 0.55; to: 0; duration: Math.round(520 * CoolDown.motionScale); easing.type: Easing.OutCubic }
        }

        Item {
            id: glyphs
            anchors.centerIn: parent
            width: 96
            height: 96

            // the ridges still to fill
            Icon {
                anchors.centerIn: parent
                visible: !root.done
                name: "fingerprint"
                size: 96
                color: root.retrying ? Theme.color.danger : Theme.color.toggleOff
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            }
            // the ridges taken so far, rising from the bottom
            Item {
                visible: !root.done
                anchors.bottom: parent.bottom
                width: parent.width
                height: parent.height * root.progress
                clip: true
                Behavior on height { NumberAnimation { duration: Math.round(320 * CoolDown.motionScale); easing.type: Easing.OutCubic } }
                Icon {
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 96
                    height: 96
                    name: "fingerprint"
                    size: 96
                    color: Theme.accent
                }
            }
            Icon {
                id: check
                anchors.centerIn: parent
                visible: root.done
                name: "check-circle"
                fill: true
                size: 96
                color: Theme.color.success
                onVisibleChanged: if (visible && CoolDown.motionScale > 0) pop.restart()
            }
        }
        SequentialAnimation {
            id: tap
            NumberAnimation { target: glyphs; property: "scale"; to: 0.92; duration: Math.round(90 * CoolDown.motionScale); easing.type: Easing.OutCubic }
            NumberAnimation { target: glyphs; property: "scale"; to: 1; duration: Math.round(240 * CoolDown.motionScale); easing.type: Easing.OutBack }
        }
        SequentialAnimation {
            id: shake
            NumberAnimation { target: glyphs; property: "anchors.horizontalCenterOffset"; to: -8; duration: 50 }
            NumberAnimation { target: glyphs; property: "anchors.horizontalCenterOffset"; to: 7; duration: 70 }
            NumberAnimation { target: glyphs; property: "anchors.horizontalCenterOffset"; to: -5; duration: 60 }
            NumberAnimation { target: glyphs; property: "anchors.horizontalCenterOffset"; to: 3; duration: 50 }
            NumberAnimation { target: glyphs; property: "anchors.horizontalCenterOffset"; to: 0; duration: 40 }
        }
        SequentialAnimation {
            id: pop
            NumberAnimation { target: check; property: "scale"; from: 0.5; to: 1.08; duration: Math.round(240 * CoolDown.motionScale); easing.type: Easing.OutBack }
            NumberAnimation { target: check; property: "scale"; to: 1; duration: Math.round(140 * CoolDown.motionScale) }
        }
    }

    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: printBox.bottom
        anchors.topMargin: 16
        width: parent.width - 32
        spacing: 4
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.done ? root.finger + " added" : Fingerprint.enrollHint || "Touch the fingerprint reader"
            color: root.retrying ? Theme.color.danger : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.done ? "It works on the lock screen now."
                : root.finger + (Fingerprint.enrollStages > 0 ? " · " + Fingerprint.enrollCount + " of " + Fingerprint.enrollStages : "")
                  + " · lift and touch again, moving it a little each time"
            color: Theme.color.secondary
            wrapMode: Text.Wrap
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
        }
    }
}
