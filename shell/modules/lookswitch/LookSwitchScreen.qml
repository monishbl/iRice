import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// The switching screen between the looks (common/LookSwitch drives it): the
// blurred wallpaper covers every screen while the iRice mark morphs from a
// glass squircle through Material's shapes into a cookie (or back) and its
// colours move to the new look's; the shell restyles out of sight while the
// mark keeps turning; then the new look is revealed. The fades and the turn
// are Animators (render thread), so they stay smooth while the shell is busy
// restyling.
Scope {
    id: root

    readonly property bool toMaterial: LookSwitch.to === "material"
    function mix(c1, c2, t) {
        const a = Qt.color(c1), b = Qt.color(c2);
        return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t);
    }
    readonly property var scheme: LookSwitch.materialScheme
    // Each look's colours: Glass's frosted white, Material's containers
    readonly property color glassVeil: Theme.dark ? Qt.rgba(0.08, 0.08, 0.1, 0.5) : Qt.rgba(0.96, 0.96, 0.98, 0.5)
    readonly property color materialVeil: Qt.alpha(scheme.surface ?? "#141218", 0.72)
    readonly property color glassMark: Theme.dark ? Qt.rgba(1, 1, 1, 0.2) : Qt.rgba(1, 1, 1, 0.8)
    readonly property color materialMark: scheme.primaryContainer ?? "#4F378B"
    readonly property color glassInk: Theme.dark ? "white" : "#1C1C1E"
    readonly property color materialInk: scheme.onPrimaryContainer ?? "#EADDFF"

    // Glass's squircle, through the shapes, to Material's cookie
    readonly property var path: ["square", "cookie4", "clover4", "sunny", "flower", "cookie9"]

    Variants {
        model: LookSwitch.running ? Quickshell.screens : []

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:lookswitch"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"

            readonly property bool out: LookSwitch.phase === "out"
            // How far along the morph is: 0 the look being left, 1 the new one
            property real arrive: 0
            NumberAnimation on arrive {
                from: 0
                to: 1
                duration: 820
                easing.type: Easing.InOutCubic
            }
            readonly property real m: root.toMaterial ? arrive : 1 - arrive // how Material it looks
            readonly property int step: Math.round(arrive * (root.path.length - 1))

            Item {
                id: cover
                anchors.fill: parent
                opacity: 0
                OpacityAnimator on opacity {
                    id: coverIn
                    from: 0
                    to: 1
                    duration: 380
                    easing.type: Easing.OutCubic
                }
                OpacityAnimator {
                    target: cover
                    running: win.out
                    from: 1
                    to: 0
                    duration: 560
                    easing.type: Easing.InOutCubic
                }

                Image {
                    anchors.fill: parent
                    source: Wallpaper.backdropUrl(win.modelData.name) || Wallpaper.source
                    fillMode: Image.PreserveAspectCrop
                    cache: true
                }
                Rectangle {
                    anchors.fill: parent
                    color: root.mix(root.glassVeil, root.materialVeil, win.m)
                }

                // The mark: shape, colour and drop; it keeps turning while
                // the shell restyles
                Item {
                    id: markBox
                    anchors.centerIn: parent
                    anchors.verticalCenterOffset: -24
                    width: 168
                    height: 168
                    scale: 0.7
                    ScaleAnimator on scale {
                        from: 0.7
                        to: 1
                        duration: 700
                        easing.type: Easing.OutBack
                        easing.overshoot: 1.4
                    }
                    ScaleAnimator {
                        target: markBox
                        running: win.out
                        from: 1
                        to: 1.18
                        duration: 520
                        easing.type: Easing.InCubic
                    }
                    MaterialShape {
                        anchors.fill: parent
                        RotationAnimator on rotation {
                            from: 0
                            to: root.toMaterial ? 360 : -360
                            duration: 3600
                            loops: Animation.Infinite
                        }
                        shape: root.toMaterial ? root.path[win.step] : root.path[root.path.length - 1 - win.step]
                        color: root.mix(root.glassMark, root.materialMark, win.m)
                        borderWidth: 1.5 * (1 - win.m)
                        borderColor: Qt.rgba(1, 1, 1, Theme.dark ? 0.35 : 0.9)
                    }
                    Icon {
                        anchors.centerIn: parent
                        name: "drop"
                        fill: true
                        size: 64
                        color: root.mix(root.glassInk, root.materialInk, win.m)
                    }
                }
                Column {
                    id: words
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: markBox.bottom
                    anchors.topMargin: 40
                    spacing: 8
                    opacity: 0
                    OpacityAnimator on opacity {
                        from: 0
                        to: 1
                        duration: 500
                        easing.type: Easing.OutCubic
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.toMaterial ? "Material" : "Glass"
                        color: root.mix(root.glassInk, root.scheme.onSurface ?? "#E6E0E9", win.m)
                        font { family: root.toMaterial ? "Google Sans Flex" : "Inter"; pixelSize: 40; weight: root.toMaterial ? Font.Bold : Font.DemiBold }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.toMaterial ? "Material 3 Expressive" : "Liquid Glass"
                        color: root.mix(Qt.alpha(root.glassInk, 0.65), root.scheme.onSurfaceVariant ?? "#CAC4D0", win.m)
                        font { family: root.toMaterial ? "Google Sans Flex" : "Inter"; pixelSize: 16 }
                    }
                }
            }
        }
    }
}
