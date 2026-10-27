import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.modules.settings
import qs.services

// Window Rules: pick an app, then choose how its windows open. Three views:
// the list of apps with rules, the app picker, and one app's rules.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property string view: "list" // list | pick | edit
    property string editClass: ""
    property string editName: ""
    readonly property var rule: view === "edit" ? WindowRules.draft(editClass, editName) : null

    // Undo for the last removed rule
    property var removed: null // { rule, index }
    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: page.removed = null
    }

    function edit(cls, name) {
        Hyprland.refreshToplevels(); // fresh sizes for "Use current size"
        editClass = cls;
        editName = name;
        view = "edit";
    }
    function change(patch) {
        WindowRules.set(editClass, editName, patch);
    }
    function removeRule() {
        const r = WindowRules.ruleFor(editClass);
        view = "list";
        if (!r)
            return;
        removed = { rule: r, index: WindowRules.remove(r.class) };
        undoTimer.restart();
    }

    // ── Shared pieces ────────────────────────────────────────────────────
    // Ghost text button; `danger` for removing
    component TextButton: Rectangle {
        id: tb
        property string text
        property string icon
        property bool danger: false
        property bool primary: false
        signal clicked
        width: tbRow.implicitWidth + 24
        height: 28
        radius: Theme.radius.control
        color: primary ? (tbMouse.pressed ? Qt.darker(Theme.accent, 1.2) : tbMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : tbMouse.pressed ? Theme.color.fillPressed : tbMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
        scale: tbMouse.pressed ? 0.97 : 1
        Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        // (not a Row: in Settings pages Row is the settings row)
        Item {
            id: tbRow
            anchors.centerIn: parent
            implicitWidth: (tbIcon.visible ? tbIcon.implicitWidth + 6 : 0) + tbLabel.implicitWidth
            implicitHeight: tbLabel.implicitHeight
            width: implicitWidth
            height: implicitHeight
            Icon {
                id: tbIcon
                visible: tb.icon !== ""
                anchors.verticalCenter: parent.verticalCenter
                name: tb.icon
                bold: true
                size: 12
                color: tbLabel.color
            }
            Text {
                id: tbLabel
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                text: tb.text
                color: tb.primary ? Theme.color.onAccent : tb.danger ? Theme.color.danger : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: tb.primary ? Font.DemiBold : Font.Medium }
            }
        }
        MouseArea {
            id: tbMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: tb.clicked()
        }
    }

    // An app row: icon, name, detail, chevron; the whole row is clickable
    component AppRow: Rectangle {
        id: ar
        property string name
        property string detail
        property string iconSource
        property bool chevron: true
        signal clicked
        width: parent?.width ?? 0
        height: 48
        color: arMouse.pressed ? Theme.color.fillPressed : arMouse.containsMouse ? Theme.color.fill : "transparent"
        AppIcon {
            id: arIcon
            x: 14
            anchors.verticalCenter: parent.verticalCenter
            size: 28
            source: ar.iconSource
        }
        Column {
            anchors { left: arIcon.right; leftMargin: 12; right: arChevron.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: ar.name
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
            Text {
                width: parent.width
                visible: ar.detail !== ""
                text: ar.detail
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        Icon {
            id: arChevron
            anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
            visible: ar.chevron
            name: "caret-right"
            bold: true
            size: 12
            color: Theme.color.tertiary
        }
        MouseArea {
            id: arMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: ar.clicked()
        }
    }

    // New windows: tiled or floating, for every app (a rule below can make
    // an app the opposite; dialogs and other child windows always float)
    Group {
        visible: page.view === "list" && WindowRules.available
        width: parent.width
        footer: WindowRules.floating ? "Every app opens as a free window; dialogs too. To tile an app anyway, add a rule for it and turn on Tile." : "Apps share the screen side by side; dialogs and pickers still float. To float an app anyway, add a rule for it and turn on Float."
        Row {
            label: "New windows open"
            detail: "Changing it also re-arranges the windows on the spaces you can see"
            Segmented {
                width: 220
                readonly property var values: ["tiled", "floating"]
                options: ["Tiled", "Floating"]
                current: Math.max(0, values.indexOf(Config.options.windowRules.defaultLayout))
                onSelected: i => Config.options.windowRules.defaultLayout = values[i]
            }
        }
    }

    // ── Errors ───────────────────────────────────────────────────────────
    Group {
        visible: !WindowRules.available || WindowRules.error !== ""
        width: parent.width
        Row {
            label: !WindowRules.available ? "Window rules aren't set up" : "Rules weren't saved"
            detail: !WindowRules.available ? "The windowRules section is missing from the iRice config. Update iRice, then open this page again." : WindowRules.error
        }
    }

    // ── List ─────────────────────────────────────────────────────────────
    Group {
        visible: page.removed !== null && page.view === "list"
        width: parent.width
        Row {
            label: "Removed the rule for " + (page.removed?.rule.name ?? "")
            TextButton {
                text: "Undo"
                icon: "arrow-counter-clockwise"
                onClicked: {
                    WindowRules.restore(page.removed.rule, page.removed.index);
                    page.removed = null;
                    undoTimer.stop();
                }
            }
        }
    }

    // Empty state
    Item {
        visible: page.view === "list" && WindowRules.available && WindowRules.rules.length === 0
        width: parent.width
        height: empty.implicitHeight + 64
        Column {
            id: empty
            anchors.centerIn: parent
            width: Math.min(parent.width, 360)
            spacing: 8
            Icon {
                anchors.horizontalCenter: parent.horizontalCenter
                name: "app-window"
                size: 40
                color: Theme.color.tertiary
            }
            Item { width: 1; height: 4 }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "No window rules"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: "Choose an app to set how its windows open: floating, on a space, opaque, maximized or at a set size."
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
            Item { width: 1; height: 24 }
            TextButton {
                anchors.horizontalCenter: parent.horizontalCenter
                primary: true
                icon: "plus"
                text: "Choose app"
                onClicked: page.view = "pick"
            }
        }
    }

    Item {
        visible: page.view === "list" && WindowRules.rules.length > 0
        width: parent.width
        height: 28
        Text {
            anchors { left: parent.left; leftMargin: 4; right: addButton.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            text: "Apps open the way you set here. Rules match the window class."
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        TextButton {
            id: addButton
            anchors.right: parent.right
            primary: true
            icon: "plus"
            text: "Add app"
            onClicked: page.view = "pick"
        }
    }

    Group {
        visible: page.view === "list" && WindowRules.rules.length > 0
        width: parent.width
        footer: "Changes apply to windows opened after the change."
        Repeater {
            model: WindowRules.rules
            AppRow {
                required property var modelData
                name: modelData.name
                detail: WindowRules.summary(modelData)
                iconSource: Apps.iconFor(modelData.class)
                onClicked: page.edit(modelData.class, modelData.name)
            }
        }
    }

    // ── Picker ───────────────────────────────────────────────────────────
    Item {
        id: picker
        visible: page.view === "pick"
        width: parent.width
        height: 28

        readonly property string query: search.text.trim().toLowerCase()
        function matches(a) {
            return query === "" || a.name.toLowerCase().includes(query) || a.cls.toLowerCase().includes(query);
        }
        // Apps with windows open now, one per class
        readonly property var openApps: {
            if (page.view !== "pick")
                return [];
            Apps.entries; // re-evaluate once entries load
            const seen = new Set(), out = [];
            for (const t of ToplevelManager.toplevels.values) {
                const cls = t.appId;
                if (!cls || cls === "org.quickshell" || seen.has(cls))
                    continue;
                seen.add(cls);
                const e = Apps.entryFor(cls);
                out.push({ cls: cls, name: e?.name ?? cls, icon: Apps.iconFor(e ?? cls) });
            }
            return out.sort((a, b) => a.name.localeCompare(b.name));
        }
        readonly property var installedApps: {
            if (page.view !== "pick")
                return [];
            const open = new Set(openApps.map(a => a.cls)), seen = new Set(), out = [];
            for (const e of Apps.entries) {
                const cls = e.startupClass || e.id;
                if (e.noDisplay || !cls || open.has(cls) || seen.has(cls))
                    continue;
                seen.add(cls);
                out.push({ cls: cls, name: e.name, icon: Apps.iconFor(e) });
            }
            return out.sort((a, b) => a.name.localeCompare(b.name));
        }
        readonly property var openShown: openApps.filter(a => matches(a))
        readonly property var installedMatches: installedApps.filter(a => matches(a))
        readonly property int limit: 80
        readonly property var installedShown: installedMatches.slice(0, limit)
        readonly property bool noResults: query !== "" && openShown.length === 0 && installedMatches.length === 0

        function pick(a) {
            page.edit(a.cls, a.name);
        }
        // Enter takes the first result, or the text as a window class
        function pickFirst() {
            const first = openShown[0] ?? installedShown[0];
            if (first)
                pick(first);
            else if (search.text.trim() !== "")
                pick({ cls: search.text.trim(), name: search.text.trim() });
        }

        onVisibleChanged: if (visible) {
            search.text = "";
            Qt.callLater(search.focusInput);
        }

        IconButton {
            id: pickBack
            icon: "caret-left"
            iconColor: Theme.color.label
            tooltip: "Back to all rules"
            onClicked: page.view = "list"
        }
        TextField {
            id: search
            anchors { left: pickBack.right; leftMargin: 8; right: parent.right }
            placeholder: "Search apps, or type a window class"
            commitOnBlur: false
            onCommitted: picker.pickFirst()
        }
    }

    Text {
        visible: page.view === "pick" && Apps.entries.length === 0 && picker.openApps.length === 0
        leftPadding: 4
        text: "Loading apps…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }

    Group {
        visible: page.view === "pick" && picker.noResults
        width: parent.width
        Row {
            label: "No apps match “" + search.text.trim() + "”"
            detail: "Check the spelling, or press Enter to use “" + search.text.trim() + "” as the window class. Open the app and it appears under Open now."
        }
    }

    Group {
        visible: page.view === "pick" && picker.openShown.length > 0
        width: parent.width
        title: "Open now"
        Repeater {
            model: picker.openShown
            AppRow {
                required property var modelData
                name: modelData.name
                detail: WindowRules.ruleFor(modelData.cls) ? WindowRules.summary(WindowRules.ruleFor(modelData.cls)) : modelData.cls
                iconSource: modelData.icon
                onClicked: picker.pick(modelData)
            }
        }
    }

    Group {
        visible: page.view === "pick" && picker.installedShown.length > 0
        width: parent.width
        title: "Applications"
        footer: picker.installedMatches.length > picker.limit ? "Showing " + picker.limit + " of " + picker.installedMatches.length + ". Type to narrow the list." : ""
        Repeater {
            model: picker.installedShown
            AppRow {
                required property var modelData
                name: modelData.name
                detail: WindowRules.ruleFor(modelData.cls) ? WindowRules.summary(WindowRules.ruleFor(modelData.cls)) : modelData.cls
                iconSource: modelData.icon
                onClicked: picker.pick(modelData)
            }
        }
    }

    // ── One app's rules ──────────────────────────────────────────────────
    Item {
        visible: page.view === "edit"
        width: parent.width
        height: 44

        IconButton {
            id: editBack
            anchors.verticalCenter: parent.verticalCenter
            icon: "caret-left"
            iconColor: Theme.color.label
            tooltip: "Back to all rules"
            onClicked: page.view = "list"
        }
        AppIcon {
            id: editIcon
            anchors { left: editBack.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
            size: 36
            source: page.view === "edit" ? Apps.iconFor(page.editClass) : ""
        }
        Column {
            anchors { left: editIcon.right; leftMargin: 12; right: removeButton.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: page.editName
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                text: "Window class " + page.editClass
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        TextButton {
            id: removeButton
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            visible: WindowRules.ruleFor(page.editClass) !== null
            danger: true
            icon: "trash"
            text: "Remove rule"
            onClicked: page.removeRule()
        }
    }

    Group {
        visible: page.view === "edit"
        width: parent.width
        title: "When it opens"
        // The opposite of what new windows do (Settings: New windows open)
        Row {
            visible: !WindowRules.floating
            label: "Float"
            detail: "Opens as a free window instead of tiling"
            Switch {
                checked: page.rule?.float || page.rule?.pin || false
                onToggled: page.change(page.rule.pin ? { float: false, pin: false } : { float: !page.rule.float, tile: false })
            }
        }
        Row {
            visible: WindowRules.floating
            label: "Tile"
            detail: "Tiles with the others, though new windows float"
            Switch {
                checked: page.rule?.tile || false
                onToggled: page.change({ tile: !page.rule.tile, float: false, pin: false })
            }
        }
        Row {
            label: "Open on space"
            detail: page.rule?.workspace > 0 ? "Opens on space " + page.rule.workspace + " and takes you there" : "Opens on the space you're on"
            Item {
                width: wsRow.width
                height: 28
                Flow { // (Row here is the settings row)
                    id: wsRow
                    width: 28 + 4 + 64 + 4 + 28
                    spacing: 4
                    IconButton {
                        icon: "minus"
                        iconColor: page.rule?.workspace > 0 ? Theme.color.label : Theme.color.tertiary
                        tooltip: "Previous space"
                        onClicked: if (page.rule.workspace > 0) page.change({ workspace: page.rule.workspace - 1 })
                    }
                    Text {
                        width: 64
                        height: 28
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        text: page.rule?.workspace > 0 ? "Space " + page.rule.workspace : "Any"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
                    }
                    IconButton {
                        icon: "plus"
                        iconColor: page.rule?.workspace < 10 ? Theme.color.label : Theme.color.tertiary
                        tooltip: "Next space"
                        onClicked: if (page.rule.workspace < 10) page.change({ workspace: page.rule.workspace + 1 })
                    }
                }
            }
        }
        Row {
            label: "Start as"
            Segmented {
                width: 280
                readonly property var values: ["", "maximize", "fullscreen"]
                options: ["Normal", "Maximized", "Fullscreen"]
                current: Math.max(0, values.indexOf(page.rule?.start ?? ""))
                onSelected: i => page.change({ start: values[i] })
            }
        }
        Row {
            label: "Show on all spaces"
            detail: "Floats and stays in view when you switch spaces"
            Switch {
                checked: page.rule?.pin ?? false
                onToggled: page.change({ pin: !page.rule.pin })
            }
        }
    }

    Group {
        visible: page.view === "edit"
        width: parent.width
        title: "Look"
        Row {
            label: "Always opaque"
            detail: "Ignores window transparency (Appearance), like video and games"
            Switch {
                checked: page.rule?.opaque ?? false
                onToggled: page.change({ opaque: !page.rule.opaque })
            }
        }
    }

    Group {
        id: sizeGroup
        visible: page.view === "edit"
        width: parent.width
        title: "Size"
        footer: "Used when the window floats. Leave both empty for the app's own size."

        property string sizeError: ""
        // The app's open window, for "Use current size"
        readonly property var openWindow: page.view === "edit" ? Hyprland.toplevels.values.find(t => t.lastIpcObject?.class === page.editClass && t.lastIpcObject?.size) ?? null : null

        function commit() {
            const ws = widthField.text.trim(), hs = heightField.text.trim();
            if (ws === "" && hs === "") {
                sizeError = "";
                page.change({ width: 0, height: 0 });
                return;
            }
            if (ws === "" || hs === "") {
                sizeError = "Enter both a width and a height, or leave both empty.";
                return;
            }
            const w = Number(ws), h = Number(hs);
            if (!Number.isInteger(w) || !Number.isInteger(h) || w < 200 || h < 150 || w > 7680 || h > 4320) {
                sizeError = "Enter whole numbers: a width from 200 to 7680 and a height from 150 to 4320 pixels.";
                return;
            }
            sizeError = "";
            page.change({ width: w, height: h });
        }
        function fill() {
            widthField.text = page.rule?.width > 0 ? String(page.rule.width) : "";
            heightField.text = page.rule?.height > 0 ? String(page.rule.height) : "";
            sizeError = "";
        }
        Connections {
            target: page
            function onViewChanged() { if (page.view === "edit") sizeGroup.fill(); }
            function onEditClassChanged() { if (page.view === "edit") sizeGroup.fill(); }
        }

        Row {
            label: "Default size"
            detail: sizeGroup.sizeError === "" ? "Width × height, in pixels" : ""
            Flow { // (Row here is the settings row)
                spacing: 8
                width: widthField.width + times.width + heightField.width + 16
                TextField {
                    id: widthField
                    width: 72
                    placeholder: "Width"
                    onCommitted: sizeGroup.commit()
                }
                Text {
                    id: times
                    height: 28
                    verticalAlignment: Text.AlignVCenter
                    text: "×"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
                TextField {
                    id: heightField
                    width: 72
                    placeholder: "Height"
                    onCommitted: sizeGroup.commit()
                }
            }
        }
        Row {
            visible: sizeGroup.openWindow !== null
            label: "Current window"
            detail: sizeGroup.openWindow ? sizeGroup.openWindow.lastIpcObject.size[0] + " × " + sizeGroup.openWindow.lastIpcObject.size[1] : ""
            TextButton {
                text: "Use current size"
                onClicked: {
                    const s = sizeGroup.openWindow.lastIpcObject.size;
                    widthField.text = String(Math.max(200, s[0]));
                    heightField.text = String(Math.max(150, s[1]));
                    sizeGroup.commit();
                }
            }
        }
    }

    Text {
        visible: page.view === "edit" && sizeGroup.sizeError !== ""
        width: parent.width
        leftPadding: 4
        wrapMode: Text.Wrap
        text: sizeGroup.sizeError
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }

    Text {
        visible: page.view === "edit"
        width: parent.width
        leftPadding: 4
        wrapMode: Text.Wrap
        text: "Changes apply to " + page.editName + " windows opened from now on."
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
}
