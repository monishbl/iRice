import QtQuick
import QtQuick.Controls.Basic
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.controlcenter
import qs.services

// Overlay (Super+G): widgets over everything. While open the screen dims and a
// toolbar toggles widgets; pinned widgets stay after closing (optionally
// click-through). A crosshair can be drawn at the screen centre.
Scope {
    id: root

    readonly property bool open: Panels.open === "overlay"
    readonly property var ids: ["notes", "resources", "recorder", "mixer", "image", "crosshair", "fps"]
    readonly property var meta: ({
            notes: ["Notes", "note"],
            resources: ["Resources", "cpu"],
            recorder: ["Recorder", "record"],
            mixer: ["Volume Mixer", "speaker-high"],
            image: ["Floating Image", "image"],
            crosshair: ["Crosshair", "crosshair"],
            fps: ["FPS Limit", "gauge"]
        })
    readonly property bool anyPinned: ids.some(id => UiState.widget(id).open && UiState.widget(id).pinned)
    readonly property bool crosshairOn: UiState.data.crosshair.enabled

    Binding {
        target: Resources
        property: "overlayActive"
        value: UiState.widget("resources").open && (root.open || UiState.widget("resources").pinned)
    }

    LazyLoader {
        active: root.open || root.anyPinned || root.crosshairOn

        GlassWindow {
            id: win
            name: "overlay"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: root.open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // Closed: only pinned, clickable widgets take input
            mask: Region {
                item: root.open ? full : null
                regions: root.open ? [] : pinnedRegions.instances
            }
            Item { id: full; anchors.fill: parent }
            Variants {
                id: pinnedRegions
                model: widgets.children.filter(w => w.widgetId && w.visible && !UiState.widget(w.widgetId).clickthrough)
                delegate: Region {
                    required property var modelData
                    item: modelData
                }
            }

            Rectangle {
                anchors.fill: parent
                color: "black"
                opacity: root.open ? 0.3 : 0
                Behavior on opacity { OpacityAnimator { duration: Theme.motion.popover } }
                MouseArea {
                    anchors.fill: parent
                    enabled: root.open
                    onClicked: Panels.close()
                }
            }

            Item {
                anchors.fill: parent
                focus: root.open
                Keys.onEscapePressed: Panels.close()
            }

            // Crosshair at the centre of the screen
            Item {
                visible: root.crosshairOn
                anchors.centerIn: parent
                readonly property var c: UiState.data.crosshair
                width: 2 * (c.size + c.gap)
                height: width
                Repeater {
                    model: 4
                    Rectangle {
                        required property int index
                        readonly property var c: parent.c
                        readonly property bool horizontal: index < 2
                        color: c.color
                        width: horizontal ? c.size : c.thickness
                        height: horizontal ? c.thickness : c.size
                        x: horizontal ? (index === 0 ? 0 : c.size + 2 * c.gap) : parent.width / 2 - c.thickness / 2
                        y: horizontal ? parent.height / 2 - c.thickness / 2 : (index === 2 ? 0 : c.size + 2 * c.gap)
                    }
                }
                Rectangle {
                    visible: parent.c.dot
                    anchors.centerIn: parent
                    width: parent.c.thickness + 1
                    height: width
                    radius: width / 2
                    color: parent.c.color
                }
            }

            // Toolbar
            Item {
                id: toolbar
                visible: root.open
                anchors.horizontalCenter: parent.horizontalCenter
                y: 44
                width: toolRow.implicitWidth + 20 + 40
                height: 52 + 40

                Glass {
                    anchors.fill: parent
                    radius: 26
                    bevel: 18
                    shadowMargin: 20
                }
                Row {
                    id: toolRow
                    anchors.centerIn: parent
                    spacing: 4
                    Repeater {
                        model: root.ids
                        Rectangle {
                            id: toggle
                            required property string modelData
                            readonly property bool on: UiState.widget(modelData).open
                            width: 40
                            height: 40
                            radius: 20
                            color: on ? Theme.accent : tMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            Icon {
                                anchors.centerIn: parent
                                name: root.meta[toggle.modelData][1]
                                fill: toggle.on
                                size: 18
                                color: toggle.on ? Theme.color.onAccent : Theme.color.label
                            }
                            MouseArea {
                                id: tMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: UiState.setWidget(toggle.modelData, { open: !toggle.on })
                            }
                        }
                    }
                }
            }

            Item {
                id: widgets
                anchors.fill: parent

                OverlayWidget {
                    widgetId: "notes"
                    title: root.meta.notes[0]
                    icon: root.meta.notes[1]
                    overlayOpen: root.open
                    ScrollView {
                        anchors.fill: parent
                        TextArea {
                            id: notes
                            wrapMode: TextEdit.Wrap
                            placeholderText: "Jot something down"
                            placeholderTextColor: Theme.color.tertiary
                            color: Theme.color.label
                            background: null
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                            text: notesFile.loaded ? notesFile.text() : ""
                            onTextChanged: if (activeFocus) saveNotes.restart()
                        }
                    }
                    FileView {
                        id: notesFile
                        path: UiState.dir + "/notes.md"
                        property bool loaded: false
                        onLoaded: loaded = true
                        onLoadFailed: loaded = true
                    }
                    Timer {
                        id: saveNotes
                        interval: 600
                        onTriggered: notesFile.setText(notes.text)
                    }
                }

                OverlayWidget {
                    widgetId: "resources"
                    title: root.meta.resources[0]
                    icon: root.meta.resources[1]
                    overlayOpen: root.open
                    Column {
                        anchors.fill: parent
                        spacing: 10
                        Repeater {
                            model: [
                                { label: "CPU", value: Resources.cpu, detail: Math.round(Resources.cpu * 100) + "%" + (Resources.temperature > 0 ? " · " + Math.round(Resources.temperature) + "°C" : ""), history: Resources.cpuHistory },
                                { label: "Memory", value: Resources.memory, detail: Resources.memUsedGb.toFixed(1) + " / " + Resources.memTotalGb.toFixed(1) + " GB", history: Resources.memHistory }
                            ]
                            Column {
                                id: metric
                                required property var modelData
                                width: parent.width
                                spacing: 4
                                Item {
                                    width: parent.width
                                    height: 16
                                    Text { text: metric.modelData.label; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                                    Text { anchors.right: parent.right; text: metric.modelData.detail; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } } }
                                }
                                Canvas {
                                    id: graph
                                    width: parent.width
                                    height: 52
                                    readonly property var history: metric.modelData.history
                                    onHistoryChanged: requestPaint()
                                    onPaint: {
                                        const ctx = getContext("2d");
                                        ctx.reset();
                                        const h = history, n = Resources.historyLength;
                                        if (h.length < 2)
                                            return;
                                        const step = width / (n - 1), off = (n - h.length) * step;
                                        ctx.beginPath();
                                        ctx.moveTo(off, height);
                                        for (let i = 0; i < h.length; i++)
                                            ctx.lineTo(off + i * step, height - h[i] * (height - 2));
                                        ctx.lineTo(off + (h.length - 1) * step, height);
                                        ctx.closePath();
                                        ctx.fillStyle = Theme.accentAlpha(0.25);
                                        ctx.fill();
                                        ctx.beginPath();
                                        for (let i = 0; i < h.length; i++)
                                            ctx.lineTo(off + i * step, height - h[i] * (height - 2));
                                        ctx.strokeStyle = Theme.accent;
                                        ctx.lineWidth = 1.5;
                                        ctx.stroke();
                                    }
                                }
                            }
                        }
                    }
                }

                OverlayWidget {
                    widgetId: "recorder"
                    title: root.meta.recorder[0]
                    icon: root.meta.recorder[1]
                    overlayOpen: root.open
                    minHeight: 120
                    Column {
                        anchors.fill: parent
                        spacing: 10
                        Text {
                            text: ScreenTools.recording ? "Recording…" : "Not recording"
                            color: ScreenTools.recording ? Theme.color.danger : Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                        }
                        Row {
                            spacing: 8
                            Repeater {
                                model: ScreenTools.recording ? [{ text: "Stop", act: () => ScreenTools.stopRecording(), primary: true }] : [{ text: "Record screen", act: () => ScreenTools.recordScreen(), primary: true }, { text: "Record area", act: () => ScreenTools.select("record"), primary: false }]
                                Rectangle {
                                    required property var modelData
                                    width: rText.implicitWidth + 24
                                    height: 30
                                    radius: 15
                                    color: modelData.primary ? (ScreenTools.recording ? Theme.color.danger : Theme.accent) : Theme.color.fill
                                    Text { id: rText; anchors.centerIn: parent; text: parent.modelData.text; color: parent.modelData.primary ? Theme.color.onAccent : Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                                    MouseArea { anchors.fill: parent; onClicked: parent.modelData.act() }
                                }
                            }
                        }
                        Text {
                            text: "Saved to ~/Videos"
                            color: Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }

                OverlayWidget {
                    id: mixerW
                    widgetId: "mixer"
                    title: root.meta.mixer[0]
                    icon: root.meta.mixer[1]
                    overlayOpen: root.open
                    readonly property var streams: Pipewire.nodes.values.filter(n => n.isSink && n.isStream && n.audio)
                    PwObjectTracker { objects: mixerW.streams }
                    Flickable {
                        anchors.fill: parent
                        contentHeight: mixCol.implicitHeight
                        clip: true
                        Column {
                            id: mixCol
                            width: parent.width
                            spacing: 12
                            CCSlider {
                                width: parent.width
                                label: "Output"
                                icon: Audio.icon
                                value: Audio.muted ? 0 : Audio.volume
                                onMoved: v => Audio.setVolume(v)
                            }
                            Repeater {
                                model: mixerW.streams
                                CCSlider {
                                    required property var modelData
                                    width: mixCol.width
                                    label: modelData.properties["application.name"] ?? modelData.description ?? modelData.name
                                    icon: modelData.audio.muted ? "speaker-x" : "speaker-high"
                                    value: modelData.audio.muted ? 0 : modelData.audio.volume
                                    onMoved: v => { modelData.audio.muted = false; modelData.audio.volume = v; }
                                }
                            }
                        }
                    }
                }

                OverlayWidget {
                    widgetId: "image"
                    title: root.meta.image[0]
                    icon: root.meta.image[1]
                    overlayOpen: root.open
                    Image {
                        anchors.fill: parent
                        anchors.bottomMargin: root.open ? 36 : 0
                        source: UiState.data.floatingImage ? "file://" + UiState.data.floatingImage + "?" + imgPaste.stamp : ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        cache: false
                    }
                    Text {
                        anchors.centerIn: parent
                        visible: !UiState.data.floatingImage
                        text: "Copy an image, then paste it here"
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    Rectangle {
                        visible: root.open
                        anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter }
                        width: pText.implicitWidth + 24
                        height: 28
                        radius: 14
                        color: Theme.color.fill
                        Text { id: pText; anchors.centerIn: parent; text: "Paste image"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                        MouseArea { anchors.fill: parent; onClicked: imgPaste.running = true }
                    }
                    Process {
                        id: imgPaste
                        property int stamp: 0
                        command: ["sh", "-c", "wl-paste --type image/png > \"$1\" && echo ok", "sh", UiState.dir + "/floating.png"]
                        stdout: StdioCollector {
                            onStreamFinished: {
                                if (text.trim() === "ok") {
                                    UiState.data.floatingImage = UiState.dir + "/floating.png";
                                    imgPaste.stamp++;
                                }
                            }
                        }
                    }
                }

                OverlayWidget {
                    id: crossW
                    widgetId: "crosshair"
                    title: root.meta.crosshair[0]
                    icon: root.meta.crosshair[1]
                    overlayOpen: root.open
                    readonly property var c: UiState.data.crosshair
                    function set(patch) { UiState.data.crosshair = Object.assign({}, c, patch); }
                    Column {
                        anchors.fill: parent
                        spacing: 8
                        Item {
                            width: parent.width
                            height: 24
                            Text { anchors.verticalCenter: parent.verticalCenter; text: "Show crosshair"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small } }
                            Switch { anchors { right: parent.right; verticalCenter: parent.verticalCenter } checked: crossW.c.enabled; onToggled: crossW.set({ enabled: !checked }) }
                        }
                        Row {
                            spacing: 6
                            Repeater {
                                model: ["#30D158", "#FF453A", "#0A84FF", "#FFD60A", "#FFFFFF", "#FF375F"]
                                Rectangle {
                                    required property string modelData
                                    width: 20; height: 20; radius: 10
                                    color: modelData
                                    border.width: UiState.data.crosshair.color === modelData ? 2 : 0
                                    border.color: Theme.color.label
                                    MouseArea { anchors.fill: parent; onClicked: UiState.data.crosshair = Object.assign({}, UiState.data.crosshair, { color: parent.modelData }) }
                                }
                            }
                        }
                        Repeater {
                            model: [["Size", "size", 4, 40], ["Thickness", "thickness", 1, 8], ["Gap", "gap", 0, 20]]
                            Item {
                                required property var modelData
                                width: parent.width
                                height: 22
                                Text { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData[0]; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.small } }
                                Slider {
                                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                    width: parent.width - 80
                                    from: parent.modelData[2]
                                    to: parent.modelData[3]
                                    stepSize: 1
                                    value: UiState.data.crosshair[parent.modelData[1]]
                                    onMoved: v => {
                                        const p = {};
                                        p[parent.modelData[1]] = v;
                                        UiState.data.crosshair = Object.assign({}, UiState.data.crosshair, p);
                                    }
                                }
                            }
                        }
                    }
                }

                OverlayWidget {
                    id: fpsW
                    widgetId: "fps"
                    title: root.meta.fps[0]
                    icon: root.meta.fps[1]
                    overlayOpen: root.open
                    minHeight: 120
                    property string status: ""
                    function setLimit(n) {
                        fpsProc.command = ["sh", "-c", "for f in \"$HOME/.config/MangoHud/MangoHud.conf\"; do mkdir -p \"$(dirname \"$f\")\"; touch \"$f\"; if grep -q '^fps_limit=' \"$f\"; then sed -i \"s/^fps_limit=.*/fps_limit=$1/\" \"$f\"; else echo \"fps_limit=$1\" >> \"$f\"; fi; done; pkill -SIGUSR2 -x mangohud 2>/dev/null; true", "sh", String(n)];
                        fpsProc.running = true;
                        status = n === 0 ? "Unlimited" : "Limited to " + n + " fps";
                    }
                    Process { id: fpsProc }
                    Column {
                        anchors.fill: parent
                        spacing: 10
                        Row {
                            spacing: 6
                            Repeater {
                                model: [30, 60, 90, 120, 0]
                                Rectangle {
                                    required property int modelData
                                    width: fText.implicitWidth + 20
                                    height: 28
                                    radius: 14
                                    color: Theme.color.fill
                                    Text { id: fText; anchors.centerIn: parent; text: parent.modelData === 0 ? "Off" : parent.modelData; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                                    MouseArea { anchors.fill: parent; onClicked: fpsW.setLimit(parent.modelData) }
                                }
                            }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: fpsW.status || "Applies to games running with MangoHud."
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }
            }
        }
    }
}
