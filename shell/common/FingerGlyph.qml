import QtQuick

// A fingerprint that says what the reader is doing, wherever a password is
// asked (the lock screen's field, password prompts): "" | waiting |
// miss | match. Waiting pulses once each time the reader asks; a miss
// shakes it red; a match turns it green with a small pop. Nothing loops, so
// a prompt left open draws no frames.
Item {
    id: root

    property string phase: ""
    property int size: 22
    property color color: Theme.color.label

    implicitWidth: size
    implicitHeight: size

    Icon {
        id: glyph
        anchors.centerIn: parent
        name: root.phase === "match" ? "check-circle" : "fingerprint"
        fill: root.phase === "match"
        size: root.size
        color: root.phase === "miss" ? Theme.color.danger : root.phase === "match" ? Theme.color.success : root.color
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    onPhaseChanged: {
        if (CoolDown.motionScale <= 0)
            return;
        if (phase === "waiting")
            pulse.restart();
        else if (phase === "miss")
            shake.restart();
        else if (phase === "match")
            pop.restart();
    }

    SequentialAnimation {
        id: pulse
        NumberAnimation { target: glyph; property: "scale"; to: 1.14; duration: Math.round(180 * CoolDown.motionScale); easing.type: Easing.OutCubic }
        NumberAnimation { target: glyph; property: "scale"; to: 1; duration: Math.round(260 * CoolDown.motionScale); easing.type: Easing.InOutCubic }
    }
    SequentialAnimation {
        id: shake
        NumberAnimation { target: glyph; property: "anchors.horizontalCenterOffset"; to: -4; duration: 50 }
        NumberAnimation { target: glyph; property: "anchors.horizontalCenterOffset"; to: 4; duration: 70 }
        NumberAnimation { target: glyph; property: "anchors.horizontalCenterOffset"; to: -3; duration: 60 }
        NumberAnimation { target: glyph; property: "anchors.horizontalCenterOffset"; to: 2; duration: 50 }
        NumberAnimation { target: glyph; property: "anchors.horizontalCenterOffset"; to: 0; duration: 40 }
    }
    SequentialAnimation {
        id: pop
        NumberAnimation { target: glyph; property: "scale"; from: 0.6; to: 1.12; duration: Math.round(200 * CoolDown.motionScale); easing.type: Easing.OutBack }
        NumberAnimation { target: glyph; property: "scale"; to: 1; duration: Math.round(140 * CoolDown.motionScale) }
    }
}
