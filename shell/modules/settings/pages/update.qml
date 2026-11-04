import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Software Update: one Check for updates, one plan, one Update for Tessera and
// the system together (services/SoftwareUpdate.qml; nothing is checked on
// its own). The plan lists what's new in Tessera and every conflict to decide,
// the system's updates, Tessera's packages, and Hyprland when the new one isn't
// supported yet; Update runs the system part, then Tessera's, with one
// progress view (it keeps going with this window closed).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var report: TesseraUpdate.report
    readonly property int pad: Theme.material ? 20 : 14 // (the lines' edge: a settings Row's)
    readonly property var plan: SoftwareUpdate.plan
    readonly property var job: Updates.job
    readonly property bool over: !!job && job.state !== "running" && !SoftwareUpdate.running
    Component.onCompleted: {
        SoftwareUpdate.checkUndo();
        BootSnapshots.refresh();
    }
    readonly property bool showPlan: SoftwareUpdate.checked && !SoftwareUpdate.running && !page.over && SoftwareUpdate.anything
    readonly property var kinds: [
        { kind: "setting", title: "Settings with new defaults", footer: "You kept these at their defaults. Your current value stays unless you choose the new one." },
        { kind: "preset", title: "Your glass material", footer: "Keeping your look saves it as a Custom material." },
        { kind: "shortcut", title: "Shortcut clashes", footer: "The update brings a shortcut on keys you already use." },
        { kind: "removed", title: "Settings that change name or go away", footer: "You changed these, and the update renames or no longer has them." },
        { kind: "code", title: "Files you changed", footer: "You changed these Tessera files, and the update changes them too. Keep mine drops the update's change to that file; Use the update's sets yours aside in the backup folder; Merge both is offered when the two fit together." }
    ]
    function plural(n, word) {
        return n + " " + word + (n === 1 ? "" : "s");
    }

    // ── Status, and Check for updates ───────────────────────────────────
    Group {
        width: parent.width
        footer: !TesseraUpdate.allowed ? "Updates are off in the dev sandbox (they would change this copy of Tessera); the system part is simulated." : TesseraUpdate.result ? "Tessera was updated. Your settings and shortcuts were backed up" + (TesseraUpdate.result.backup ? " to " + TesseraUpdate.result.backup.replace(Quickshell.env("HOME"), "~") : "") + "." : "Nothing is checked on its own. Checking downloads nothing but the package lists."
        Row {
            label: SoftwareUpdate.running ? "Updating…" : SoftwareUpdate.checking ? "Checking for updates…" : !SoftwareUpdate.checked ? "Software Update" : SoftwareUpdate.anything ? "Updates are ready" : "Everything is up to date"
            detail: !SoftwareUpdate.checked ? "Tessera and your system (pacman, and the AUR if you use yay or paru), together." : page.report ? "Tessera " + page.report.current + (TesseraUpdate.available ? " → " + page.report.latest : "") + (SoftwareUpdate.hyprNow ? " · Hyprland " + SoftwareUpdate.hyprNow : "") : ""
            SettingsButton {
                visible: !SoftwareUpdate.running
                text: SoftwareUpdate.checked ? "Check again" : "Check for updates"
                icon: "arrow-clockwise"
                busy: SoftwareUpdate.checking
                onClicked: SoftwareUpdate.check()
            }
        }
        // (the last update needed one; its result made way for a new check)
        Row {
            visible: SoftwareUpdate.restartPending && !SoftwareUpdate.running
            label: "Restart to finish the last update"
            detail: "The kernel or system libraries changed."
            SettingsButton {
                text: "Restart"
                icon: "arrow-clockwise"
                onClicked: Quickshell.execDetached(["systemctl", "reboot"])
            }
        }
    }

    // Errors say what happened and what to do
    Group {
        visible: TesseraUpdate.error !== "" && !SoftwareUpdate.running
        width: parent.width
        Row {
            label: "Tessera's part can't go ahead"
            detail: TesseraUpdate.error
            SettingsButton {
                text: "Try again"
                onClicked: SoftwareUpdate.check()
            }
        }
    }

    // ── The plan: Tessera ─────────────────────────────────────────────────
    Group {
        visible: page.showPlan && TesseraUpdate.available && ((page.report?.changelog?.length ?? 0) > 0 || (page.report?.notes?.length ?? 0) > 0)
        width: parent.width
        title: "What's new in Tessera"
        UpdateNotes {
            x: page.pad
            width: parent.width - 2 * page.pad
            topPadding: 14
            bottomPadding: 14
            entries: page.report?.changelog ?? []
            fallback: page.report?.notes ?? []
        }
    }
    Repeater {
        model: page.showPlan && TesseraUpdate.available ? page.kinds.filter(k => TesseraUpdate.conflicts.some(c => c.kind === k.kind)) : []
        Group {
            id: section
            required property var modelData
            width: page.width
            title: modelData.title
            footer: modelData.footer
            Repeater {
                model: TesseraUpdate.conflicts.filter(c => c.kind === section.modelData.kind)
                Row {
                    id: conflict
                    required property var modelData
                    readonly property bool file: modelData.kind === "code"
                    label: modelData.title
                    // Files: what each side did ("your changes +3 −1 · the update's +12 −4")
                    detail: file && modelData.mine ? modelData.detail + "\n" + modelData.mine + " · " + modelData.theirs : modelData.detail
                    // Short labels on the buttons for files (the long ones explain in the footer)
                    function short(ch) {
                        if (!file)
                            return modelData.labels?.[ch] ?? ch;
                        if (ch === "both")
                            return "Merge both";
                        if (ch === "mine")
                            return modelData.change === "deleted-mine" ? "Keep it deleted" : "Keep mine";
                        return modelData.change === "deleted-theirs" ? "Delete it" : modelData.change === "deleted-mine" ? "Bring it back" : "Use the update's";
                    }
                    Segmented {
                        width: Math.max(implicitWidth, 260)
                        options: conflict.modelData.choices.map(ch => conflict.short(ch))
                        // (-1 until chosen: nothing is picked for you)
                        current: conflict.modelData.choices.indexOf(TesseraUpdate.choices[conflict.modelData.id])
                        onSelected: i => TesseraUpdate.choose(conflict.modelData.id, conflict.modelData.choices[i])
                    }
                }
            }
        }
    }

    // ── The plan: Hyprland newer than this Tessera supports ───────────────
    Group {
        visible: page.showPlan && SoftwareUpdate.hyprTooNew
        width: parent.width
        title: "Hyprland"
        footer: SoftwareUpdate.hold === 1 ? "Held for this update only: " + SoftwareUpdate.heldHypr.join(", ") + ". Asked again next time." : SoftwareUpdate.hold === 0 ? "If the glass plugin can't be built for it, glass falls back to frosted until Tessera catches up." : "This Tessera is tested with Hyprland " + (SoftwareUpdate.compat?.hyprland?.max ?? "") + "."
        Row {
            label: "Hyprland " + SoftwareUpdate.hyprShort + " isn't supported by this Tessera yet"
            detail: "Choose what to do for this update."
            Segmented {
                width: Math.max(implicitWidth, 300)
                options: ["Keep " + (SoftwareUpdate.hyprNow || "the current one"), "Update anyway"]
                current: SoftwareUpdate.hold === 1 ? 0 : SoftwareUpdate.hold === 0 ? 1 : -1
                onSelected: i => SoftwareUpdate.hold = i === 0 ? 1 : 0
            }
        }
    }

    // ── The plan: packages affected ─────────────────────────────────────
    // System (name · old → new, repo or AUR, held ones greyed), what Tessera
    // adds (and why) and drops; long lists show 8 and "and N more"
    component PkgLine: Item {
        id: line
        property string name
        property string from
        property string to
        property string badge // AUR | held for Tessera | a reason
        property bool dim: false
        width: parent?.width ?? 0
        height: 26
        opacity: dim ? 0.5 : 1
        Text {
            id: pkgName
            anchors { left: parent.left; leftMargin: page.pad; verticalCenter: parent.verticalCenter }
            width: Math.min(implicitWidth, parent.width * 0.45)
            elide: Text.ElideRight
            text: line.name
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
        Rectangle {
            id: badgeBox
            visible: line.badge !== ""
            anchors { left: pkgName.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
            width: Math.min(badgeText.implicitWidth + 12, line.width * 0.3)
            height: 18
            radius: 9
            color: Theme.color.fill
            Text {
                id: badgeText
                anchors.centerIn: parent
                width: parent.width - 12
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
                text: line.badge
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        Text {
            anchors { right: parent.right; rightMargin: page.pad; left: badgeBox.visible ? badgeBox.right : pkgName.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
            horizontalAlignment: Text.AlignRight
            elide: Text.ElideLeft
            visible: line.to !== ""
            text: line.from !== "" ? line.from + "  →  " + line.to : line.to
            color: Theme.color.secondary
            // versions in mono (they line up); a reason in the normal font
            font { family: line.from !== "" || /\d/.test(line.to) && !/ /.test(line.to) ? Theme.font.mono : Theme.font.family; pixelSize: Theme.font.caption }
        }
    }
    component PkgHeading: Text {
        width: parent?.width ?? 0
        leftPadding: page.pad
        topPadding: 12
        bottomPadding: 4
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
    }
    component More: Text {
        id: more
        property int total: 0
        property bool open: false
        visible: total > 8
        leftPadding: page.pad
        topPadding: 2
        bottomPadding: 6
        text: open ? "Show fewer" : "and " + (total - 8) + " more"
        color: Theme.accent
        font { family: Theme.font.family; pixelSize: Theme.font.small }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: more.open = !more.open
        }
    }
    Group {
        visible: page.showPlan && (page.plan.system || page.plan.aur || TesseraUpdate.packages.missing.length > 0 || TesseraUpdate.packages.dropped.length > 0)
        width: parent.width
        title: "Packages affected"
        footer: SoftwareUpdate.snapshot !== "" ? "A snapshot is taken first (" + SoftwareUpdate.snapshot + "). Afterwards you can go back to the versions you have now, below." : "Afterwards you can go back to the versions you have now, below: pacman keeps them in its cache. Tessera backs up your settings."
        Column {
            width: parent.width
            bottomPadding: 8
            // System
            readonly property var sys: Updates.list.map(x => Object.assign({ aur: false }, x)).concat(Updates.aurList.map(x => Object.assign({ aur: true }, x)))
            PkgHeading {
                visible: parent.sys.length > 0
                text: "System · " + page.plural(page.plan.sysCount + (SoftwareUpdate.includeAur ? page.plan.aurCount : 0), "update") + (SoftwareUpdate.held.length > 0 ? " (" + SoftwareUpdate.held.length + " held)" : "")
            }
            Repeater {
                model: sysMore.open ? parent.sys : parent.sys.slice(0, 8)
                PkgLine {
                    required property var modelData
                    readonly property bool isHeld: SoftwareUpdate.held.includes(modelData.name)
                    name: modelData.name
                    from: modelData.from
                    to: modelData.to
                    badge: isHeld ? (SoftwareUpdate.heldHypr.includes(modelData.name) ? "held for Tessera" : "skipped · you went back from it") : modelData.aur ? (SoftwareUpdate.includeAur ? "AUR" : "AUR · not included") : ""
                    dim: isHeld || (modelData.aur && !SoftwareUpdate.includeAur)
                }
            }
            More {
                id: sysMore
                total: parent.sys.length
            }
            // Tessera adds
            PkgHeading {
                visible: TesseraUpdate.packages.missing.length > 0
                text: "Tessera adds"
            }
            Repeater {
                model: addMore.open ? TesseraUpdate.packages.missing : TesseraUpdate.packages.missing.slice(0, 8)
                PkgLine {
                    required property string modelData
                    name: modelData
                    to: TesseraUpdate.packages.reasons?.[modelData] ?? ""
                }
            }
            More {
                id: addMore
                total: TesseraUpdate.packages.missing.length
            }
            // Tessera drops
            PkgHeading {
                visible: TesseraUpdate.packages.dropped.length > 0
                text: "Tessera drops"
            }
            Repeater {
                model: dropMore.open ? TesseraUpdate.packages.dropped : TesseraUpdate.packages.dropped.slice(0, 8)
                PkgLine {
                    required property string modelData
                    name: modelData
                    badge: SoftwareUpdate.removeDropped ? "removed" : "kept"
                    dim: !SoftwareUpdate.removeDropped
                }
            }
            More {
                id: dropMore
                total: TesseraUpdate.packages.dropped.length
            }
        }
        Row {
            visible: Updates.aurList.length > 0
            label: "Include AUR updates"
            detail: "AUR packages are built from scripts anyone can submit, and they're built without showing you what changed. Include them only if you trust these packages."
            Switch {
                checked: SoftwareUpdate.includeAur
                onToggled: SoftwareUpdate.includeAur = !checked
            }
        }
        Row {
            visible: TesseraUpdate.packages.dropped.length > 0
            label: "Remove packages Tessera no longer uses"
            detail: "Turn off if you use them yourself."
            Switch {
                checked: SoftwareUpdate.removeDropped
                onToggled: SoftwareUpdate.removeDropped = !checked
            }
        }
    }

    // ── Update ──────────────────────────────────────────────────────────
    Group {
        visible: page.showPlan
        width: parent.width
        footer: "Your system is updated first, then Tessera, which restarts the shell; your apps keep running. Your password is asked once, at the start."
        Row {
            label: SoftwareUpdate.undecided > 0 ? page.plural(SoftwareUpdate.undecided, "thing") + " left to decide" : "Ready to update"
            detail: SoftwareUpdate.undecided > 0 ? "Choose for each item above; nothing changes until you update." : [page.plan.system || page.plan.aur ? "the system" : "", page.plan.install.length > 0 || page.plan.remove.length > 0 ? "Tessera's packages" : "", page.plan.setup && !page.plan.tessera ? "Tessera's system helper (for the firewall fix, Quick Share and more)" : "", page.plan.tessera ? "Tessera" : page.plan.rebuild.length > 0 ? "the glass plugin (rebuilt for the new Hyprland)" : ""].filter(x => x).join(", ").replace(/^./, c => c.toUpperCase())
            SettingsButton {
                text: "Update"
                icon: "arrow-circle-up"
                primary: true
                enabled: SoftwareUpdate.canUpdate
                opacity: enabled ? 1 : 0.45
                onClicked: SoftwareUpdate.update()
            }
        }
    }

    // ── Progress (while it runs, and how it ended) ──────────────────────
    Group {
        id: progressGroup
        visible: SoftwareUpdate.running || page.over
        width: parent.width
        title: "Update"
        footer: Updates.fake ? "In the dev sandbox the update is simulated; nothing is installed." : SoftwareUpdate.running ? "Keeps going if you close Settings; the island shows how far it is." : ""
        Row {
            label: Updates.interrupted ? "The last update was interrupted" : SoftwareUpdate.running ? (TesseraUpdate.applying ? "Updating Tessera" : page.job?.text || "Updating") : page.job?.state === "done" ? (page.job.kind === "undo" ? "Back to the earlier versions" : (page.job.upgraded?.length ?? 0) > 0 ? page.plural(page.job.upgraded.length, "package") + " updated" : "Updated") : page.job?.text ?? ""
            detail: Updates.interrupted ? "Update again to finish; pacman picks up where it stopped." : SoftwareUpdate.running ? (page.job?.phase === "auth" ? "Enter your password in the dialog." : page.job?.package ?? "") : page.job?.state === "error" ? page.job.error : page.job?.restart ? "Restart to finish: the kernel or system libraries changed." : ""
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: Updates.cancellable
                    text: "Cancel"
                    onClicked: SoftwareUpdate.cancel()
                }
                SettingsButton {
                    visible: page.over && page.job?.state === "done" && page.job?.restart === true
                    text: "Restart"
                    icon: "arrow-clockwise"
                    onClicked: Quickshell.execDetached(["systemctl", "reboot"])
                }
                SettingsButton {
                    visible: page.over
                    text: "Done"
                    onClicked: Updates.dismiss()
                }
            }
        }
        // The steps, the current one bold
        Item {
            visible: SoftwareUpdate.steps.length > 1
            width: parent.width
            height: 30
            Text {
                anchors { left: parent.left; leftMargin: page.pad; right: parent.right; rightMargin: page.pad; verticalCenter: parent.verticalCenter }
                elide: Text.ElideRight
                textFormat: Text.StyledText
                text: SoftwareUpdate.steps.map(s => s === SoftwareUpdate.stepNow ? "<b>" + SoftwareUpdate.stepName(s) + "</b>" : SoftwareUpdate.stepName(s)).join("  ·  ")
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
        Item {
            visible: Updates.running
            width: parent.width
            height: 32
            WavyProgress {
                anchors { left: parent.left; leftMargin: page.pad; right: pct.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                value: Math.max(0.02, Updates.progress)
            }
            Text {
                id: pct
                anchors { right: parent.right; rightMargin: page.pad; verticalCenter: parent.verticalCenter }
                text: Math.round(Math.max(0, Updates.progress) * 100) + "%"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
            }
        }
        // The full output, like a terminal
        Row {
            visible: !!page.job
            label: "Details"
            detail: "The full output of pacman" + (page.job?.steps?.includes("aur") ? " and the AUR helper" : "")
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: Updates.showDetails
                    text: "Copy"
                    icon: "copy"
                    onClicked: Updates.copyLog()
                }
                SettingsButton {
                    text: Updates.showDetails ? "Hide details" : "Show details"
                    icon: Updates.showDetails ? "caret-up" : "terminal-window"
                    onClicked: Updates.showDetails = !Updates.showDetails
                }
            }
        }
        Item {
            readonly property bool active: visible && Updates.showDetails
            visible: !!page.job && Updates.showDetails && progressGroup.visible
            onActiveChanged: Updates.logUsers += active ? 1 : -1
            Component.onDestruction: if (active) Updates.logUsers--
            width: parent.width
            height: 240
            LogView {
                anchors { fill: parent; margins: 10 }
                follow: true
                text: Updates.log === "" ? "Waiting for output…" : Updates.log.replace(/^@@step .*\n/gm, "")
            }
        }
    }

    // ── Going back ──────────────────────────────────────────────────────
    // The last update undone (its earlier package versions put back), the
    // versions you went back from (skipped until newer ones come), and
    // snapshots at startup (GRUB's menu: for when the desktop won't start)
    Group {
        id: goingBack
        readonly property bool fromSnapshot: BootSnapshots.booted !== ""
        visible: !SoftwareUpdate.running && (fromSnapshot || SoftwareUpdate.undoCount > 0 || SoftwareUpdate.skippedAll.length > 0 || BootSnapshots.canMenu)
        width: parent.width
        title: "Going back"
        footer: fromSnapshot ? "" : "If the desktop doesn't start after an update, Tessera offers to go back on its own. From a text console (Ctrl+Alt+F3), run: tessera undo-update"
        property bool asking: false // (Go Back pressed once: what it does, then Go Back again)

        // Started from a snapshot
        Row {
            visible: goingBack.fromSnapshot
            label: "Started from snapshot " + BootSnapshots.booted
            labelColor: Theme.accent
            detail: BootSnapshots.assistant ? "Your system is as it was then, until you restart. To keep it, restore this snapshot in Btrfs Assistant (Snapper → Restore), then restart."
                : "Your system is as it was then, until you restart. To keep it, restore this snapshot with Btrfs Assistant: install it, then choose Snapper → Restore."
            SettingsButton {
                text: BootSnapshots.assistant ? "Open Btrfs Assistant" : Needs.installing ? "Installing…" : "Install Btrfs Assistant"
                busy: !BootSnapshots.assistant && Needs.installing
                enabled: BootSnapshots.assistant || !Updates.running
                onClicked: BootSnapshots.assistant ? BootSnapshots.openAssistant() : Needs.install(["btrfs-assistant"])
            }
        }
        // The last update
        Row {
            visible: SoftwareUpdate.undoCount > 0 && !goingBack.fromSnapshot
            readonly property bool missing: (SoftwareUpdate.undo.missing?.length ?? 0) > 0
            label: goingBack.asking ? "Go back to the earlier versions?" : "Go back to before the last update"
            labelColor: missing ? Theme.color.secondary : Theme.color.label
            detail: missing ? "Some earlier versions aren't on this computer any more: " + (SoftwareUpdate.undo.missing ?? []).slice(0, 4).join(", ") + ((SoftwareUpdate.undo.missing?.length ?? 0) > 4 ? "…" : "")
                : goingBack.asking ? page.plural(SoftwareUpdate.undoCount, "package") + " go back to the version before " + Qt.formatDateTime(new Date(SoftwareUpdate.undo.at * 1000), "dddd h:mm ap") + ". Your files and settings stay as they are. It asks for your password."
                : "The update on " + Qt.formatDateTime(new Date(SoftwareUpdate.undo.at * 1000), "d MMMM, h:mm ap") + " changed " + page.plural(SoftwareUpdate.undoCount, "package") + ". " + (SoftwareUpdate.undo.packages ?? []).slice(0, 3).map(p => p.name).join(", ") + (SoftwareUpdate.undoCount > 3 ? "…" : "")
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: goingBack.asking
                    text: "Cancel"
                    onClicked: goingBack.asking = false
                }
                SettingsButton {
                    text: "Go Back"
                    icon: goingBack.asking ? "" : "clock-counter-clockwise"
                    enabled: SoftwareUpdate.canGoBack
                    onClicked: {
                        if (!goingBack.asking) {
                            goingBack.asking = true;
                            return;
                        }
                        goingBack.asking = false;
                        SoftwareUpdate.goBack();
                    }
                }
            }
        }
        // Skipped until a newer version comes
        Repeater {
            model: SoftwareUpdate.skippedAll
            Row {
                required property var modelData
                label: modelData.name + " " + modelData.to
                detail: SoftwareUpdate.includeSkipped.includes(modelData.name) ? "Included in this update" : "Skipped: you went back from this version. A newer one is installed as usual."
                Switch {
                    checked: SoftwareUpdate.includeSkipped.includes(parent.parent.modelData.name)
                    onToggled: SoftwareUpdate.includeSkip(parent.parent.modelData.name, !checked)
                }
            }
        }
        // Snapshots in the boot menu
        Row {
            visible: BootSnapshots.canMenu && !goingBack.fromSnapshot
            label: "Snapshots in the boot menu"
            detail: BootSnapshots.error !== "" ? BootSnapshots.error
                : !BootSnapshots.grubBtrfs ? "To start from a snapshot when the desktop won't load, GRUB can list them (grub-btrfs). It asks for your password once."
                : BootSnapshots.menuOn ? "GRUB lists your snapshots under “snapshots”: start from one to see your system as it was."
                : "GRUB can list your snapshots, to start from one when the desktop won't load."
            labelColor: BootSnapshots.error !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                visible: !BootSnapshots.grubBtrfs
                text: Needs.installing ? "Installing…" : "Install"
                busy: Needs.installing
                enabled: !Updates.running
                onClicked: Needs.install(["grub-btrfs"])
            }
            Switch {
                visible: BootSnapshots.grubBtrfs
                enabled: !BootSnapshots.busy
                checked: BootSnapshots.menuOn
                onToggled: BootSnapshots.setMenu(!checked)
            }
        }
    }

    // ── After a Tessera update: what's new, until Done ───────────────────
    Group {
        visible: !!TesseraUpdate.result && !SoftwareUpdate.running && ((TesseraUpdate.result.changelog?.length ?? 0) > 0 || (TesseraUpdate.result.notes?.length ?? 0) > 0)
        width: parent.width
        title: "What's new"
        UpdateNotes {
            x: page.pad
            width: parent.width - 2 * page.pad
            topPadding: 14
            bottomPadding: 6
            entries: TesseraUpdate.result?.changelog ?? []
            fallback: TesseraUpdate.result?.notes ?? []
        }
        Row {
            label: "Updated to " + (TesseraUpdate.result?.now ?? "")
            SettingsButton {
                text: "Done"
                onClicked: TesseraUpdate.dismissResult()
            }
        }
    }
}
