import QtQuick

// A low-rate clock for slow, soft ambient motion (heat ripples, breathing):
// the scene redraws `fps` times a second instead of on every display frame.
// Bind to `time` (seconds while running; it pauses, not resets, when stopped).
Timer {
    id: root

    property int fps: 25
    property real time: 0
    property real origin: 0
    interval: Math.round(1000 / fps)
    repeat: true
    onRunningChanged: if (running) origin = Date.now() - time * 1000
    onTriggered: time = (Date.now() - origin) / 1000
}
