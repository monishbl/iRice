import QtQuick

// Segmented control: a track with a sliding selection pill.
Rectangle {
    id: root

    property var options: [] // labels
    property int current: 0
    property string accessibleName
    signal selected(int index)

    implicitHeight: 30
    activeFocusOnTab: true
    Keys.onPressed: event => {
        const k = event.key;
        const i = k === Qt.Key_Left || k === Qt.Key_Up ? current - 1 : k === Qt.Key_Right || k === Qt.Key_Down ? current + 1
            : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? options.length - 1 : -2;
        if (i !== -2) {
            const c = Math.max(0, Math.min(options.length - 1, i));
            if (c !== current)
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
    color: Theme.color.toggleOff

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
    readonly property var widths: {
        const n = Math.max(1, natural.length), avail = width - 4;
        const total = natural.reduce((a, b) => a + b, 0);
        const extra = (avail - total) / n;
        return natural.map(w => extra >= 0 ? w + extra : w * avail / Math.max(1, total));
    }
    function offset(i) {
        let x = 0;
        for (let k = 0; k < i; k++)
            x += widths[k] ?? 0;
        return x;
    }
    FontMetrics {
        id: metrics
        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
    }

    Rectangle {
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
        Repeater {
            model: root.options
            Item {
                required property string modelData
                required property int index
                width: root.widths[index] ?? 0
                height: root.height
                Text {
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, parent.width - 12)
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: parent.modelData
                    color: parent.index === root.current ? Theme.color.label : Theme.color.secondary
                    font: metrics.font
                }
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData
                Accessible.checked: index === root.current
                Accessible.onPressAction: root.selected(index)
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.selected(parent.index)
                }
            }
        }
    }
    FocusRing {
        radius: height / 2
    }
}
