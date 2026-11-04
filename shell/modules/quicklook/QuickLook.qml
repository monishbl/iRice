import QtQuick
import QtQuick.Controls
import QtMultimedia
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.screentools.editor
import qs.services

// Quick Look (Panels.quickLook(path)): a preview of a file over whatever is
// open: images, video and audio, the first page of a PDF, text and code, a
// folder's contents, or the file's details. Space or Esc closes it, Return
// opens the file in its app. A picture's Text button (T) turns on Live Text:
// its words to select and copy, links and numbers as chips (the screenshot
// editor's LiveTextLayer and LiveTextBar).
Scope {
    id: scope

    LazyLoader {
        id: loader
        // Kept a moment after closing, for the exit animation
        property bool alive: false
        active: alive
        Component.onCompleted: alive = Panels.quickLookPath !== ""
        property Connections watch: Connections {
            target: Panels
            function onQuickLookPathChanged() {
                if (Panels.quickLookPath !== "") {
                    unload.stop();
                    loader.alive = true;
                } else {
                    unload.restart();
                }
            }
        }
        property Timer unload: Timer {
            id: unload
            interval: 300
            onTriggered: loader.alive = false
        }

        GlassWindow {
            id: win
            name: "quicklook"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            readonly property bool shown: Panels.quickLookPath !== ""
            // The file shown (kept while it animates out)
            property string path: Panels.quickLookPath
            onShownChanged: if (shown) path = Panels.quickLookPath
            Connections {
                target: Panels
                function onQuickLookPathChanged() {
                    if (Panels.quickLookPath !== "")
                        win.path = Panels.quickLookPath;
                }
            }
            readonly property string fileName: (path || Panels.quickLookPath).split("/").pop()
            readonly property string ext: fileName.includes(".") ? fileName.split(".").pop().toLowerCase() : ""

            // Live Text on a picture, and what Search found it by (opened
            // from Search by the words in it: those, selected)
            property bool textMode: false
            property string textQuery: ""
            function takeQuery() {
                const q = LiveText.takeQuery(path);
                if (q !== "") {
                    textQuery = q;
                    textMode = true;
                }
            }

            // What it is: from the file's own type (mime), else its extension
            property string mime: ""
            property var info: ({ size: 0, modified: "" })
            Component.onDestruction: player.stop()
            onPathChanged: if (path) {
                textMode = false;
                textQuery = "";
                takeQuery();
                mime = "";
                probe.running = false;
                Qt.callLater(() => probe.running = true); // (a restart in the same tick is ignored)
            }
            Process {
                id: probe
                property string asked: ""
                onRunningChanged: if (running) asked = win.path
                command: ["sh", "-c", "[ -d \"$1\" ] && echo inode/directory || file -b --mime-type -- \"$1\" 2>/dev/null; stat -c '%s\t%y' -- \"$1\" 2>/dev/null", "sh", win.path]
                stdout: StdioCollector {
                    onStreamFinished: {
                        if (probe.asked !== win.path)
                            return;
                        const l = text.split("\n");
                        win.mime = l[0]?.trim() ?? "";
                        const [size, mod] = (l[1] ?? "").split("\t");
                        win.info = { size: parseInt(size) || 0, modified: (mod ?? "").slice(0, 16) };
                    }
                }
            }
            readonly property string kind: {
                const m = mime, e = ext;
                if (m === "inode/directory")
                    return "folder";
                if (m.startsWith("image/") || ["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "avif"].includes(e))
                    return "image";
                if (m.startsWith("video/") || ["mp4", "mkv", "webm", "mov", "avi"].includes(e))
                    return "video";
                if (m.startsWith("audio/") || ["mp3", "flac", "ogg", "wav", "m4a", "opus"].includes(e))
                    return "audio";
                if (m === "application/pdf" || e === "pdf")
                    return "pdf";
                if (m.startsWith("text/") || m === "application/json" || m.endsWith("+xml") || m === "application/javascript" || m === "inode/x-empty" || ["md", "txt", "json", "yaml", "yml", "toml", "ini", "conf", "lua", "qml", "js", "ts", "py", "sh", "c", "cpp", "h", "rs", "go", "css", "html", "log", "csv"].includes(e))
                    return "text";
                return m === "" ? "" : "other";
            }

            // Starts at 0 so it animates in when created
            property real progress: 0
            Component.onCompleted: {
                progress = Qt.binding(() => shown ? 1 : 0);
                takeQuery(); // (the first file: no change of path)
            }
            Behavior on progress { NumberAnimation { duration: win.shown ? 200 : 140; easing.type: win.shown ? Easing.OutCubic : Easing.InCubic } }
            visible: progress > 0.01 || shown
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }

            function close() {
                Panels.quickLookPath = "";
            }
            function openIt() {
                Files.open(path);
                close();
            }

            Rectangle {
                anchors.fill: parent
                color: Theme.material ? Qt.alpha(Theme.m3.scrim, 0.32 * win.progress) : Qt.rgba(0, 0, 0, 0.3 * win.progress)
                MouseArea {
                    anchors.fill: parent
                    onClicked: win.close()
                }
            }

            Item {
                id: card
                focus: true
                Keys.onPressed: e => {
                    // Live Text: copy, select all, clear the selection first; T turns it on or off
                    const ctrl = e.modifiers & Qt.ControlModifier;
                    if (win.kind === "image" && e.key === Qt.Key_T && !ctrl) {
                        win.textMode = !win.textMode;
                        e.accepted = true;
                        return;
                    }
                    if (win.textMode && liveText.status === "ready") {
                        if (ctrl && e.key === Qt.Key_C) {
                            liveBar.say(liveText.copy());
                            e.accepted = true;
                            return;
                        }
                        if (ctrl && e.key === Qt.Key_A) {
                            liveText.selectAll();
                            e.accepted = true;
                            return;
                        }
                        if (e.key === Qt.Key_Escape && liveText.hasSelection) {
                            liveText.clear();
                            e.accepted = true;
                            return;
                        }
                    }
                    if (e.key === Qt.Key_Space || e.key === Qt.Key_Escape) {
                        win.close();
                        e.accepted = true;
                    } else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                        win.openIt();
                        e.accepted = true;
                    }
                }
                // Size: the preview's own shape, within 80% of the screen
                readonly property real maxW: win.width * 0.8
                readonly property real maxH: win.height * 0.8 - header.height - footer
                // (Live Text's bar, under the picture)
                readonly property real footer: win.textMode && win.kind === "image" ? liveBar.height + 12 : 0
                readonly property size natural: win.kind === "image" && image.status === Image.Ready ? Qt.size(image.implicitWidth, image.implicitHeight)
                    : win.kind === "video" ? Qt.size(1280, 720)
                    : win.kind === "pdf" ? Qt.size(page.implicitWidth || 850, page.implicitHeight || 1100)
                    : win.kind === "text" || win.kind === "folder" ? Qt.size(760, 900)
                    : Qt.size(420, 260)
                readonly property real k: Math.min(1, maxW / Math.max(1, natural.width), maxH / Math.max(1, natural.height))
                width: Math.max(360, natural.width * k)
                height: natural.height * k + header.height + footer
                anchors.centerIn: parent
                opacity: win.progress
                scale: 0.94 + 0.06 * win.progress

                Glass {
                    anchors.fill: parent
                    anchors.margins: -shadowMargin
                    radius: Theme.material ? Theme.radius.panel : 20
                }
                MouseArea { anchors.fill: parent } // clicks on the card don't close it

                // Title bar: name, what it is, Open, close
                Item {
                    id: header
                    width: parent.width
                    height: 52
                    IconButton {
                        id: closeBtn
                        anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                        icon: "x"
                        onClicked: win.close()
                    }
                    Column {
                        anchors { left: closeBtn.right; leftMargin: 10; right: textBtn.visible ? textBtn.left : shareBtn.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                        Text {
                            width: parent.width
                            text: win.fileName
                            elide: Text.ElideMiddle
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            text: [win.kind === "folder" ? folder.count + (folder.count === 1 ? " item" : " items") : Transfers.size(win.info.size), win.info.modified].filter(x => x).join(" · ")
                            elide: Text.ElideRight
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                        }
                    }
                    // A picture: Live Text on or off
                    EditorButton {
                        id: textBtn
                        anchors { right: shareBtn.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                        visible: win.kind === "image"
                        icon: "text-aa"
                        label: "Text"
                        selected: win.textMode
                        tooltip: win.textMode ? "Stop selecting text" : "Select the text in this picture"
                        shortcut: "T"
                        enabled: image.status === Image.Ready
                        onClicked: win.textMode = !win.textMode
                    }
                    IconButton {
                        id: shareBtn
                        anchors { right: openBtn.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                        visible: win.kind !== "folder"
                        icon: "share-network"
                        tooltip: "Share"
                        onClicked: {
                            const p = win.path;
                            win.close();
                            Panels.share([p]);
                        }
                    }
                    // (Material: a filled primary pill with state layers, its
                    // corners tightening while pressed)
                    Squircle {
                        id: openBtn
                        anchors { right: parent.right; rightMargin: Theme.material ? 16 : 12; verticalCenter: parent.verticalCenter }
                        width: openText.implicitWidth + (Theme.material ? 32 : 28)
                        height: Theme.material ? 36 : 28
                        // (Material's press swell)
                        transform: Theme.material ? pressFx1.effects : []
                        ElasticControl { id: pressFx1; enabled: Theme.material; pressed: Theme.material && openMouse.pressed }
                        radius: Theme.material ? (openMouse.pressed ? 8 : height / 2) : Theme.radius.control
                        Behavior on radius {
                            enabled: Theme.material
                            MaterialSpring { speed: "fast"; epsilon: 0.25 }
                        }
                        color: Theme.material ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, openMouse.pressed ? 0.1 : openMouse.containsMouse ? 0.08 : 0))
                            : openMouse.pressed ? Qt.darker(Theme.accent, 1.2) : openMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent
                        Text {
                            id: openText
                            anchors.centerIn: parent
                            text: win.kind === "folder" ? (Theme.material ? "Open folder" : "Open Folder") : (Theme.material ? "Open file" : "Open File")
                            color: Theme.color.onAccent
                            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium }
                        }
                        MouseArea {
                            id: openMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: win.openIt()
                        }
                    }
                }

                // Live Text: what it found, chips and copying
                LiveTextBar {
                    id: liveBar
                    live: liveText
                    maxWidth: card.width - 24
                    visible: win.textMode && win.kind === "image" && status !== ""
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 12 }
                }

                // The preview
                Item {
                    id: body
                    anchors { left: parent.left; right: parent.right; top: header.bottom; bottom: parent.bottom; margins: 12; topMargin: 0; bottomMargin: 12 + card.footer }
                    clip: true

                    Image {
                        id: image
                        anchors.fill: parent
                        visible: win.kind === "image"
                        source: win.kind === "image" ? "file://" + win.path : ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                        mipmap: true
                        sourceSize: Qt.size(card.maxW * 1.5, (card.maxH + card.footer) * 1.5) // (the same with Live Text on: not loaded again)
                    }
                    // Live Text over the picture as drawn
                    LiveTextLayer {
                        id: liveText
                        x: (image.width - image.paintedWidth) / 2
                        y: (image.height - image.paintedHeight) / 2
                        width: image.paintedWidth
                        height: image.paintedHeight
                        active: win.textMode && win.kind === "image" && image.status === Image.Ready
                        path: win.path
                        highlight: win.textQuery
                    }

                    // PDF: the first page, rendered once into the cache
                    property string pagePng: ""
                    Process {
                        running: win.kind === "pdf" && Tools.has("pdftoppm")
                        command: ["sh", "-c", "d=\"${XDG_CACHE_HOME:-$HOME/.cache}/tessera/quicklook\"; mkdir -p \"$d\"; o=\"$d/$(printf '%s' \"$1\" | md5sum | cut -c1-16)\"; [ -f \"$o.png\" ] || pdftoppm -png -r 110 -f 1 -l 1 -singlefile -- \"$1\" \"$o\" 2>/dev/null; echo \"$o.png\"", "sh", win.path]
                        stdout: StdioCollector { onStreamFinished: body.pagePng = text.trim() }
                    }
                    Image {
                        id: page
                        anchors.fill: parent
                        visible: win.kind === "pdf"
                        source: win.kind === "pdf" && body.pagePng ? "file://" + body.pagePng : ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                    }

                    // Video and audio
                    MediaPlayer {
                        id: player
                        source: win.kind === "video" || win.kind === "audio" ? "file://" + win.path : ""
                        videoOutput: video
                        audioOutput: AudioOutput {}
                        Component.onCompleted: if (source != "") play()
                        onSourceChanged: if (source != "") play()
                    }
                    VideoOutput {
                        id: video
                        anchors.fill: parent
                        visible: win.kind === "video"
                    }
                    Column {
                        visible: win.kind === "audio" || win.kind === "other" || win.kind === "folder" && folder.count === 0
                        anchors.centerIn: parent
                        spacing: 16
                        // (Material: the file's icon on a shape)
                        Item {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Theme.material ? 144 : 96
                            height: width
                            Loader {
                                anchors.fill: parent
                                active: Theme.material
                                sourceComponent: MaterialShape {
                                    shape: win.kind === "audio" ? "sunny" : "cookie12"
                                    color: Theme.m3.surfaceContainerHighest
                                }
                            }
                            AppIcon {
                                anchors.centerIn: parent
                                size: 96
                                styled: false
                                source: IconLookup.path(win.kind === "folder" ? "folder" : Search.iconForFile(win.path), "text-x-generic")
                            }
                        }
                        IconButton {
                            visible: !Theme.material && win.kind === "audio"
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 44
                            height: 44
                            icon: player.playbackState === MediaPlayer.PlayingState ? "pause" : "play"
                            fill: true
                            iconSize: 22
                            iconColor: Theme.color.label
                            onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
                        }
                        // Material: play on a big primary-container shape
                        // that morphs, a rounded square while paused and a
                        // circle while it plays, as Android's media controls
                        Loader {
                            active: Theme.material && win.kind === "audio"
                            visible: active
                            anchors.horizontalCenter: parent.horizontalCenter
                            sourceComponent: Item {
                                id: playBtn
                                readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
                                width: 64
                                height: 64
                                activeFocusOnTab: true
                                Accessible.role: Accessible.Button
                                Accessible.name: playing ? "Pause" : "Play"
                                Keys.onSpacePressed: toggle()
                                function toggle() {
                                    playing ? player.pause() : player.play();
                                }
                                MaterialShape {
                                    anchors.fill: parent
                                    transform: pressFx2.effects
                                    ElasticControl { id: pressFx2; pressed: playMouse.pressed }
                                    shape: playMouse.pressed ? "cookie9" : playBtn.playing ? "circle" : "square"
                                    color: Qt.tint(Theme.m3.primaryContainer, Qt.alpha(Theme.m3.onPrimaryContainer, playMouse.pressed ? 0.1 : playMouse.containsMouse ? 0.08 : 0))
                                }
                                Icon {
                                    anchors.centerIn: parent
                                    name: playBtn.playing ? "pause" : "play"
                                    fill: true
                                    size: 28
                                    color: Theme.m3.onPrimaryContainer
                                }
                                MouseArea {
                                    id: playMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: playBtn.toggle()
                                }
                            }
                        }
                        Text {
                            visible: win.kind !== "audio"
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: win.kind === "folder" ? "Empty folder" : "No preview for this kind of file"
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                    }
                    // Video: a thin progress line and click to pause
                    MouseArea {
                        anchors.fill: parent
                        visible: win.kind === "video"
                        onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
                    }
                    // (Material: its wavy progress)
                    Loader {
                        active: Theme.material && (win.kind === "video" || win.kind === "audio")
                        visible: active
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                        sourceComponent: WavyProgress {
                            value: player.duration > 0 ? player.position / player.duration : 0
                            paused: player.playbackState !== MediaPlayer.PlayingState
                        }
                    }
                    Rectangle {
                        visible: !Theme.material && (win.kind === "video" || win.kind === "audio")
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                        height: 3
                        radius: 1.5
                        color: Theme.color.fill
                        Rectangle {
                            width: parent.width * (player.duration > 0 ? player.position / player.duration : 0)
                            height: parent.height
                            radius: 1.5
                            color: Theme.accent
                        }
                    }

                    // Text and code: the first 200 KB
                    FileView {
                        id: textFile
                        path: win.kind === "text" ? win.path : ""
                        blockLoading: false
                    }
                    ScrollView {
                        anchors.fill: parent
                        visible: win.kind === "text"
                        TextArea {
                            readOnly: true
                            text: win.kind === "text" ? textFile.text().slice(0, 200000) : ""
                            wrapMode: TextEdit.Wrap
                            color: Theme.color.label
                            selectionColor: Theme.accentAlpha(0.4)
                            font { family: Theme.font.mono; pixelSize: 12 }
                            background: null
                        }
                    }

                    // Folder: what's inside
                    FolderListModel {
                        id: folder
                        folder: win.kind === "folder" ? "file://" + win.path : ""
                        showDirsFirst: true
                    }
                    ListView {
                        Momentum {}
                        anchors.fill: parent
                        visible: win.kind === "folder" && folder.count > 0
                        model: folder
                        clip: true
                        delegate: Row {
                            required property string fileName
                            required property string filePath
                            required property bool fileIsDir
                            height: 32
                            spacing: 10
                            AppIcon {
                                anchors.verticalCenter: parent.verticalCenter
                                size: 20
                                styled: false
                                source: IconLookup.path(parent.fileIsDir ? "folder" : Search.iconForFile(parent.filePath), "text-x-generic")
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.fileName
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                        }
                    }
                }
            }
        }
    }
}
