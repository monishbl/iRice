import QtQuick
import qs.common

// A one-line label that, when it doesn't fit and `scroll` is on, slides
// slowly to show the rest, pauses, and comes back. Otherwise it elides.
Item {
    id: root

    property string text
    property color color: "white"
    property font font
    property bool scroll: true
    property real maxWidth: 140

    readonly property bool overflow: probe.implicitWidth > maxWidth
    implicitWidth: Math.min(probe.implicitWidth, maxWidth)
    implicitHeight: probe.implicitHeight
    clip: overflow && scroll

    Text {
        id: probe
        visible: false
        text: root.text
        font: root.font
    }
    Text {
        id: label
        width: root.overflow && root.scroll ? implicitWidth : root.width
        text: root.text
        color: root.color
        font: root.font
        elide: root.overflow && root.scroll ? Text.ElideNone : Text.ElideRight
        // Slide to the end at ~30 px/s, wait, slide back (whole pixels, on a
        // 30 fps clock: no redraw unless the text actually moved a pixel)
        x: {
            if (!clock.running)
                return 0;
            const span = probe.implicitWidth - root.width;
            const out = Math.max(0.6, span * 0.033), back = Math.max(0.4, span * 0.012);
            const t = clock.time % (1.8 + out + 1.4 + back);
            const ease = f => 0.5 - 0.5 * Math.cos(Math.PI * f);
            const k = t < 1.8 ? 0 : t < 1.8 + out ? ease((t - 1.8) / out) : t < 3.2 + out ? 1 : 1 - ease((t - 3.2 - out) / back);
            return -Math.round(span * k);
        }
    }
    Ticker {
        id: clock
        running: root.overflow && root.scroll && root.visible
        fps: 30
    }
    onTextChanged: clock.time = 0
}
