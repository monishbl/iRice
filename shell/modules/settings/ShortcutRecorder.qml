import QtQuick
import qs.common
import qs.services

// Picks the keys for a shortcut. Hyprland takes most combinations before
// they reach Settings, so the full combo can't simply be pressed: the
// modifiers are toggles (Super, Ctrl, Alt, Shift) and the box catches one
// plain key. "Edit as text" takes anything Hyprland understands, such as
// SUPER + mouse:272 or SUPER + code:86.
//
// load(keys) to start; `result` is the keys in Hyprland's syntax ("$MOD" is
// Super); `edited` fires on every change.
Column {
    id: root

    property var mods: []
    property string key: ""
    property bool textMode: false
    readonly property string result: textMode ? fromText(textField.text) : Keymap.formatKeys(mods, key)
    readonly property bool complete: textMode ? Keymap.parseKeys(result).key !== "" : key !== ""
    signal edited

    spacing: 12

    function load(keys) {
        const p = Keymap.parseKeys(keys);
        mods = p.mods;
        key = p.key;
        textField.text = toText(keys);
        // Keys the box can't catch (mouse buttons, key codes, two keys) open as text
        textMode = p.key !== "" && !catchable(p.key);
    }
    function catchable(k) {
        return k === "$MODKEY" || /^([A-Z0-9]|F\d{1,2}|Left|Right|Up|Down|Space|Return|Tab|Escape|BackSpace|Delete|Home|End|Page_Up|Page_Down|Print|Insert|Period|Comma|Slash|Semicolon|Apostrophe|Minus|Equal|bracketleft|bracketright|grave|backslash)$/.test(k);
    }
    function toText(keys) {
        return String(keys ?? "").replace(/\$MODKEY/g, "SUPER_L").replace(/\$MOD/g, "SUPER");
    }
    function fromText(t) {
        return String(t).split("+").map(s => s.trim()).filter(s => s !== "").map(s => {
            const u = s.toUpperCase();
            if (u === "SUPER" || u === "WIN" || u === "META" || u === "MOD4")
                return "$MOD";
            if (u === "SUPER_L")
                return "$MODKEY";
            if (u === "CTRL" || u === "CONTROL")
                return "CTRL";
            if (u === "ALT" || u === "SHIFT")
                return u;
            return s;
        }).join(" + ");
    }
    function toggle(m) {
        mods = mods.includes(m) ? mods.filter(x => x !== m) : mods.concat([m]);
        textField.text = toText(Keymap.formatKeys(mods, key));
        edited();
    }
    function setMod(m, on) {
        if (on !== mods.includes(m))
            toggle(m);
    }

    // Qt key → Hyprland key name
    readonly property var named: {
        const m = {};
        m[Qt.Key_Left] = "Left"; m[Qt.Key_Right] = "Right"; m[Qt.Key_Up] = "Up"; m[Qt.Key_Down] = "Down";
        m[Qt.Key_Space] = "Space"; m[Qt.Key_Return] = "Return"; m[Qt.Key_Enter] = "Return"; m[Qt.Key_Tab] = "Tab"; m[Qt.Key_Backtab] = "Tab";
        m[Qt.Key_Escape] = "Escape"; m[Qt.Key_Backspace] = "BackSpace"; m[Qt.Key_Delete] = "Delete"; m[Qt.Key_Insert] = "Insert";
        m[Qt.Key_Home] = "Home"; m[Qt.Key_End] = "End"; m[Qt.Key_PageUp] = "Page_Up"; m[Qt.Key_PageDown] = "Page_Down"; m[Qt.Key_Print] = "Print";
        m[Qt.Key_Period] = "Period"; m[Qt.Key_Comma] = "Comma"; m[Qt.Key_Slash] = "Slash"; m[Qt.Key_Semicolon] = "Semicolon";
        m[Qt.Key_Apostrophe] = "Apostrophe"; m[Qt.Key_Minus] = "Minus"; m[Qt.Key_Equal] = "Equal"; m[Qt.Key_BracketLeft] = "bracketleft";
        m[Qt.Key_BracketRight] = "bracketright"; m[Qt.Key_QuoteLeft] = "grave"; m[Qt.Key_Backslash] = "backslash";
        return m;
    }
    // Shifted symbols (US layout) → the key underneath, with Shift on
    readonly property var shifted: {
        const m = {};
        m[Qt.Key_Question] = "Slash"; m[Qt.Key_Colon] = "Semicolon"; m[Qt.Key_QuoteDbl] = "Apostrophe"; m[Qt.Key_Less] = "Comma";
        m[Qt.Key_Greater] = "Period"; m[Qt.Key_Underscore] = "Minus"; m[Qt.Key_Plus] = "Equal"; m[Qt.Key_BraceLeft] = "bracketleft";
        m[Qt.Key_BraceRight] = "bracketright"; m[Qt.Key_AsciiTilde] = "grave"; m[Qt.Key_Bar] = "backslash";
        m[Qt.Key_Exclam] = "1"; m[Qt.Key_At] = "2"; m[Qt.Key_NumberSign] = "3"; m[Qt.Key_Dollar] = "4"; m[Qt.Key_Percent] = "5";
        m[Qt.Key_AsciiCircum] = "6"; m[Qt.Key_Ampersand] = "7"; m[Qt.Key_Asterisk] = "8"; m[Qt.Key_ParenLeft] = "9"; m[Qt.Key_ParenRight] = "0";
        return m;
    }
    function keyName(event) {
        const k = event.key;
        if (k >= Qt.Key_A && k <= Qt.Key_Z)
            return String.fromCharCode(k);
        if (k >= Qt.Key_0 && k <= Qt.Key_9)
            return String.fromCharCode(k);
        if (k >= Qt.Key_F1 && k <= Qt.Key_F24)
            return "F" + (k - Qt.Key_F1 + 1);
        if (named[k] !== undefined)
            return named[k];
        if (shifted[k] !== undefined) {
            setMod("SHIFT", true);
            return shifted[k];
        }
        // Anything else by its key code (what Hyprland calls code:N)
        return event.nativeScanCode > 0 ? "code:" + event.nativeScanCode : "";
    }

    component ModChip: Squircle {
        id: chip
        required property string mod
        readonly property bool checked: root.mods.includes(mod)
        width: Math.max(56, chipLabel.implicitWidth + 24)
        height: 32
        radius: Theme.radius.control
        activeFocusOnTab: true
        color: checked ? Theme.accent : chipMouse.pressed ? Theme.color.fillPressed : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
        scale: chipMouse.pressed ? 0.97 : 1
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
        Accessible.role: Accessible.CheckBox
        Accessible.name: chipLabel.text
        Accessible.checked: checked
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) {
                root.toggle(chip.mod);
                event.accepted = true;
            }
        }
        Squircle {
            anchors { fill: parent; margins: -3 }
            visible: chip.activeFocus
            radius: chip.radius + 3
            color: "transparent"
            border.width: 2
            border.color: Theme.accent
        }
        Text {
            id: chipLabel
            anchors.centerIn: parent
            // The name in words, and the symbol with Mac symbols on
            text: KeyNames.style === "mac" ? KeyNames.name(chip.mod) + " " + ({ SUPER: "Command", CTRL: "Control", ALT: "Option", SHIFT: "Shift" })[chip.mod] : KeyNames.name(chip.mod === "SUPER" ? Keymap.modKey : chip.mod)
            color: chip.checked ? Theme.color.onAccent : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
        }
        MouseArea {
            id: chipMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.toggle(chip.mod)
        }
    }

    // Modifiers + the key
    Flow {
        visible: !root.textMode
        width: parent.width
        spacing: 8
        ModChip { mod: "SUPER" }
        ModChip { mod: "CTRL" }
        ModChip { mod: "ALT" }
        ModChip { mod: "SHIFT" }
        Text {
            height: 32
            verticalAlignment: Text.AlignVCenter
            text: "+"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.title }
        }
        Squircle {
            id: catcher
            width: 168
            height: 32
            radius: Theme.radius.control
            color: activeFocus ? Theme.accentAlpha(0.12) : Theme.color.fill
            border.width: activeFocus ? 2 : 1
            border.color: activeFocus ? Theme.accent : Theme.color.separator
            activeFocusOnTab: true
            Accessible.role: Accessible.EditableText
            Accessible.name: "Key: " + (root.key ? Keymap.keyLabel(root.key) : "none") + ". Press a key to change it."
            Text {
                anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                text: catcher.activeFocus ? "Press a key…" : root.key === "$MODKEY" ? "Tap " + KeyNames.name(Keymap.modKey) + " alone" : root.key ? Keymap.keyLabel(root.key) : "Click, then press a key"
                color: catcher.activeFocus || !root.key ? Theme.color.secondary : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: root.key && !catcher.activeFocus ? Font.DemiBold : Font.Normal }
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: catcher.forceActiveFocus()
            }
            // One key, then focus moves on (so the box never traps the keyboard)
            Keys.onPressed: event => {
                event.accepted = true;
                const k = event.key;
                if (k === Qt.Key_Shift || k === Qt.Key_Control || k === Qt.Key_Alt || k === Qt.Key_Meta || k === Qt.Key_Super_L || k === Qt.Key_Super_R || k === Qt.Key_AltGr)
                    return;
                const name = root.keyName(event);
                if (name === "")
                    return;
                if (event.modifiers & Qt.ShiftModifier)
                    root.setMod("SHIFT", true);
                if (event.modifiers & Qt.ControlModifier)
                    root.setMod("CTRL", true);
                if (event.modifiers & Qt.AltModifier)
                    root.setMod("ALT", true);
                if (event.modifiers & Qt.MetaModifier)
                    root.setMod("SUPER", true);
                root.key = name;
                textField.text = root.toText(Keymap.formatKeys(root.mods, root.key));
                root.edited();
                textToggle.forceActiveFocus();
            }
        }
    }

    // As text
    Column {
        visible: root.textMode
        width: parent.width
        spacing: 4
        TextField {
            id: textField
            width: Math.min(parent.width, 420)
            implicitHeight: 32
            placeholder: "SUPER + SHIFT + S"
            commitOnBlur: false
            onCommitted: root.edited()
            Connections {
                target: textField
                function onTextChanged() { if (root.textMode) root.edited(); }
            }
        }
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: "Hyprland key names joined with +, for example SUPER + mouse:272 (left click), SUPER + code:86 (a key by its code) or XF86AudioPlay."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    SettingsButton {
        id: textToggle
        text: root.textMode ? "Use the key picker" : "Edit as text"
        onClicked: {
            if (root.textMode) {
                // Back to the picker with what was typed
                const p = Keymap.parseKeys(root.fromText(textField.text));
                root.mods = p.mods;
                root.key = root.catchable(p.key) ? p.key : "";
                root.textMode = false;
            } else {
                textField.text = root.toText(Keymap.formatKeys(root.mods, root.key));
                root.textMode = true;
                textField.focusInput();
            }
            root.edited();
        }
    }
}
