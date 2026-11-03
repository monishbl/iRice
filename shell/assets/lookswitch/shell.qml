import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// The screen shown while the shell restarts into the other look (Glass or
// Material; switch-look.sh): your blurred wallpaper on every display, the
// iRice mark morphing through Material's shapes, and the look's name. A
// config of its own (run from a copy, with qs -p, like the update screen), so
// it stays up while the shell restarts. It reads ../status: "run", then
// "done" (fade out and quit). Its own namespace (not "irice:"), so the
// compositor doesn't blur it.
ShellRoot {
    id: root

    property string to: "material"
    property var backdrops: ({}) // screen name -> blurred wallpaper (file)
    property bool dark: true
    FileView {
        path: Quickshell.shellDir + "/../look.json"
        blockLoading: true
        printErrors: false
        onLoaded: {
            try {
                const j = JSON.parse(text());
                root.to = j.to === "glass" ? "glass" : "material";
                root.backdrops = j.backdrops ?? {};
                root.dark = j.dark !== false;
            } catch (e) {}
        }
    }
    FontLoader { source: Quickshell.shellDir + "/Phosphor-Fill.ttf" }
    FontLoader { source: Quickshell.shellDir + "/GoogleSansFlex.ttf" }

    property real shown: 0
    property bool leaving: false
    Component.onCompleted: shown = 1
    Behavior on shown { NumberAnimation { duration: root.leaving ? 450 : 320; easing.type: root.leaving ? Easing.InCubic : Easing.OutCubic } }
    onShownChanged: if (leaving && shown <= 0.001) Qt.quit()
    FileView {
        path: Quickshell.shellDir + "/../status"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: if (text().trim() === "done") root.leaving = true
    }
    onLeavingChanged: if (leaving) shown = 0
    // Never up for good (the script ends it; this is in case it can't)
    Timer {
        running: true
        interval: 45000
        onTriggered: root.leaving = true
    }

    // The mark's shapes, a new one every 420 ms: Glass's squircle, then
    // Material's cookies and flowers ([kind, n, a, b, rotation°, scale], as
    // material/common/MaterialShapes.js)
    readonly property var shapes: [[2, 0, 0.3, 1, 0, 0.92], [0, 4, 0.16, 1, 45, 1], [0, 4, 0.34, 3, 45, 1], [0, 8, 0.11, 0.55, 0, 1], [0, 8, 0.16, 1.6, 0, 1], [0, 9, 0.08, 1, -90, 1]]
    property int step: 0
    Timer {
        running: !root.leaving
        repeat: true
        interval: 420
        onTriggered: root.step = (root.step + 1) % root.shapes.length
    }
    readonly property color ink: dark ? "white" : "#1C1C1E"

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "iricelookswitch"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive // (keys don't reach apps meanwhile)
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"

            Item {
                anchors.fill: parent
                opacity: root.shown

                Rectangle {
                    anchors.fill: parent
                    color: root.dark ? "#101012" : "#F2F2F5"
                }
                Image {
                    anchors.fill: parent
                    source: root.backdrops[win.modelData.name] ? "file://" + root.backdrops[win.modelData.name] : ""
                    fillMode: Image.PreserveAspectCrop
                }
                Rectangle {
                    anchors.fill: parent
                    color: root.dark ? Qt.rgba(0.06, 0.06, 0.08, 0.55) : Qt.rgba(0.97, 0.97, 0.99, 0.5)
                }

                Item {
                    id: mark
                    anchors.centerIn: parent
                    anchors.verticalCenterOffset: -24
                    width: 168
                    height: 168
                    ShaderEffect {
                        id: shape
                        anchors.fill: parent
                        property int from: 0
                        property int toStep: 0
                        property real morph: 1
                        readonly property var a: root.shapes[from]
                        readonly property var b: root.shapes[toStep]
                        property size size: Qt.size(width, height)
                        property vector4d shapeA: Qt.vector4d(a[0], a[1], a[2], a[3])
                        property vector4d shapeB: Qt.vector4d(b[0], b[1], b[2], b[3])
                        property point turnA: Qt.point(a[4] * Math.PI / 180, a[5])
                        property point turnB: Qt.point(b[4] * Math.PI / 180, b[5])
                        property real turn: 0
                        property real borderWidth: 1.5
                        property color borderColor: Qt.rgba(1, 1, 1, root.dark ? 0.35 : 0.9)
                        property real picture: 0
                        property color fillColor: root.dark ? Qt.rgba(1, 1, 1, 0.2) : Qt.rgba(1, 1, 1, 0.8)
                        property var pictureTex: ShaderEffectSource { sourceItem: null; visible: false }
                        fragmentShader: Quickshell.shellDir + "/materialshape.frag.qsb"
                        Connections {
                            target: root
                            function onStepChanged() {
                                shape.from = shape.toStep;
                                shape.toStep = root.step;
                                shape.morph = 0;
                                morphIn.restart();
                            }
                        }
                        SpringAnimation {
                            id: morphIn
                            target: shape
                            property: "morph"
                            to: 1
                            spring: 3.2
                            damping: 0.27
                            epsilon: 0.002
                        }
                        RotationAnimator on rotation {
                            from: 0
                            to: root.to === "material" ? 360 : -360
                            duration: 3600
                            loops: Animation.Infinite
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        text: "\ue210" // (Phosphor's filled drop)
                        color: root.ink
                        font { family: "Phosphor-Fill"; pixelSize: 64 }
                    }
                }
                Column {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: mark.bottom
                    anchors.topMargin: 40
                    spacing: 8
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.to === "material" ? "Material" : "Glass"
                        color: root.ink
                        font { family: root.to === "material" ? "iRice Sans Flex" : "Inter"; pixelSize: 40; weight: Font.DemiBold }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.to === "material" ? "Material · experimental" : "Glass"
                        color: Qt.alpha(root.ink, 0.65)
                        font { family: root.to === "material" ? "iRice Sans Flex" : "Inter"; pixelSize: 16 }
                    }
                }
            }
        }
    }
}
