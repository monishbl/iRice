import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// "Installing update": a full-screen screen on every display while
// update-run.sh stops the shell, updates, rebuilds and starts the new shell.
// A config of its own (run from a copy, with qs -p), so it stays up while the
// shell restarts. It reads ../status, written by update-run.sh:
//   state \t percent \t text \t detail[\x1f backup folder]
// phase: run | restart | done (fade out, quit) | error (wait for Close)
// While the shell is built it also reads tools/build-progress.py's
// $XDG_STATE_HOME/tessera/build-progress.json (the step, how far, the time);
// Show details opens ../log, the update's whole output.
ShellRoot {
    id: root

    property string phase: "run"
    property real percent: 0
    property string step: "Preparing"
    property string detail: ""
    property string backup: ""
    property bool leaving: false
    property real shown: 0
    // A build under way: {line, time, fraction (-1: not known)}, or null
    property var build: null
    property bool details: false
    property string log: ""
    function clock(s) {
        s = Math.max(0, Math.floor(s));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
    function readBuild(text) {
        let b = null;
        try { b = JSON.parse(text); } catch (e) {}
        const since = b ? Math.max(0, Date.now() / 1000 - (b.updated_at ?? 0)) : 0;
        if (!b || root.phase !== "run" || !["prepare", "download", "compile", "link"].includes(b.phase) || since > 30) {
            root.build = null;
            return;
        }
        const t = b.title || b.label || "";
        const step = b.step || "";
        root.build = {
            line: step.startsWith("Linking") ? step : (b.phase === "link" ? "Linking " : "Building ") + t + (step ? " — " + step.charAt(0).toLowerCase() + step.slice(1) : ""),
            time: root.clock((b.elapsed_s ?? 0) + since) + (b.eta_s === null || b.eta_s === undefined ? "" : " · " + (b.eta_s - since < 60 ? "less than a minute left" : "about " + Math.round((b.eta_s - since) / 60) + " min left")),
            fraction: b.fraction === null || b.fraction === undefined ? -1 : b.fraction
        };
    }
    FileView {
        id: buildFile
        path: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/tessera/build-progress.json"
        printErrors: false
        onLoaded: root.readBuild(text())
        onLoadFailed: root.build = null
    }
    Process {
        id: logTail
        command: ["tail", "-n", "200", Quickshell.shellDir + "/../log"]
        stdout: StdioCollector { onStreamFinished: root.log = this.text.replace(/^(BUILD \{|PROGRESS ).*\n/gm, "") }
    }
    // Up for at least 5 s, however quick the update (it shouldn't flash by)
    property real openedAt: 0
    Component.onCompleted: {
        shown = 1;
        openedAt = Date.now();
    }
    // Your menu icon (written by the shell that started the update: its
    // Phosphor glyph, or your distro's logo), in place of a big wordmark
    property string glyph: ""
    property string image: ""
    FileView {
        path: Quickshell.shellDir + "/../look.json"
        blockLoading: true
        printErrors: false
        onLoaded: {
            try {
                const j = JSON.parse(text());
                root.glyph = j.glyph ?? "";
                root.image = j.image ?? "";
            } catch (e) {}
        }
    }
    FontLoader {
        id: phosphor
        source: Quickshell.shellDir + "/Phosphor-Fill.ttf"
    }
    Behavior on shown { NumberAnimation { duration: root.leaving ? 400 : 300; easing.type: root.leaving ? Easing.InCubic : Easing.OutCubic } }
    Behavior on percent { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }

    FileView {
        id: status
        path: Quickshell.shellDir + "/../status"
        blockLoading: true
        onLoaded: {
            const f = text().trim().split("\t");
            if (f.length < 3)
                return;
            root.phase = f[0];
            root.percent = Math.max(root.percent, Math.min(100, parseFloat(f[1]) || 0));
            root.step = f[2];
            const d = (f[3] ?? "").split("\x1f");
            root.detail = d[0] ?? "";
            root.backup = (d[1] ?? "").replace(Quickshell.env("HOME"), "~");
            if (root.phase === "done" && !root.leaving)
                finish.start();
        }
    }
    // (the file is replaced on each step, so it's read again rather than watched)
    Timer {
        running: root.phase !== "done" && root.phase !== "error"
        interval: 250
        repeat: true
        property int n: 0
        onTriggered: {
            status.reload();
            buildFile.reload();
            if (root.details && ++n % 4 === 1 && !logTail.running)
                logTail.running = true;
        }
    }
    // A last way out if the updater itself is gone (not while a build still
    // goes: the shell's first build can take longer)
    Timer {
        running: root.phase !== "error"
        interval: 15 * 60 * 1000
        repeat: true
        onTriggered: {
            if (!root.build)
                Qt.quit();
        }
    }
    SequentialAnimation {
        id: finish
        // "Update installed", briefly, and the whole screen at least 5 s
        PauseAnimation { duration: Math.max(600, 5000 - (Date.now() - root.openedAt)) }
        ScriptAction { script: { root.leaving = true; root.shown = 0; } }
        PauseAnimation { duration: 450 }
        ScriptAction { script: Qt.quit() }
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "tessera:update"
            WlrLayershell.keyboardFocus: root.phase === "error" ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            color: "transparent"
            // Clicks do nothing while it works but Show details (and the
            // output, once open); the error screen's Close takes them all
            mask: Region { item: root.phase === "error" ? fullArea : detailsArea }
            Item { id: fullArea; anchors.fill: parent }

            Rectangle {
                anchors.fill: parent
                color: "#0A0A0B"
                opacity: root.shown

                Column {
                    id: col
                    anchors.centerIn: parent
                    width: Math.min(root.details && root.phase !== "error" ? 640 : 360, parent.width - 64)
                    spacing: 0
                    // (moves up a little as it appears)
                    transform: Translate { y: (1 - root.shown) * 8 }

                    // Your menu icon
                    Item {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 56
                        height: 56
                        Text {
                            anchors.centerIn: parent
                            visible: root.image === ""
                            text: root.glyph !== "" && phosphor.status === FontLoader.Ready ? root.glyph : ""
                            color: "#EDEDEF"
                            font { family: phosphor.name; pixelSize: 52 }
                        }
                        Image {
                            anchors.centerIn: parent
                            visible: root.image !== ""
                            source: root.image !== "" ? "file://" + root.image : ""
                            sourceSize: Qt.size(52, 52)
                        }
                    }
                    Item { width: 1; height: 28 }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.phase === "error" ? root.step : root.phase === "done" ? "Tessera is up to date" : root.phase === "restart" ? "Tessera is restarting" : "Tessera is updating"
                        color: "#EDEDEF"
                        font { family: "Inter"; pixelSize: 14; weight: Font.Medium }
                    }
                    Item { width: 1; height: 14 }

                    // Progress: the update's, or the build's while one runs
                    Rectangle {
                        visible: root.phase !== "error"
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.min(200, parent.width)
                        height: 4
                        radius: 2
                        color: "#2C2C31"
                        clip: true
                        Rectangle {
                            id: fill
                            readonly property bool sweep: !!root.build && root.build.fraction < 0
                            property real t: 0
                            NumberAnimation on t { running: fill.sweep; loops: Animation.Infinite; from: 0; to: 1; duration: 1600 }
                            x: sweep ? -parent.width * 0.3 + parent.width * 1.3 * t : 0
                            width: sweep ? parent.width * 0.3 : Math.max(height, parent.width * (root.build ? root.build.fraction * 100 : root.percent) / 100)
                            height: parent.height
                            radius: 2
                            color: "#EDEDEF"
                        }
                    }
                    Item { visible: root.phase !== "error"; width: 1; height: 12 }
                    Text {
                        visible: root.phase !== "error"
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.phase === "done" ? "Your settings and shortcuts are as you left them." : root.build ? root.build.line : root.step + "…"
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 12 }
                    }
                    Text {
                        visible: !!root.build
                        anchors.horizontalCenter: parent.horizontalCenter
                        topPadding: 4
                        text: root.build?.time ?? ""
                        color: "#94949D"
                        font { family: "Inter"; pixelSize: 12 }
                    }
                    // Show details: the update's output, as it comes
                    Item { visible: root.phase === "run" || root.phase === "restart"; width: 1; height: 16 }
                    Column {
                        id: detailsArea
                        visible: root.phase === "run" || root.phase === "restart"
                        width: parent.width
                        spacing: 8
                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: toggleText.implicitWidth + 24
                            height: 32
                            radius: 10
                            color: toggleMouse.pressed ? "#333338" : toggleMouse.containsMouse ? "#2C2C31" : "transparent"
                            Text {
                                id: toggleText
                                anchors.centerIn: parent
                                text: root.details ? "Hide details" : "Show details"
                                color: "#A1A1AA"
                                font { family: "Inter"; pixelSize: 12; weight: Font.Medium }
                            }
                            MouseArea {
                                id: toggleMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.details = !root.details;
                                    if (root.details)
                                        logTail.running = true;
                                }
                            }
                        }
                        Rectangle {
                            visible: root.details
                            width: parent.width
                            height: 240
                            radius: 14
                            color: "#1A1A1D"
                            border { width: 1; color: "#26262A" }
                            clip: true
                            Flickable {
                                id: logFlick
                                anchors { fill: parent; leftMargin: 12; rightMargin: 12; topMargin: 10; bottomMargin: 10 }
                                contentHeight: logText.implicitHeight
                                clip: true
                                property bool atEnd: true
                                onContentYChanged: atEnd = contentY >= contentHeight - height - 4
                                onContentHeightChanged: if (atEnd) contentY = Math.max(0, contentHeight - height)
                                Text {
                                    id: logText
                                    width: logFlick.width
                                    text: root.log === "" ? "Waiting for output…" : root.log
                                    wrapMode: Text.WrapAnywhere
                                    color: "#D4D4D8"
                                    font { family: "JetBrains Mono"; pixelSize: 11 }
                                }
                            }
                            Rectangle {
                                anchors { top: parent.top; right: parent.right; margins: 8 }
                                width: copyText.implicitWidth + 20
                                height: 28
                                radius: 10
                                color: copyMouse.pressed ? "#36363C" : copyMouse.containsMouse ? "#333338" : "#2C2C31"
                                Text {
                                    id: copyText
                                    anchors.centerIn: parent
                                    text: "Copy"
                                    color: "#EDEDEF"
                                    font { family: "Inter"; pixelSize: 12; weight: Font.Medium }
                                }
                                MouseArea {
                                    id: copyMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: Quickshell.execDetached(["sh", "-c", "wl-copy < \"$1\"", "sh", Quickshell.shellDir + "/../log"])
                                }
                            }
                        }
                    }

                    // Something went wrong: what, that nothing was lost, and Close
                    Text {
                        visible: root.phase === "error"
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.detail
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 13 }
                    }
                    Item { visible: root.phase === "error"; width: 1; height: 8 }
                    Text {
                        visible: root.phase === "error"
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: "Nothing was lost." + (root.backup !== "" ? " Your settings and shortcuts were backed up to " + root.backup + "." : "")
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 13 }
                    }
                    Item { visible: root.phase === "error"; width: 1; height: 24 }
                    Rectangle {
                        id: close
                        visible: root.phase === "error"
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: closeText.implicitWidth + 40
                        height: 36
                        radius: 10
                        color: closeMouse.pressed ? "#C7C7CC" : closeMouse.containsMouse ? "#DADADF" : "#EDEDEF"
                        scale: closeMouse.pressed ? 0.97 : 1
                        Behavior on scale { NumberAnimation { duration: 100 } }
                        Text {
                            id: closeText
                            anchors.centerIn: parent
                            text: "Close"
                            color: "#18181B"
                            font { family: "Inter"; pixelSize: 14; weight: Font.Medium }
                        }
                        MouseArea {
                            id: closeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: Qt.quit()
                        }
                    }
                }

                Item {
                    anchors.fill: parent
                    focus: root.phase === "error"
                    Keys.onReturnPressed: Qt.quit()
                    Keys.onEscapePressed: Qt.quit()
                }
            }
        }
    }
}
