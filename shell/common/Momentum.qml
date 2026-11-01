import QtQuick

// Touchpad scrolling that glides on after the fingers lift, as on macOS
// (Qt on Wayland stops dead: libinput leaves the momentum to the app). Put
// it inside a Flickable or ListView: two-finger scrolls move it directly,
// and on lifting (the scroll's end phase; or, from a touchpad that reports
// no phases, a pause in its events) it's flicked at the speed it was
// going. A mouse wheel is left to the Flickable.
WheelHandler {
    id: root

    // (declared inside a Flickable, a handler sits on its contentItem)
    property Flickable flickable: parent?.parent instanceof Flickable ? parent.parent : null
    target: null
    acceptedDevices: PointerDevice.TouchPad
    // (a list that only scrolls sideways takes only sideways scrolls: the
    // page around it gets the rest; one that doesn't scroll takes none)
    enabled: canX || canY
    orientation: canY ? Qt.Vertical : Qt.Horizontal

    property real vx: 0 // px/s the content was moving (contentX/Y growing)
    property real vy: 0
    property real lastAt: 0
    property bool phased: false // this touchpad says when the fingers lift
    readonly property bool canX: !!flickable && flickable.interactive && flickable.contentWidth > flickable.width + 1 && flickable.flickableDirection !== Flickable.VerticalFlick
    readonly property bool canY: !!flickable && flickable.interactive && flickable.contentHeight > flickable.height + 1 && flickable.flickableDirection !== Flickable.HorizontalFlick

    onWheel: e => {
        const f = flickable;
        if (!f)
            return;
        const now = Date.now();
        if (e.phase !== Qt.NoScrollPhase)
            phased = true;
        if (e.phase === Qt.ScrollBegin) {
            f.cancelFlick();
            vx = vy = 0;
            lastAt = now;
        }
        const dx = canX ? -e.pixelDelta.x : 0, dy = canY ? -e.pixelDelta.y : 0;
        if (dx !== 0 || dy !== 0) {
            if (f.flicking)
                f.cancelFlick();
            const x0 = f.contentX, y0 = f.contentY;
            if (dx !== 0)
                f.contentX = Math.max(f.originX - f.leftMargin, Math.min(f.originX + f.contentWidth + f.rightMargin - f.width, f.contentX + dx));
            if (dy !== 0)
                f.contentY = Math.max(f.originY - f.topMargin, Math.min(f.originY + f.contentHeight + f.bottomMargin - f.height, f.contentY + dy));
            // (the speed of the last few events, smoothed)
            const dt = Math.max(4, Math.min(100, now - lastAt)) / 1000;
            vx = vx * 0.4 + (f.contentX - x0) / dt * 0.6;
            vy = vy * 0.4 + (f.contentY - y0) / dt * 0.6;
            lastAt = now;
            if (!phased)
                pause.restart();
        }
        if (e.phase === Qt.ScrollEnd)
            glide(now);
    }
    function glide(now) {
        // (lifted after holding still: no glide)
        if (flickable && now - lastAt < 80 && (Math.abs(vx) > 120 || Math.abs(vy) > 120))
            flickable.flick(-vx, -vy);
        vx = vy = 0;
    }
    // (a handler holds no children: the timer is a property)
    property Timer pause: Timer {
        interval: 50
        onTriggered: root.glide(root.lastAt + 1)
    }
}
