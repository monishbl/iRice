import QtQuick

// Segmented control: a track with a sliding selection pill. Material: Material
// 3 Expressive's connected button group (segments 2 px apart, round outer
// ends, 8 px inner corners; the chosen one fills and turns fully round).
// Disabled, it fades (Material: its segments in onSurface at 12%, the
// labels at 38%); `off` lists segments that can't be chosen on their own.
Rectangle {
    id: root

    property var options: [] // labels
    property var off: [] // indexes that can't be chosen (an app that isn't installed)
    property int current: 0
    property string accessibleName
    signal selected(int index)

    implicitHeight: Theme.material ? 36 : 30
    opacity: enabled || Theme.material ? 1 : 0.45
    activeFocusOnTab: true
    Keys.onPressed: event => {
        const k = event.key;
        const step = k === Qt.Key_Left || k === Qt.Key_Up || k === Qt.Key_End ? -1 : 1;
        let i = k === Qt.Key_Left || k === Qt.Key_Up ? current - 1 : k === Qt.Key_Right || k === Qt.Key_Down ? current + 1
            : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? options.length - 1 : -2;
        if (i !== -2) {
            while (off.includes(i) && i >= 0 && i < options.length)
                i += step; // (past the ones that can't be chosen)
            const c = Math.max(0, Math.min(options.length - 1, i));
            if (c !== current && !off.includes(c))
                root.selected(c);
            event.accepted = true;
        }
    }
    // Read as a group of choices, the chosen one named
    Accessible.role: Accessible.PageTabList
    Accessible.name: accessibleName
    Accessible.description: options[current] ?? ""
    implicitWidth: natural.reduce((a, b) => a + b, 0) + 4
    radius: height / 2
    color: Theme.material ? "transparent" : Theme.color.toggleOff

    // Each segment fits its label (plus padding); room to spare is shared
    // equally, so short labels still split the track evenly. Too little room
    // shrinks every segment alike and the labels elide.
    readonly property real pad: 24
    readonly property var natural: {
        void metrics.font.family, metrics.font.pixelSize; // re-measure when the font changes
        const w = [];
        for (let i = 0; i < options.length; i++)
            w.push(Math.ceil(metrics.advanceWidth(options[i])) + pad);
        return w;
    }
    readonly property real gap: Theme.material ? 2 : 0
    readonly property var widths: {
        const n = Math.max(1, natural.length), avail = width - 4 - gap * (n - 1);
        const total = natural.reduce((a, b) => a + b, 0);
        const extra = (avail - total) / n;
        const w = natural.map(w => extra >= 0 ? w + extra : w * avail / Math.max(1, total));
        // Material 3 Expressive: the pressed button widens and its
        // neighbours give way, on the press spring (bouncing back)
        const p = pressing;
        if (p >= 0 && p < w.length && w.length > 1 && squeeze !== 0) {
            const next = [p - 1, p + 1].filter(i => i >= 0 && i < w.length);
            const d = Math.min(14, w[p] * 0.18) * squeeze;
            w[p] += d;
            for (const i of next)
                w[i] -= d / next.length;
        }
        return w;
    }
    property int pressing: -1
    property real squeeze: pressing >= 0 && pressedNow ? 1 : 0
    property bool pressedNow: false
    Behavior on squeeze {
        enabled: Theme.material && Theme.motion.pressBounce
        SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.002 }
    }
    function offset(i) {
        let x = 0;
        for (let k = 0; k < i; k++)
            x += (widths[k] ?? 0) + gap;
        return x;
    }
    FontMetrics {
        id: metrics
        font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
    }

    // Glass: the sliding selection pill (Material's segments draw their own)
    Rectangle {
        visible: !Theme.material
        x: 2 + root.offset(root.current)
        y: 2
        width: root.widths[root.current] ?? 0
        height: root.height - 4
        radius: height / 2
        color: Theme.dark ? Qt.rgba(1, 1, 1, 0.2) : "white"
        Behavior on x { XAnimator { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
        Behavior on width { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
    }

    Row {
        x: 2
        spacing: root.gap
        Repeater {
            model: root.options
            Item {
                id: segment
                required property string modelData
                required property int index
                readonly property bool chosen: index === root.current
                readonly property bool usable: root.enabled && !root.off.includes(index)
                width: root.widths[index] ?? 0
                height: root.height
                // Material: the segment's own button
                Loader {
                    anchors.fill: parent
                    active: Theme.material
                    sourceComponent: Rectangle {
                        readonly property real full: height / 2
                        readonly property real inner: segment.chosen ? full : segMouse.pressed ? 4 : 8
                        readonly property bool first: segment.index === 0
                        readonly property bool last: segment.index === root.options.length - 1
                        // (springs: Material's fast spatial, ζ 0.6, k 800)
                        property real leftR: first ? full : inner
                        property real rightR: last ? full : inner
                        Behavior on leftR { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.1 } }
                        Behavior on rightR { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.1 } }
                        topLeftRadius: Math.max(0, Math.min(full, leftR))
                        bottomLeftRadius: topLeftRadius
                        topRightRadius: Math.max(0, Math.min(full, rightR))
                        bottomRightRadius: topRightRadius
                        color: !root.enabled ? Qt.alpha(Theme.m3.onSurface, segment.chosen ? 0.12 : 0.06) : segment.chosen ? Theme.m3.primary : Theme.m3.surfaceContainerHighest
                        Behavior on color { ColorAnimation { duration: 150 } }
                        Rectangle {
                            anchors.fill: parent
                            topLeftRadius: parent.topLeftRadius
                            bottomLeftRadius: parent.bottomLeftRadius
                            topRightRadius: parent.topRightRadius
                            bottomRightRadius: parent.bottomRightRadius
                            color: segment.chosen ? Theme.m3.onPrimary : Theme.m3.onSurfaceVariant
                            opacity: segMouse.pressed ? 0.1 : segMouse.containsMouse ? 0.08 : 0
                        }
                    }
                }
                Text {
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, parent.width - 12)
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: parent.modelData
                    color: Theme.material ? (!parent.usable ? Qt.alpha(Theme.m3.onSurface, 0.38) : parent.chosen ? Theme.m3.onPrimary : Theme.m3.onSurfaceVariant)
                        : !parent.usable ? Theme.color.tertiary : parent.index === root.current ? Theme.color.label : Theme.color.secondary
                    font: metrics.font
                }
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData
                Accessible.checked: index === root.current
                Accessible.onPressAction: root.selected(index)
                MouseArea {
                    id: segMouse
                    anchors.fill: parent
                    enabled: parent.usable
                    hoverEnabled: Theme.material
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.selected(parent.index)
                    onPressedChanged: {
                        if (!Theme.material)
                            return;
                        if (pressed)
                            root.pressing = parent.index;
                        root.pressedNow = pressed;
                    }
                }
            }
        }
    }
    FocusRing {
        radius: height / 2
    }
}
