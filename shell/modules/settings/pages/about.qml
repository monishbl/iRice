import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        Row { label: "iRice"; Text { text: "1.0"; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "System"; Text { text: SystemInfo.osName; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Computer name"; Text { text: SystemInfo.hostname; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row {
            label: "Your name"
            detail: "Today greets you by it. Empty: your account's name"
            TextField {
                width: 220
                placeholder: Today.accountName.split(/\s+/)[0] || "Your name"
                text: Config.options.today.name
                onCommitted: t => Config.options.today.name = t.trim()
            }
        }
        Row { label: "Processor"; Text { text: SystemInfo.cpu; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Graphics"; Text { text: SystemInfo.gpu; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Memory"; Text { text: SystemInfo.memory; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
    }
    Group {
        width: parent.width
        SubpageLink {
            icon: "heartbeat"
            label: "System health"
            detail: "Whether the services iRice uses are working, and how to fix them"
            value: Health.checks.length === 0 ? "" : Health.attention.length === 0 ? "All good" : Health.attention.length + " to check"
            page: "about/health"
        }
    }
    Group {
        width: parent.width
        footer: "Settings live in ~/.config/irice/config.json and apply as you change them."
        Row {
            label: "Configuration file"
            Rectangle {
                width: openText.implicitWidth + 24
                height: 26
                radius: 13
                color: Theme.color.fill
                Text { id: openText; anchors.centerIn: parent; text: "Open"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { anchors.fill: parent; onClicked: Launch.open("file://" + Config.dir + "/config.json") }
            }
        }
        Row {
            label: "Welcome screen"
            detail: "Pick the look, light or dark, and a wallpaper again"
            Rectangle {
                width: welcomeText.implicitWidth + 24
                height: 26
                radius: 13
                color: Theme.color.fill
                activeFocusOnTab: true
                Keys.onSpacePressed: Panels.welcomeRequested()
                Keys.onReturnPressed: Panels.welcomeRequested()
                Accessible.role: Accessible.Button
                Accessible.name: "Show the welcome screen"
                Text { id: welcomeText; anchors.centerIn: parent; text: "Show"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Panels.welcomeRequested() }
                FocusRing { radius: 17 }
            }
        }
    }

    // ── Back up and restore: your setup in one file ─────────────────────
    // settings-backup.py: the settings, shortcuts, Focus modes and wallpaper,
    // without keys, passwords, accounts or this computer's displays. Restore
    // shows what's in the file first; after it, Undo (8 s) puts back what
    // was there (the helper saved it to backups/ before writing).
    Group {
        id: backup
        width: parent.width
        title: "Back up and restore"
        footer: status !== "" ? status : "One file with your settings, shortcuts, Focus modes and wallpaper, without passwords or accounts. Restore it here or on another computer."
        property string status: ""
        property string savedTo: ""
        property var pending: null // { file, info }: waiting for Restore
        readonly property bool busy: helper.running || FilePicker.busy
        readonly property string script: Quickshell.shellDir + "/assets/scripts/settings-backup.py"

        function run(args, done) {
            helper.done = done;
            helper.command = ["python3", "-I", script].concat(args);
            helper.running = true;
        }
        Process {
            id: helper
            property var done: null
            stdout: StdioCollector {
                onStreamFinished: {
                    let out;
                    try {
                        out = JSON.parse(text);
                    } catch (e) {
                        out = { ok: false, error: "The backup helper didn't answer. Check that Python 3 is installed, then try again." };
                    }
                    const d = helper.done;
                    helper.done = null;
                    if (d)
                        d(out);
                }
            }
        }
        function folderOf(p) {
            return p.slice(0, p.lastIndexOf("/")) || "/";
        }
        function backUp() {
            FilePicker.pick({ folder: true, title: "Choose where to save your settings", start: "~/Documents" }, dir => {
                status = "Saving…";
                savedTo = "";
                run(["export", dir, "--config", Config.dir], out => {
                    status = out.ok ? "" : out.error;
                    savedTo = out.ok ? out.file : "";
                });
            });
        }
        function choose() {
            FilePicker.pick({ title: "Choose an iRice settings file", start: "~/Documents", filter: "iRice settings|*.irice" }, file => {
                status = "Reading…";
                pending = null;
                run(["inspect", file], out => {
                    status = out.ok ? "" : out.error;
                    pending = out.ok ? { file: file, info: out } : null;
                });
            });
        }
        function describe(i) {
            const parts = [(i.look === "material" ? "Material" : "Glass") + " look" + (i.mode ? ", " + (({ light: "light", dark: "dark", auto: "light and dark by the sun" })[i.mode] ?? i.mode) : "")];
            parts.push("wallpaper: " + i.wallpaper);
            if (i.shortcuts > 0)
                parts.push(i.shortcuts + (i.shortcuts === 1 ? " shortcut change" : " shortcut changes"));
            if (i.focusModes.length > 0)
                parts.push("Focus: " + i.focusModes.join(", "));
            return parts.join(" · ") + (i.created ? ". Saved " + Qt.formatDateTime(new Date(i.created), "d MMM yyyy, h:mm AP") + "." : ".");
        }
        // The look restored: switched to, as Settings → Appearance does
        function follow(look) {
            if (look && look !== (Theme.material ? "material" : "glass"))
                Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/switch-look.sh", look]);
        }
        function restore() {
            const p = pending;
            pending = null;
            status = "Restoring…";
            run(["restore", p.file, "--config", Config.dir], out => {
                if (!out.ok) {
                    status = out.error;
                    return;
                }
                status = out.skipped.length > 0 ? "Left out " + out.skipped.join(" and ") + "." : "";
                Focus.reload();
                toast.show("Settings restored", () => {
                    run(["restore", out.undo, "--config", Config.dir, "--no-undo"], back => {
                        status = back.ok ? "" : back.error;
                        if (back.ok) {
                            Focus.reload();
                            toast.show("Your earlier settings are back", null);
                            follow(back.look);
                        }
                    });
                });
                follow(out.look);
            });
        }

        Row {
            label: "Back up"
            detail: "Saves “iRice settings " + Qt.formatDate(new Date(), "yyyy-MM-dd") + ".irice” in a folder you choose"
            PageButton { text: "Back Up Settings…"; enabled: !backup.busy; onClicked: backup.backUp() }
        }
        Row {
            visible: backup.savedTo !== ""
            label: "Saved"
            detail: backup.savedTo.replace(Quickshell.env("HOME"), "~")
            PageButton { text: "Show in Files"; onClicked: Launch.open("file://" + backup.folderOf(backup.savedTo)) }
        }
        Row {
            label: "Restore"
            detail: "Your keys, accounts and displays here stay as they are"
            PageButton { text: "Restore…"; enabled: !backup.busy; onClicked: backup.choose() }
        }
        Row {
            visible: backup.pending !== null
            label: "Restore these settings?"
            detail: backup.pending ? backup.describe(backup.pending.info) : ""
            QQ.Row {
                spacing: 8
                PageButton { text: "Cancel"; onClicked: backup.pending = null }
                PageButton { text: "Restore Settings"; primary: true; enabled: !backup.busy; onClicked: backup.restore() }
            }
        }
    }

    Toast {
        id: toast
    }

    // A pill button (Material: a 40 px tonal one, or filled when primary)
    component PageButton: Rectangle {
        id: pb
        property string text
        property bool primary: false
        signal clicked
        width: pbText.implicitWidth + (Theme.material ? 32 : 24)
        height: Theme.material ? 40 : 26
        radius: height / 2
        opacity: enabled ? 1 : 0.5
        color: primary ? (Theme.material ? Theme.m3.primary : Theme.accent) : Theme.material ? Theme.m3.secondaryContainer : Theme.color.fill
        activeFocusOnTab: enabled
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                pb.clicked();
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: pb.clicked()
        FocusRing {
            radius: pb.radius + 4
        }
        Text {
            id: pbText
            anchors.centerIn: parent
            text: pb.text
            color: pb.primary ? (Theme.material ? Theme.m3.onPrimary : Theme.color.onAccent) : Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium }
        }
        MouseArea {
            anchors.fill: parent
            enabled: pb.enabled
            cursorShape: Qt.PointingHandCursor
            onClicked: pb.clicked()
        }
    }
}
