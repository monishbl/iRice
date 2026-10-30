import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass

// On-screen keyboard. Never takes focus; keys go to the focused app via
// wtype. Modifiers are sticky (tap Shift, then a key). Windows make room.
Scope {
    id: root

    readonly property bool open: Panels.open === "keyboard" || keepOpen
    property bool keepOpen: false
    property var mods: ({ shift: false, ctrl: false, alt: false, logo: false })
    property bool caps: false

    // [label, shiftLabel, keysym or text, width units]
    readonly property var rows: [
        [["`", "~", "grave"], ["1", "!"], ["2", "@"], ["3", "#"], ["4", "$"], ["5", "%"], ["6", "^"], ["7", "&"], ["8", "*"], ["9", "("], ["0", ")"], ["-", "_", "minus"], ["=", "+", "equal"], ["⌫", "⌫", "BackSpace", 1.6]],
        [["Tab", "Tab", "Tab", 1.5], ["q"], ["w"], ["e"], ["r"], ["t"], ["y"], ["u"], ["i"], ["o"], ["p"], ["[", "{", "bracketleft"], ["]", "}", "bracketright"], ["\\", "|", "backslash", 1.1]],
        [["Caps", "Caps", "@caps", 1.8], ["a"], ["s"], ["d"], ["f"], ["g"], ["h"], ["j"], ["k"], ["l"], [";", ":", "semicolon"], ["'", "\"", "apostrophe"], ["Return", "Return", "Return", 2.0]],
        [["Shift", "Shift", "@shift", 2.3], ["z"], ["x"], ["c"], ["v"], ["b"], ["n"], ["m"], [",", "<", "comma"], [".", ">", "period"], ["/", "?", "slash"], ["Shift", "Shift", "@shift", 2.5]],
        [["Ctrl", "Ctrl", "@ctrl", 1.4], ["Super", "Super", "@logo", 1.4], ["Alt", "Alt", "@alt", 1.4], ["", "", "space", 6.2], ["←", "←", "Left"], ["↓", "↓", "Down"], ["↑", "↑", "Up"], ["→", "→", "Right"], ["Esc", "Esc", "Escape", 1.4]]
    ]

    function press(key) {
        const sym = key[2] ?? key[0];
        if (sym.startsWith("@")) {
            const m = sym.slice(1);
            if (m === "caps") {
                caps = !caps;
                return;
            }
            const next = Object.assign({}, mods);
            next[m] = !next[m];
            mods = next;
            return;
        }
        const active = Object.keys(mods).filter(m => mods[m]);
        const args = ["wtype"];
        for (const m of active)
            args.push("-M", m);
        // Plain characters: let shift/caps pick the case; named keys use -k
        const upper = (mods.shift !== caps) && sym.length === 1 && /[a-z]/.test(sym);
        if (sym.length === 1 && !key[2])
            args.push("-k", upper ? sym.toUpperCase() : sym);
        else
            args.push("-k", sym);
        for (const m of active.slice().reverse())
            args.push("-m", m);
        Quickshell.execDetached(args);
        if (active.length > 0)
            mods = { shift: false, ctrl: false, alt: false, logo: false };
    }

    LazyLoader {
        active: root.open

        GlassWindow {
            id: win
            name: "keyboard"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            readonly property int m: 20
            readonly property real unit: Math.min(58, ((screen?.width ?? 1920) - 120) / 15.6)
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Normal
            exclusiveZone: height - m
            anchors.bottom: true
            margins.bottom: 8 - m
            implicitWidth: unit * 15.6 + 24 + 2 * m
            implicitHeight: unit * 5 + 4 * 6 + 24 + 2 * m

            Glass {
                anchors.fill: parent
                radius: 26
                bevel: 20
                shadowMargin: win.m
            }

            Column {
                anchors.centerIn: parent
                spacing: 6
                Repeater {
                    model: root.rows
                    Row {
                        id: kbRow
                        required property var modelData
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 6
                        Repeater {
                            model: kbRow.modelData
                            Squircle {
                                id: key
                                required property var modelData
                                readonly property string sym: modelData[2] ?? modelData[0]
                                readonly property bool isMod: sym.startsWith("@")
                                readonly property bool latched: isMod && (sym === "@caps" ? root.caps : root.mods[sym.slice(1)])
                                readonly property bool shifted: root.mods.shift !== root.caps
                                width: win.unit * (modelData[3] ?? 1) - 6 + (modelData[3] ? 0 : 0)
                                height: win.unit - 6
                                radius: 10
                                color: latched ? Theme.accent : keyMouse.pressed ? Theme.color.fillPressed : keyMouse.containsMouse ? Theme.color.fillHover : (isMod || (modelData[3] ?? 1) > 1 ? Theme.color.toggleOff : Theme.color.fill)
                                scale: keyMouse.pressed ? 0.95 : 1
                                Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }

                                Text {
                                    anchors.centerIn: parent
                                    text: {
                                        const d = key.modelData;
                                        if (d.length === 1)
                                            return key.shifted ? d[0].toUpperCase() : d[0];
                                        return root.mods.shift ? d[1] : d[0];
                                    }
                                    color: key.latched ? Theme.color.onAccent : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: key.modelData[0].length > 1 ? Theme.font.small : Theme.font.title; weight: Font.Medium }
                                }
                                MouseArea {
                                    id: keyMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: root.press(key.modelData)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
