import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

// Login Items: the apps that start when you log in (XDG autostart:
// ~/.config/autostart and /etc/xdg/autostart), as assets/scripts/
// xdg-autostart.py sees them. A switch each (a system one is turned off with
// a copy of it in your folder, the XDG way), Add app… from the installed
// apps, Remove for ones you added, Start now. Every change shows a toast
// with Undo.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../../../assets/scripts/xdg-autostart.py").toString().replace("file://", ""))

    // [{ file, name, comment, icon, exec, origin, overridden, enabled, status,
    //    reason, skippedWhenOn, canStart, removable, added }]
    property var allItems: []
    property bool loaded: false
    property string loadError: ""
    readonly property bool startAtLogin: Config.options.loginItems.startAtLogin
    // Skipped by iRice whatever the switch says: their own group, greyed
    function skipped(it) {
        return it.status === "skipped" || it.skippedWhenOn !== "";
    }
    readonly property var active: allItems.filter(it => !skipped(it))
    readonly property var ignored: allItems.filter(it => skipped(it))
    readonly property bool empty: loaded && loadError === "" && allItems.length === 0

    function iconFor(it) {
        return Apps.iconFor(it.icon ? { icon: it.icon } : it.file.replace(/\.desktop$/, ""));
    }
    function statusText(it) {
        const from = it.origin === "user" ? "From you" : "System";
        if (skipped(it))
            return from + " · Skipped: " + (it.skippedWhenOn || it.reason);
        if (!it.enabled)
            return from + " · Turned off";
        return from + " · " + (page.startAtLogin ? "Starts when you log in" : "Paused while login items are off");
    }

    // ── The script: one call at a time, queued ──────────────────────────
    function refresh() {
        listProc.running = true;
    }
    property var queue: [] // [{ args, done(result) }]
    function call(args, done) {
        queue = queue.concat([{ args: args, done: done }]);
        next();
    }
    function next() {
        if (actProc.running || queue.length === 0)
            return;
        const job = queue[0];
        queue = queue.slice(1);
        actProc.job = job;
        actProc.command = ["python3", "-I", page.script].concat(job.args);
        actProc.running = true;
    }
    // Keeps the same object for an item that didn't change (its row stays)
    function merge(fresh) {
        const old = {};
        for (const it of allItems)
            old[it.file] = it;
        return fresh.map(it => {
            const o = old[it.file];
            return o && JSON.stringify(o) === JSON.stringify(it) ? o : it;
        });
    }
    // Shown at once, before the script has written it
    function patch(file, changes) {
        allItems = allItems.map(it => it.file === file ? Object.assign({}, it, changes) : it);
    }

    Process {
        id: listProc
        command: ["python3", "-I", page.script, "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const out = JSON.parse(text);
                    page.allItems = page.merge(out.items);
                    page.loadError = "";
                } catch (e) {
                    page.loadError = "The list came back unreadable. Try again; if it keeps happening, report a bug from Settings → Report a Bug.";
                }
                page.loaded = true;
            }
        }
        onExited: code => {
            if (code !== 0 && !page.loaded) {
                page.loadError = "Python 3 is needed to read them. Install python, then try again.";
                page.loaded = true;
            }
        }
    }
    Process {
        id: actProc
        property var job: null
        stdout: StdioCollector {
            onStreamFinished: {
                let out;
                try {
                    out = JSON.parse(text);
                } catch (e) {
                    out = { ok: false, error: "It didn't work; try again." };
                }
                const job = actProc.job;
                actProc.job = null;
                if (job && job.done)
                    job.done(out);
                page.refresh();
                Qt.callLater(page.next);
            }
        }
    }
    Component.onCompleted: refresh()

    // ── Changes, each with Undo ─────────────────────────────────────────
    function setOn(it, on, quiet) {
        patch(it.file, { enabled: on, status: on ? "start" : "off" });
        call(["set", it.file, on ? "on" : "off"], out => {
            if (!out.ok)
                toast.show("Couldn't turn " + (on ? "on " : "off ") + it.name + ". " + out.error, null);
            else if (!quiet)
                toast.show((on ? "Turned on " : "Turned off ") + it.name, () => page.setOn(it, !on, true));
        });
    }
    function addApp(entry) {
        picker.text = "";
        page.picking = false;
        call(["add", entry.id], out => {
            if (!out.ok)
                toast.show("Couldn't add " + entry.name + ". " + out.error, null);
            else if (out.existed)
                toast.show(entry.name + " was already a login item; it's on now", null);
            else
                toast.show("Added " + entry.name, () => page.call(["remove", out.file], () => {}));
        });
    }
    function removeItem(it) {
        allItems = allItems.filter(x => x.file !== it.file);
        call(["remove", it.file], out => {
            if (!out.ok)
                toast.show("Couldn't remove " + it.name + ". " + out.error, null);
            else
                toast.show("Removed " + it.name, () => page.call(["restore", it.file], r => {
                    if (!r.ok)
                        toast.show("Couldn't bring back " + it.name + ". " + r.error, null);
                }));
        });
    }
    function startNow(it) {
        call(["start", it.file], out => toast.show(out.ok ? "Started " + it.name : "Couldn't start " + it.name + ". " + out.error, null));
    }
    function openMenu(anchor, it) {
        const entries = [{ text: "Start now", icon: "play", enabled: it.canStart, action: () => page.startNow(it) }];
        if (it.removable)
            entries.push({ text: "Remove", icon: "trash", danger: true, action: () => page.removeItem(it) });
        pageMenu.open(anchor, entries);
    }

    // ── A login item ────────────────────────────────────────────────────
    component ItemRow: QQ.Rectangle {
        id: ir
        required property var modelData
        readonly property bool dim: page.skipped(modelData)
        width: parent?.width ?? 0
        height: 52
        color: "transparent"
        AppIcon {
            id: irIcon
            x: 14
            anchors.verticalCenter: parent.verticalCenter
            size: 28
            source: page.iconFor(ir.modelData)
            opacity: ir.dim ? 0.45 : 1
        }
        Column {
            anchors { left: irIcon.right; leftMargin: 12; right: irControls.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: ir.modelData.name
                elide: Text.ElideRight
                color: ir.dim ? Theme.color.secondary : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
            Text {
                width: parent.width
                text: page.statusText(ir.modelData)
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        Item {
            id: irControls
            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            width: (irSwitch.visible ? irSwitch.width + 8 : 0) + irMenu.width
            height: 28
            Switch {
                id: irSwitch
                visible: !ir.dim
                anchors.verticalCenter: parent.verticalCenter
                checked: ir.modelData.enabled
                accessibleName: "Start " + ir.modelData.name + " when you log in"
                onToggled: page.setOn(ir.modelData, !checked, false)
            }
            IconButton {
                id: irMenu
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                icon: "dots-three"
                iconColor: pageMenu.anchor === irMenu && pageMenu.shown ? Theme.color.label : Theme.color.tertiary
                tooltip: "Options for " + ir.modelData.name
                onClicked: page.openMenu(irMenu, ir.modelData)
            }
        }
    }

    // ── The master switch ───────────────────────────────────────────────
    Group {
        width: parent.width
        footer: page.startAtLogin ? "Apps that ask to start with your session (in ~/.config/autostart and /etc/xdg/autostart)." : "Nothing below starts when you log in. You can still start one now from its menu."
        Row {
            label: "Start them when you log in"
            detail: "Once each time you log in, after the menu bar is up"
            Switch {
                checked: page.startAtLogin
                onToggled: Config.options.loginItems.startAtLogin = !checked
            }
        }
    }

    // ── Loading, error, empty ───────────────────────────────────────────
    Text {
        visible: !page.loaded
        leftPadding: 4
        text: "Loading login items…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }
    Group {
        visible: page.loadError !== ""
        width: parent.width
        Row {
            label: "Couldn't read your login items"
            labelColor: Theme.color.danger
            detail: page.loadError
            SettingsButton {
                text: "Try again"
                icon: "arrow-clockwise"
                onClicked: {
                    page.loaded = false;
                    page.refresh();
                }
            }
        }
    }
    Group {
        visible: page.empty && !page.picking
        width: parent.width
        Row {
            label: "No login items"
            detail: "Apps you add here start when you log in."
            SettingsButton {
                text: "Add app…"
                icon: "plus"
                primary: true
                onClicked: page.startPicking()
            }
        }
    }

    // ── The list ────────────────────────────────────────────────────────
    Group {
        visible: page.active.length > 0
        width: parent.width
        title: "Open at login"
        footer: "Turning one off here turns it off in other desktops too."
        Repeater {
            model: ScriptModel { values: page.active }
            ItemRow {}
        }
    }

    // ── Add app… ────────────────────────────────────────────────────────
    property bool picking: false
    function startPicking() {
        picking = true;
        Qt.callLater(() => picker.focusInput());
    }
    Group {
        visible: page.loaded && page.loadError === "" && !page.empty && !page.picking
        width: parent.width
        Row {
            label: "Add an app to open at login"
            detail: "Pick from the apps installed on this computer"
            SettingsButton {
                text: "Add app…"
                icon: "plus"
                primary: true
                onClicked: page.startPicking()
            }
        }
    }
    Group {
        id: pickGroup
        visible: page.picking
        width: parent.width
        title: "Add an app"
        readonly property string q: picker.text.trim().toLowerCase()
        readonly property var have: page.allItems.map(it => it.file.replace(/\.desktop$/, ""))
        readonly property var matches: q === "" ? [] : Apps.entries.filter(a => !a.noDisplay && !have.includes(a.id) && (a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q))).sort((a, b) => a.name.localeCompare(b.name)).slice(0, 8)
        Row {
            label: "Search apps"
            QQ.Row {
                spacing: 8
                TextField {
                    id: picker
                    width: 240
                    placeholder: "App name"
                    commitOnBlur: false
                    accessibleName: "Search apps to add"
                    Keys.onEscapePressed: page.picking = false
                    onCommitted: if (pickGroup.matches.length > 0) page.addApp(pickGroup.matches[0])
                }
                SettingsButton {
                    text: "Cancel"
                    onClicked: {
                        picker.text = "";
                        page.picking = false;
                    }
                }
            }
        }
        Row {
            visible: pickGroup.q === ""
            label: "Type part of an app's name, like “sync” or “mail”"
            labelColor: Theme.color.secondary
        }
        Row {
            visible: pickGroup.q !== "" && pickGroup.matches.length === 0
            label: "No apps match “" + picker.text.trim() + "”"
            detail: "Check the spelling, or install the app first. Apps that are already login items aren't listed."
        }
        Repeater {
            model: ScriptModel { values: pickGroup.matches }
            QQ.Rectangle {
                id: hit
                required property var modelData
                width: parent?.width ?? 0
                height: 48
                color: hitMouse.pressed ? Theme.color.fillPressed : hitMouse.containsMouse ? Theme.color.fill : "transparent"
                AppIcon {
                    id: hitIcon
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    size: 28
                    source: Apps.iconFor(hit.modelData)
                }
                Text {
                    anchors { left: hitIcon.right; leftMargin: 12; right: hitAdd.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    text: hit.modelData.name
                    elide: Text.ElideRight
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
                MouseArea {
                    id: hitMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: page.addApp(hit.modelData)
                }
                SettingsButton {
                    id: hitAdd
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    text: "Add"
                    onClicked: page.addApp(hit.modelData)
                }
            }
        }
    }

    // ── Skipped by iRice ────────────────────────────────────────────────
    Group {
        visible: page.ignored.length > 0
        width: parent.width
        title: "Not started by iRice"
        footer: "These ask to start with your session, but iRice leaves them out: it does their job itself, they're for another desktop, or they aren't installed."
        Repeater {
            model: ScriptModel { values: page.ignored }
            ItemRow {}
        }
    }

    Toast {
        id: toast
    }
    ActionMenu {
        id: pageMenu
    }
}
