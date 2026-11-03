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
            id: glass
            anchors.fill: parent
            radius: Theme.material ? 24 : 20
            bevel: 16
            shadowMargin: win.m
        }
        // Material: a card in surfaceContainerHigh
        Rectangle {
            visible: Theme.material
            anchors.fill: glass.shape
            radius: glass.radius
            color: Theme.m3.surfaceContainerHigh ?? "transparent"
        }

        Column {
            id: content
            x: win.m + (Theme.material ? 20 : 14)
            y: win.m + (Theme.material ? 16 : 14)
            width: win.cardWidth - (Theme.material ? 40 : 28)
            spacing: 8
            focus: true
            Keys.onEscapePressed: ScreenTools.translation = null

            Item {
                width: parent.width
                height: Theme.material ? 32 : 22
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Translation"
                    // (Material: a label in primary, sentence case)
                    color: Theme.material ? Theme.m3.primary : Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.small : Theme.font.caption; weight: Font.DemiBold; capitalization: Theme.material ? Font.MixedCase : Font.AllUppercase }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 4
                    Repeater {
                        model: [{ icon: "copy", act: () => Quickshell.execDetached(["wl-copy", win.t?.text ?? ""]) }, { icon: "x", act: () => ScreenTools.translation = null }]
                        Rectangle {
                            required property var modelData
                            width: Theme.material ? 32 : 22
                            height: width
                            transform: pressFx1.effects
                            ElasticControl { id: pressFx1; pressed: m2.pressed }
                            radius: Theme.material && m2.pressed ? 8 : width / 2
                            // (Material: a tonal icon button, its state layer over it)
                            color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, m2.pressed ? 0.1 : m2.containsMouse ? 0.08 : 0))
                                : m2.containsMouse ? Theme.color.fillHover : "transparent"
                            Icon { anchors.centerIn: parent; name: parent.modelData.icon; bold: !Theme.material; size: Theme.material ? 16 : 12; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.secondary }
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
