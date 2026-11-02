import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common

// The first login: choose a look, Glass or Material, from two live previews
// on your wallpaper. Picking one switches to it (the switching screen runs
// and this comes back restyled); Continue (or Enter, or Esc) finishes. Shown
// once, on a fresh install; an existing install counts as welcomed.
Scope {
    id: root

    readonly property bool wanted: UiState.loaded && UiState.initialized && UiState.data.welcome === "" && Wallpaper.started
    function finish() {
        UiState.data.welcome = "done";
    }
    function choose(look) {
        if (Config.options.appearance.look !== look)
            Config.options.appearance.look = look;
    }

    LazyLoader {
        active: root.wanted && !LookSwitch.running

        PanelWindow {
            id: win
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:welcome"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"

            readonly property bool mat: Theme.material
            readonly property string look: Config.options.appearance.look

            Item {
                id: page
                anchors.fill: parent
                focus: true
                opacity: 0
                Component.onCompleted: opacity = 1
                Behavior on opacity { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Left)
                        root.choose("glass");
                    else if (event.key === Qt.Key_Right)
                        root.choose("material");
                    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Escape)
                        root.finish();
                    else
                        return;
                    event.accepted = true;
                }

                Image {
                    anchors.fill: parent
                    source: Wallpaper.backdropUrl(win.screen?.name ?? "") || Wallpaper.source
                    fillMode: Image.PreserveAspectCrop
                }
                Rectangle {
                    anchors.fill: parent
                    color: win.mat ? Qt.alpha(Theme.m3.surface, 0.78) : Theme.dark ? Qt.rgba(0.06, 0.06, 0.08, 0.55) : Qt.rgba(0.97, 0.97, 0.99, 0.5)
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 40

                    Column {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 12
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "Welcome to iRice"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: 40; weight: win.mat ? Font.Bold : Font.DemiBold }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "Choose a look. Everything works the same in both, and you can change it any time in Settings → Appearance."
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: 16 }
                        }
                    }

                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 40
                        Repeater {
                            model: [
                                { look: "glass", title: "Glass", text: "Liquid Glass: translucent, light-bending surfaces and continuous corners" },
                                { look: "material", title: "Material", text: "Material 3 Expressive: colour from your wallpaper, bold shapes, springy motion" }
                            ]
                            Column {
                                required property var modelData
                                spacing: 16
                                width: 420
                                LookPreview {
                                    width: 420
                                    height: 262
                                    look: parent.modelData.look
                                    selected: win.look === parent.modelData.look
                                    onChosen: root.choose(parent.modelData.look)
                                }
                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: parent.modelData.title
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: 20; weight: win.look === parent.modelData.look ? Font.Bold : Font.Medium }
                                }
                                Text {
                                    width: parent.width
                                    horizontalAlignment: Text.AlignHCenter
                                    wrapMode: Text.WordWrap
                                    text: parent.modelData.text
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: 14 }
                                }
                            }
                        }
                    }

                    // Continue: Material's filled pill, or a macOS default button
                    Rectangle {
                        id: go
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: label.implicitWidth + (win.mat ? 48 : 40)
                        height: win.mat ? 48 : 36
                        radius: win.mat ? height / 2 : 10
                        color: Theme.accent
                        scale: goMouse.pressed ? 0.96 : 1
                        Behavior on scale { NumberAnimation { duration: 100 } }
                        Rectangle {
                            anchors.fill: parent
                            radius: parent.radius
                            color: Theme.color.onAccent
                            opacity: goMouse.pressed ? 0.12 : goMouse.containsMouse ? 0.08 : 0
                        }
                        Text {
                            id: label
                            anchors.centerIn: parent
                            text: "Continue"
                            color: Theme.color.onAccent
                            font { family: Theme.font.family; pixelSize: win.mat ? 16 : 14; weight: Font.Medium }
                        }
                        MouseArea {
                            id: goMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.finish()
                        }
                        Accessible.role: Accessible.Button
                        Accessible.name: "Continue"
                    }
                }
            }
        }
    }
}
