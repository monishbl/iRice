import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.services

// Stage Manager's strip: the waiting stages of the space on this screen, as
// window thumbnails on the left edge. Click one to bring it forward; Shift+click
// to add its windows to the stage in front. It reserves its width, so tiled
// windows never cover it.
Variants {
    model: StageManager.enabled && Config.options.stageManager.strip ? Quickshell.screens : []

    PanelWindow {
        id: win
        required property var modelData
        screen: modelData

        readonly property var monitor: Hyprland.monitorFor(screen)
        readonly property int wsId: monitor?.activeWorkspace?.id ?? -1
        readonly property var waiting: StageManager.waiting(wsId).slice(0, 6)

        WlrLayershell.namespace: "irice:stagestrip"
        WlrLayershell.layer: WlrLayer.Bottom
        anchors { top: true; bottom: true; left: true }
        implicitWidth: 176
        exclusiveZone: implicitWidth
        color: "transparent"

        Column {
            anchors.verticalCenter: parent.verticalCenter
            x: 20
            spacing: 24

            Repeater {
                model: win.waiting

                Item {
                    id: stage
                    required property var modelData
                    readonly property var tops: modelData.wins.slice(-3).map(a => StageManager.toplevel(a)).filter(t => !!t)
                    width: 140
                    height: 104
                    scale: mouse.pressed ? 0.96 : mouse.containsMouse ? 1.04 : 1
                    Behavior on scale { ScaleAnimator { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }

                    // Up to three windows, stacked back to front
                    Repeater {
                        model: stage.tops
                        Rectangle {
                            required property var modelData
                            required property int index
                            readonly property int depth: stage.tops.length - 1 - index
                            x: 12 - depth * 6
                            y: depth * 6
                            width: 120
                            height: 80
                            radius: 8
                            color: Theme.dark ? "#1A1A1D" : "#FFFFFF"
                            border.width: 1
                            border.color: Theme.dark ? "#36363C" : "#D4D4D8"
                            opacity: 1 - depth * 0.2
                            clip: true
                            ScreencopyView {
                                id: stageShot
                                anchors { fill: parent; margins: 1 }
                                captureSource: parent.modelData.wayland ?? null
                                live: false
                                constraintSize: Qt.size(240, 160)
                            }
                            // Live previews (Settings → Overview): the strip's
                            // windows keep updating too, up to 30 times a second
                            LiveCapture {
                                target: stageShot
                                live: win.visible && Config.options.overview.previews === "live"
                            }
                        }
                    }

                    AppIcon {
                        x: 0
                        y: 64
                        size: 36
                        source: Apps.iconFor(StageManager.classOf(stage.tops[stage.tops.length - 1]))
                    }

                    MouseArea {
                        id: mouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: e => (e.modifiers & Qt.ShiftModifier) ? StageManager.merge(win.wsId, stage.modelData.id) : StageManager.show(win.wsId, stage.modelData.id)
                    }
                }
            }
        }
    }
}
