import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Result of a screen translation: a glass card next to the translated area.
// Copy the translation or dismiss with Esc / the close button.
LazyLoader {
    active: ScreenTools.translation !== null

    GlassWindow {
        id: win
        readonly property var t: ScreenTools.translation
        readonly property int m: 24
        readonly property int cardWidth: Math.max(280, Math.min(460, t?.w ?? 300))

        name: "translation"
        screen: t?.screen ?? Quickshell.screens[0]
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
        anchors { top: true; left: true }
        margins.left: Math.max(0, Math.min((t?.x ?? 0) - m, (screen?.width ?? 1920) - implicitWidth))
        margins.top: Math.max(30, Math.min((t?.y ?? 0) + (t?.h ?? 0) + 8 - m, (screen?.height ?? 1080) - implicitHeight))
        implicitWidth: cardWidth + 2 * m
        implicitHeight: content.implicitHeight + 28 + 2 * m

        Glass {
            anchors.fill: parent
            radius: 20
            bevel: 16
            shadowMargin: win.m
        }

        Column {
            id: content
            x: win.m + 14
            y: win.m + 14
            width: win.cardWidth - 28
            spacing: 8
            focus: true
            Keys.onEscapePressed: ScreenTools.translation = null

            Item {
                width: parent.width
                height: 22
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Translation"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold; capitalization: Font.AllUppercase }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 4
                    Repeater {
                        model: [{ icon: "copy", act: () => Quickshell.execDetached(["wl-copy", win.t?.text ?? ""]) }, { icon: "x", act: () => ScreenTools.translation = null }]
                        Rectangle {
                            required property var modelData
                            width: 22
                            height: 22
                            radius: 11
                            color: m2.containsMouse ? Theme.color.fillHover : "transparent"
                            Icon { anchors.centerIn: parent; name: parent.modelData.icon; bold: true; size: 12; color: Theme.color.secondary }
                            MouseArea { id: m2; anchors.fill: parent; hoverEnabled: true; onClicked: parent.modelData.act() }
                        }
                    }
                }
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: win.t?.busy ? "Translating…" : (win.t?.error || win.t?.text || "")
                color: win.t?.error ? Theme.color.danger : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.title }
            }
            Text {
                width: parent.width
                visible: (win.t?.source ?? "") !== ""
                wrapMode: Text.Wrap
                maximumLineCount: 4
                elide: Text.ElideRight
                text: win.t?.source ?? ""
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
    }
}
