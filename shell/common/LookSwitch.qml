pragma Singleton
import QtQuick
import Quickshell
import "MaterialColor.js" as MaterialColor

// Switching between the looks (Glass and Material). The shell shows `shown`:
// the chosen look, except while switching, when it keeps the old one until
// the switching screen (modules/lookswitch) covers the desktop, then restyles
// behind it and reveals the new look. Changing Config.options.appearance.look
// from anywhere runs it. At startup, and with Reduce motion, the look just
// changes.
// Restyling re-evaluates the whole shell and builds the new look's panels: a
// second or so of the main thread. So the screen does its own motion on the
// render thread (Animators keep running meanwhile), the restyle waits for the
// cover to be up, and the reveal waits until the main thread is free again.
//   phase: "in" (covering, the mark morphing) → "restyle" → "out" (revealing)
Singleton {
    id: root

    readonly property string chosen: Config.options.appearance.look === "material" ? "material" : "glass"
    property string held: ""
    readonly property string shown: held !== "" ? held : chosen

    property bool running: false
    property string phase: ""
    property string from: ""
    property string to: ""

    property string last: ""
    onChosenChanged: {
        if (!Config.ready || !Wallpaper.started || last === "" || Config.options.appearance.reduceMotion) {
            last = chosen;
            return;
        }
        if (chosen === last)
            return;
        if (running) {
            // Changed again mid-switch: head for the latest choice
            to = chosen;
            last = chosen;
            if (phase !== "in")
                held = "";
            return;
        }
        from = last;
        to = chosen;
        last = chosen;
        held = from;
        running = true;
        phase = "in";
        inTimer.restart();
    }
    Connections {
        target: Config
        function onReadyChanged() {
            if (Config.ready)
                root.last = root.chosen;
        }
    }
    Component.onCompleted: if (Config.ready) last = chosen

    // The Material palette the switch is heading to (or leaving), whichever
    // look is shown right now
    readonly property var materialScheme: running ? MaterialColor.MCU.scheme(Theme.materialSeed, Theme.dark, Config.options.appearance.materialStyle, 0) : ({})

    // Covered (the screen fades in and the mark morphs): restyle
    Timer {
        id: inTimer
        interval: 1000
        onTriggered: {
            root.phase = "restyle";
            root.held = "";
            settle.restart();
        }
    }
    // A timer only fires once the main thread is free: the restyle is done
    Timer {
        id: settle
        interval: 350
        onTriggered: {
            root.phase = "out";
            outTimer.restart();
        }
    }
    Timer {
        id: outTimer
        interval: 700
        onTriggered: {
            root.running = false;
            root.phase = "";
        }
    }
}
