import QtQuick

// Keeps a ScreencopyView updating at most 30 times a second, instead of on
// every frame the window draws (live: true): give the view `live: false`
// (it then takes one picture when its source is set) and add
//   LiveCapture { target: preview; live: shown && mode === "live" }
// Nothing runs while `live` is false or the view has no source.
Timer {
    id: root

    property var target: null // the ScreencopyView
    property bool live: false

    interval: 33 // ~30 fps
    repeat: true
    running: live && !!target && !!target.captureSource
    onTriggered: target.captureFrame()
}
