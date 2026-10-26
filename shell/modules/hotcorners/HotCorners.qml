import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.services

// Hot corners: push the pointer into a screen corner and hold it there for
// `delay` ms to run that corner's action (Settings → Desktop → Hot corners).
// Each corner with an action is a 3px transparent layer surface at the very
// corner; corners set to "none" have no window at all. Nothing polls: the
// compositor tells the surface when the pointer enters, and the only timer
// runs while the pointer rests in a corner.
// No action while the corner's monitor shows a fullscreen window, while the
// screen is locked, or while dragging (a button held elsewhere keeps the
// pointer grabbed by that surface, so the corner never sees it enter; a
// press inside the corner cancels).
Scope {
    id: root

    readonly property var cfg: Config.options.hotCorners
    readonly property int delay: cfg?.delay ?? 200
    readonly property var corners: ["topLeft", "topRight", "bottomLeft", "bottomRight"]
    readonly property var defaults: ({ topLeft: "overview", topRight: "none", bottomLeft: "none", bottomRight: "none" })

    function actionFor(corner) {
        return cfg?.[corner] ?? defaults[corner];
    }

    // Values for Config.options.hotCorners.<corner>
    function run(action, screen) {
        switch (action) {
        case "overview":
            Panels.toggle("overview", screen);
            break;
        case "missionControl":
            Panels.toggle("missionControl", screen);
            break;
        case "desktop":
            Desktop.showDesktop();
            break;
        case "notificationCenter":
            Panels.toggle("notificationCenter", screen);
            break;
        case "launchpad":
            Panels.toggle("launchpad", screen);
            break;
        case "lock":
            System.power("lock");
            break;
        case "standby":
            // The lock screen turns into StandBy on its own while charging
            System.power("lock");
            break;
        case "quickNote":
            Panels.show("quickNote", screen);
            break;
        }
    }

    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData

            Variants {
                model: root.corners.filter(c => root.actionFor(c) !== "none")

                PanelWindow {
                    id: zone
                    required property string modelData
                    readonly property string corner: modelData
                    readonly property bool atTop: corner.startsWith("top")
                    readonly property bool atLeft: corner.endsWith("Left")

                    screen: perScreen.modelData
                    anchors { top: zone.atTop; bottom: !zone.atTop; left: zone.atLeft; right: !zone.atLeft }
                    implicitWidth: 3
                    implicitHeight: 3
                    color: "transparent"
                    exclusionMode: ExclusionMode.Ignore
                    WlrLayershell.namespace: "irice:hotcorner"
                    WlrLayershell.layer: WlrLayer.Overlay
                    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

                    function allowed() {
                        if (!hover.hovered || hover.point.pressedButtons !== Qt.NoButton || System.locked)
                            return false;
                        return !(Hyprland.monitorFor(screen)?.activeWorkspace?.hasFullscreen ?? false);
                    }

                    HoverHandler {
                        id: hover
                        onHoveredChanged: {
                            if (hovered && point.pressedButtons === Qt.NoButton)
                                dwell.restart();
                            else
                                dwell.stop();
                        }
                    }
                    // Clicks in the corner do nothing, and cancel a pending action
                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.AllButtons
                        onPressed: dwell.stop()
                    }
                    // Fires once per visit: leave the corner to use it again
                    Timer {
                        id: dwell
                        interval: root.delay
                        onTriggered: if (zone.allowed()) root.run(root.actionFor(zone.corner), zone.screen)
                    }
                }
            }
        }
    }
}
