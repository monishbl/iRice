import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Keyboard Shortcuts: every bind, grouped like the data, searchable by name
// or by keys ("super s", "ctrl print"). Change, turn off or reset a default;
// add shortcuts of your own for apps, default apps, iRice actions or
// commands. Two views: the list, and the form for one shortcut.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property string view: "list" // list | form
    // The form: editId "" = a new shortcut of your own
    property string editId: ""
    readonly property var editing: editId !== "" ? Keymap.bind(editId) : null
    readonly property bool editCustom: editId === "" || (editing?.custom ?? false)

    // Undo for the last reset, removal or save: { text, snap }
    property var undo: null
    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: page.undo = null
    }
    function offerUndo(text, snap) {
        undo = { text: text, snap: snap };
        undoTimer.restart();
    }

    readonly property string query: search.text.trim()
    readonly property var shownGroups: {
        const q = query;
        return Keymap.groups.map(g => ({ name: g.name, binds: g.binds.filter(b => Keymap.matches(b, q)) })).filter(g => g.binds.length > 0 || (g.name === "Your shortcuts" && q === ""));
    }
    readonly property bool noResults: query !== "" && shownGroups.every(g => g.binds.length === 0)

    function openForm(id) {
        editId = id;
        view = "form";
        form.load();
    }
    function openMenu(anchor, b) {
        const items = [];
        if (b.custom)
            items.push({ text: "Edit", icon: "pencil-simple", action: () => page.openForm(b.id) });
        else
            items.push({ text: "Change shortcut", icon: "keyboard", action: () => page.openForm(b.id) });
        items.push({
            text: b.disabled ? "Turn on" : "Turn off", icon: "power",
            action: () => {
                const snap = Keymap.snapshot();
                Keymap.disable(b.id, !b.disabled);
                page.offerUndo((b.disabled ? "Turned on " : "Turned off ") + b.label, snap);
            }
        });
        if (b.custom)
            items.push({
                text: "Remove", icon: "trash", danger: true,
                action: () => {
                    const snap = Keymap.snapshot();
                    Keymap.removeShortcut(b.id);
                    page.offerUndo("Removed " + b.label, snap);
                }
            });
        else
            items.push({
                text: "Reset to default", icon: "arrow-counter-clockwise", enabled: b.changed,
                action: () => {
                    const snap = Keymap.snapshot();
                    Keymap.reset(b.id);
                    page.offerUndo(b.label + " is back to " + Keymap.keyText(b.defaultKeys), snap);
                }
            });
        pageMenu.open(anchor, items);
    }

    // ── A shortcut in the list ───────────────────────────────────────────
    component BindRow: Rectangle {
        id: br
        required property var bind
        width: parent?.width ?? 0
        height: Math.max(44, brText.implicitHeight + 20)
        color: brMouse.pressed ? Theme.color.fillPressed : brMouse.containsMouse || br.activeFocus ? Theme.color.fill : "transparent"
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: bind.label + ", " + bind.keyText + (bind.disabled ? ", off" : "")
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space || event.key === Qt.Key_Menu) {
                page.openMenu(brMenu, br.bind);
                event.accepted = true;
            }
        }
        Rectangle {
            anchors { fill: parent; margins: 2 }
            visible: br.activeFocus
            radius: Theme.radius.item
            color: "transparent"
            border.width: 2
            border.color: Theme.accent
        }
        Column {
            id: brText
            anchors { left: parent.left; leftMargin: 14; right: brKeys.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: br.bind.label
                elide: Text.ElideRight
                color: br.bind.disabled ? Theme.color.secondary : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
            Text {
                width: parent.width
                readonly property string what: br.bind.disabled ? "Off" : br.bind.keysChanged ? "Changed · was " + Keymap.keyText(br.bind.defaultKeys) : br.bind.custom ? br.bind.actionText : ""
                visible: what !== ""
                text: what
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        KeyCombo {
            id: brKeys
            anchors { right: brMenu.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            keys: br.bind.keyList
            opacity: br.bind.disabled ? 0.4 : 1
        }
        MouseArea {
            id: brMouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: page.openMenu(brMenu, br.bind)
        }
        IconButton {
            id: brMenu
            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            icon: "dots-three"
            iconColor: brMouse.containsMouse || pageMenu.anchor === brMenu && pageMenu.shown ? Theme.color.label : Theme.color.tertiary
            tooltip: "Options for " + br.bind.label
            onClicked: page.openMenu(brMenu, br.bind)
        }
    }

    // ── Problems ─────────────────────────────────────────────────────────
    Repeater {
        model: [Keymap.defaultsError, Keymap.userError, Keymap.saveError].filter(e => e !== "")
        Group {
            id: problem
            required property string modelData
            width: page.width
            Row {
                label: "Something's wrong with the shortcuts files"
                detail: problem.modelData
                Icon { name: "warning-circle"; size: 20; color: Theme.color.warning }
            }
        }
    }

    // ── List ─────────────────────────────────────────────────────────────
    Item {
        visible: page.view === "list"
        width: parent.width
        height: 28
        TextField {
            id: search
            anchors { left: parent.left; right: addButton.left; rightMargin: 12 }
            placeholder: "Search by name or keys, like “super s” or “ctrl print”"
            commitOnBlur: false
        }
        SettingsButton {
            id: addButton
            anchors.right: parent.right
            primary: true
            icon: "plus"
            text: "Add shortcut"
            enabled: Keymap.userError === ""
            onClicked: page.openForm("")
        }
    }

    Text {
        visible: page.view === "list" && !Keymap.ready
        leftPadding: 4
        text: "Loading shortcuts…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }

    Group {
        visible: page.undo !== null && page.view === "list"
        width: parent.width
        Row {
            label: page.undo?.text ?? ""
            SettingsButton {
                text: "Undo"
                icon: "arrow-counter-clockwise"
                onClicked: {
                    Keymap.restore(page.undo.snap);
                    page.undo = null;
                    undoTimer.stop();
                }
            }
        }
    }

    Group {
        visible: page.view === "list" && page.noResults
        width: parent.width
        Row {
            label: "No shortcuts match “" + page.query + "”"
            detail: "Try part of a name, like “screenshot”, or keys, like “super shift s”."
            SettingsButton { text: "Clear search"; onClicked: search.text = "" }
        }
    }

    Repeater {
        model: page.view === "list" ? page.shownGroups : []
        Group {
            id: group
            required property var modelData
            width: page.width
            title: modelData.name
            footer: modelData.name === "Your shortcuts" && modelData.binds.length > 0 ? "Your shortcuts win when they share keys with a default." : ""

            // Empty state for your own shortcuts
            Row {
                visible: group.modelData.binds.length === 0
                label: "No shortcuts of your own yet"
                detail: "Add one to open an app, run a command or start an iRice action."
                SettingsButton { text: "Add shortcut"; icon: "plus"; onClicked: page.openForm("") }
            }
            Repeater {
                model: group.modelData.binds
                BindRow {
                    required property var modelData
                    bind: modelData
                }
            }
        }
    }

    ImportExport {
        visible: page.view === "list" && Keymap.ready
        width: parent.width
        sections: ["keybinds", "shortcuts", "gestures"]
    }

    // ── Screenshots ──────────────────────────────────────────────────────
    component EditorPicker: Rectangle {
        id: ep
        property var options: []
        property var off: [] // indexes that can't be chosen
        property int current: 0
        signal selected(int index)
        implicitHeight: 30
        radius: height / 2
        color: Theme.color.toggleOff
        readonly property real segment: (width - 4) / Math.max(1, options.length)
        Rectangle {
            x: 2 + ep.current * ep.segment
            y: 2
            width: ep.segment
            height: ep.height - 4
            radius: height / 2
            color: Theme.dark ? Qt.rgba(1, 1, 1, 0.2) : "white"
            Behavior on x { XAnimator { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
        }
        Repeater {
            model: ep.options
            Item {
                required property string modelData
                required property int index
                readonly property bool usable: !ep.off.includes(index)
                x: 2 + index * ep.segment
                width: ep.segment
                height: ep.height
                Text {
                    anchors.centerIn: parent
                    text: parent.modelData
                    color: !parent.usable ? Theme.color.tertiary : parent.index === ep.current ? Theme.color.label : Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    anchors.fill: parent
                    enabled: parent.usable
                    cursorShape: Qt.PointingHandCursor
                    onClicked: ep.selected(parent.index)
                }
            }
        }
    }

    Group {
        visible: page.view === "list"
        width: parent.width
        title: "Screenshots"
        footer: Tools.has("swappy") ? "" : "Install swappy to use it as the markup editor."
        Row {
            label: "Markup editor"
            EditorPicker {
                width: 200
                options: ["iRice", "Swappy"]
                off: Tools.has("swappy") ? [] : [1]
                current: Config.options.screenshot.editor === "swappy" ? 1 : 0
                onSelected: i => Config.options.screenshot.editor = i === 1 ? "swappy" : "irice"
            }
        }
        Row {
            visible: Config.options.screenshot.editor !== "swappy"
            label: "Show floating thumbnail"
            detail: "Click it to mark up the screenshot; off shows a notification with Edit"
            Switch {
                checked: Config.options.screenshot.thumbnail
                onToggled: Config.options.screenshot.thumbnail = !checked
            }
        }
        Row {
            visible: Config.options.screenshot.editor === "swappy"
            label: "Open area screenshots in Swappy"
            detail: "To crop, draw and add text, then save or copy"
            Switch {
                enabled: Tools.has("swappy")
                checked: Config.options.screenshot.markup
                onToggled: Config.options.screenshot.markup = !checked
            }
        }
    }

    // ── Form ─────────────────────────────────────────────────────────────
    Item {
        id: form
        visible: page.view === "form"
        width: parent.width
        height: 44

        property string name: ""
        property string kind: "shell" // app | role | shell | command
        property string appId: ""
        property string role: "terminal"
        property string shellName: ""
        property string command: ""
        property bool repeating: false
        property bool locked: false
        property string error: ""
        property string keys: ""
        readonly property var conflicts: page.view === "form" && recorder.complete ? Keymap.conflicts(recorder.result, page.editId) : []

        function load() {
            error = "";
            const b = page.editing;
            const a = b?.action ?? {};
            name = b?.label ?? "";
            nameField.text = name;
            appSearch.text = "";
            appId = a.desktop ?? "";
            role = a.app ?? "terminal";
            shellName = a.shell ?? "";
            command = a.exec !== undefined && !a.desktop ? a.exec : "";
            commandField.text = command;
            kind = a.desktop ? "app" : a.app !== undefined ? "role" : a.shell !== undefined ? "shell" : a.exec !== undefined ? "command" : "app";
            repeating = !!b?.opts?.repeating;
            locked = !!b?.opts?.locked;
            recorder.load(b?.keys ?? "");
        }
        function action() {
            if (kind === "app") {
                const e = DesktopEntries.byId(appId);
                return e ? Keymap.appAction(e) : (page.editing?.action?.desktop === appId ? page.editing.action : null);
            }
            if (kind === "role")
                return { app: role };
            if (kind === "shell")
                return shellName ? { shell: shellName } : null;
            return commandField.text.trim() ? { exec: commandField.text.trim() } : null;
        }
        function validate() {
            if (!recorder.complete)
                return "Choose a key: click the box and press one, or use Edit as text.";
            if (!page.editCustom)
                return "";
            if (kind === "app" && !action())
                return "Choose an app from the list.";
            if (kind === "shell" && !shellName)
                return "Choose an iRice action.";
            if (kind === "command" && commandField.text.trim() === "")
                return "Enter a command to run, for example kitty -e btop.";
            return "";
        }
        function save(takeOver) {
            error = validate();
            if (error !== "")
                return;
            if (conflicts.length > 0 && !takeOver) {
                error = "These keys are used by " + conflicts[0].label + ". Choose other keys, or use them anyway.";
                return;
            }
            const snap = Keymap.snapshot();
            const keys = recorder.result;
            const others = takeOver ? conflicts.map(c => c.id) : [];
            for (const o of others)
                Keymap.disable(o, true);
            let label;
            if (!page.editCustom) {
                Keymap.setKeys(page.editId, keys);
                label = page.editing.label;
            } else {
                const a = action();
                label = nameField.text.trim() || Keymap.describe(a);
                const opts = {};
                if (repeating)
                    opts.repeating = true;
                if (locked)
                    opts.locked = true;
                const entry = { keys: keys, label: label, action: a, opts: opts };
                if (page.editId === "")
                    Keymap.addShortcut(entry);
                else
                    Keymap.updateShortcut(page.editId, entry);
            }
            page.view = "list";
            page.offerUndo(label + ": " + Keymap.keyText(keys) + (others.length ? " (turned off " + conflicts.map(c => c.label).join(", ") + ")" : ""), snap);
        }

        IconButton {
            id: formBack
            anchors.verticalCenter: parent.verticalCenter
            icon: "caret-left"
            iconColor: Theme.color.label
            tooltip: "Back to all shortcuts"
            onClicked: page.view = "list"
        }
        Column {
            anchors { left: formBack.right; leftMargin: 8; right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: page.editId === "" ? "New shortcut" : page.editCustom ? "Edit shortcut" : "Change shortcut"
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                visible: text !== ""
                text: page.editCustom ? "" : (page.editing?.label ?? "") + " · " + (page.editing?.group ?? "")
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
    }

    Group {
        visible: page.view === "form" && page.editCustom
        width: parent.width
        title: "Name"
        Row {
            label: "Shown in Settings and the cheatsheet"
            TextField {
                id: nameField
                width: 260
                placeholder: "Open Firefox"
                commitOnBlur: false
            }
        }
    }

    Group {
        visible: page.view === "form"
        width: parent.width
        title: "Keys"
        footer: "Most combinations go to the window manager before Settings sees them, so turn the modifiers on here and press only the last key."
        Item {
            width: parent.width
            height: recorder.implicitHeight + 28
            ShortcutRecorder {
                id: recorder
                x: 14
                y: 14
                width: parent.width - 28
                onEdited: form.error = ""
            }
        }
        Row {
            visible: page.view === "form" && !page.editCustom && Keymap.canonical(recorder.result) !== Keymap.canonical(page.editing?.defaultKeys ?? "")
            label: "Default"
            detail: Keymap.keyText(page.editing?.defaultKeys ?? "")
            SettingsButton {
                text: "Use default keys"
                onClicked: recorder.load(page.editing.defaultKeys)
            }
        }
    }

    Group {
        visible: page.view === "form" && form.conflicts.length > 0
        width: parent.width
        Row {
            label: "Used by " + form.conflicts.map(c => c.label).join(", ")
            detail: "Only one shortcut can use " + Keymap.keyText(recorder.result) + "."
            SettingsButton {
                text: form.conflicts.length === 1 ? "Use anyway (turns that one off)" : "Use anyway (turns those off)"
                onClicked: form.save(true)
            }
        }
    }

    // What a shortcut of your own does
    Group {
        visible: page.view === "form" && page.editCustom
        width: parent.width
        title: "Action"
        Item {
            width: parent.width
            height: 54
            Segmented {
                id: kindPicker
                anchors { left: parent.left; right: parent.right; margins: 14; verticalCenter: parent.verticalCenter }
                readonly property var kinds: ["app", "role", "shell", "command"]
                options: ["App", "Default app", "iRice action", "Command"]
                current: Math.max(0, kinds.indexOf(form.kind))
                onSelected: i => {
                    form.kind = kinds[i];
                    form.error = "";
                }
            }
        }
        Row {
            visible: form.kind === "role"
            label: "Opens your"
            detail: "The app chosen for it in Settings → Default apps"
            Choice {
                menu: pageMenu
                width: 180
                options: Keymap.roles.map(r => ({ value: r.name, label: r.label }))
                value: form.role
                onChosen: v => form.role = v
            }
        }
        Row {
            visible: form.kind === "shell"
            label: "Action"
            Choice {
                menu: pageMenu
                width: 260
                placeholder: "Choose an action"
                options: Keymap.shellActions.map(a => ({ value: a.name, label: a.label }))
                value: form.shellName
                onChosen: v => {
                    form.shellName = v;
                    form.error = "";
                }
            }
        }
        Row {
            visible: form.kind === "command"
            label: "Command"
            detail: "Runs with sh, for example kitty -e btop"
            TextField {
                id: commandField
                width: 300
                placeholder: "kitty -e btop"
                commitOnBlur: false
            }
        }
        Item {
            visible: form.kind === "app"
            width: parent.width
            height: 48
            TextField {
                id: appSearch
                anchors { left: parent.left; right: parent.right; margins: 14; verticalCenter: parent.verticalCenter }
                placeholder: "Search apps"
                commitOnBlur: false
            }
        }
        Repeater {
            id: appList
            readonly property string q: appSearch.text.trim().toLowerCase()
            readonly property var all: {
                if (page.view !== "form" || form.kind !== "app")
                    return [];
                return DesktopEntries.applications.values.filter(e => !e.noDisplay).slice().sort((a, b) => a.name.localeCompare(b.name));
            }
            readonly property var matched: all.filter(e => q === "" || e.name.toLowerCase().includes(q) || (e.genericName ?? "").toLowerCase().includes(q) || e.id.toLowerCase().includes(q))
            // The chosen app first, then up to 30
            model: {
                const chosen = matched.filter(e => e.id === form.appId);
                return chosen.concat(matched.filter(e => e.id !== form.appId).slice(0, 30));
            }
            Rectangle {
                id: appRow
                required property var modelData
                readonly property bool chosen: modelData.id === form.appId
                width: parent?.width ?? 0
                height: 40
                color: chosen ? Theme.accentAlpha(0.14) : appMouse.pressed ? Theme.color.fillPressed : appMouse.containsMouse ? Theme.color.fill : "transparent"
                activeFocusOnTab: true
                function choose() {
                    form.appId = modelData.id;
                    form.error = "";
                    if (nameField.text.trim() === "")
                        nameField.text = "Open " + modelData.name;
                }
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) {
                        appRow.choose();
                        event.accepted = true;
                    }
                }
                Rectangle {
                    anchors { fill: parent; margins: 2 }
                    visible: appRow.activeFocus
                    radius: Theme.radius.item
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.accent
                }
                AppIcon {
                    id: appIcon
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    size: 24
                    source: Apps.iconFor(appRow.modelData)
                }
                Text {
                    anchors { left: appIcon.right; leftMargin: 10; right: appCheck.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    text: appRow.modelData.name
                    elide: Text.ElideRight
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: appRow.chosen ? Font.DemiBold : Font.Normal }
                }
                Icon {
                    id: appCheck
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    visible: appRow.chosen
                    name: "check"
                    bold: true
                    size: 14
                    color: Theme.accent
                }
                MouseArea {
                    id: appMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: appRow.choose()
                }
            }
        }
        Row {
            visible: form.kind === "app" && appList.all.length === 0
            label: "Loading apps…"
        }
        Row {
            visible: form.kind === "app" && appList.all.length > 0 && appList.matched.length === 0
            label: "No apps match “" + appSearch.text.trim() + "”"
            detail: "Check the spelling, or choose Command to run any program."
        }
        Row {
            visible: form.kind === "app" && appList.matched.length > 31
            label: "Showing 30 of " + appList.matched.length + ". Type to narrow the list."
        }
    }

    Group {
        visible: page.view === "form" && page.editCustom
        width: parent.width
        title: "Options"
        Row {
            label: "Repeat while held"
            detail: "Like volume keys"
            Switch {
                checked: form.repeating
                onToggled: form.repeating = !checked
            }
        }
        Row {
            label: "Works on the lock screen"
            Switch {
                checked: form.locked
                onToggled: form.locked = !checked
            }
        }
    }

    Text {
        visible: page.view === "form" && form.error !== ""
        width: parent.width
        leftPadding: 4
        wrapMode: Text.Wrap
        text: form.error
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Item {
        visible: page.view === "form"
        width: parent.width
        height: 28
        Flow {
            anchors.right: parent.right
            spacing: 8
            width: cancelForm.implicitWidth + saveForm.implicitWidth + 8
            SettingsButton { id: cancelForm; text: "Cancel"; onClicked: page.view = "list" }
            SettingsButton {
                id: saveForm
                primary: true
                text: page.editId === "" ? "Add shortcut" : "Save shortcut"
                enabled: Keymap.userError === ""
                onClicked: form.save(false)
            }
        }
    }

    ActionMenu {
        id: pageMenu
    }
}
