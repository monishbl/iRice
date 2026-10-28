import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Software Update: one Check for updates, one plan, one Update for iRice and
// the system together (services/SoftwareUpdate.qml; nothing is checked on
// its own). The plan lists what's new in iRice and every conflict to decide,
// the system's updates, iRice's packages, and Hyprland when the new one isn't
// supported yet; Update runs the system part, then iRice's, with one
// progress view (it keeps going with this window closed).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var report: IriceUpdate.report
    readonly property var plan: SoftwareUpdate.plan
    readonly property var job: Updates.job
    readonly property bool over: !!job && job.state !== "running" && !SoftwareUpdate.running
    readonly property bool showPlan: SoftwareUpdate.checked && !SoftwareUpdate.running && !page.over && SoftwareUpdate.anything
    readonly property var kinds: [
        { kind: "setting", title: "Settings with new defaults", footer: "You kept these at their defaults. Your current value stays unless you choose the new one." },
        { kind: "preset", title: "Your glass material", footer: "Keeping your look saves it as a Custom material." },
        { kind: "shortcut", title: "Shortcut clashes", footer: "The update brings a shortcut on keys you already use." },
        { kind: "removed", title: "Settings that change name or go away", footer: "You changed these, and the update renames or no longer has them." },
        { kind: "code", title: "Files you changed", footer: "You changed these iRice files, and the update changes them too. Keep mine drops the update's change to that file; Use the update's sets yours aside in the backup folder; Merge both is offered when the two fit together." }
    ]
    function plural(n, word) {
        return n + " " + word + (n === 1 ? "" : "s");
    }

    // ── Status, and Check for updates ───────────────────────────────────
    Group {
        width: parent.width
        footer: !IriceUpdate.allowed ? "Updates are off in the dev sandbox (they would change this copy of iRice); the system part is simulated." : IriceUpdate.result ? "iRice was updated. Your settings and shortcuts were backed up" + (IriceUpdate.result.backup ? " to " + IriceUpdate.result.backup.replace(Quickshell.env("HOME"), "~") : "") + "." : "Nothing is checked on its own. Checking downloads nothing but the package lists."
        Row {
            label: SoftwareUpdate.running ? "Updating…" : SoftwareUpdate.checking ? "Checking for updates…" : !SoftwareUpdate.checked ? "Software Update" : SoftwareUpdate.anything ? "Updates are ready" : "Everything is up to date"
            detail: !SoftwareUpdate.checked ? "iRice and your system (pacman, and the AUR if you use yay or paru), together." : page.report ? "iRice " + page.report.current + (IriceUpdate.available ? " → " + page.report.latest : "") + (SoftwareUpdate.hyprNow ? " · Hyprland " + SoftwareUpdate.hyprNow : "") : ""
            SettingsButton {
                visible: !SoftwareUpdate.running
                text: SoftwareUpdate.checked ? "Check again" : "Check for updates"
                icon: "arrow-clockwise"
                busy: SoftwareUpdate.checking
                onClicked: SoftwareUpdate.check()
            }
        }
    }

    // Errors say what happened and what to do
    Group {
        visible: IriceUpdate.error !== "" && !SoftwareUpdate.running
        width: parent.width
        Row {
            label: "iRice's part can't go ahead"
            detail: IriceUpdate.error
            SettingsButton {
                text: "Try again"
                onClicked: SoftwareUpdate.check()
            }
        }
    }

    // ── The plan: iRice ─────────────────────────────────────────────────
    Group {
        visible: page.showPlan && IriceUpdate.available && ((page.report?.changelog?.length ?? 0) > 0 || (page.report?.notes?.length ?? 0) > 0)
        width: parent.width
        title: "What's new in iRice"
        UpdateNotes {
            x: 14
            width: parent.width - 28
            topPadding: 14
            bottomPadding: 14
            entries: page.report?.changelog ?? []
            fallback: page.report?.notes ?? []
        }
    }
    Repeater {
        model: page.showPlan && IriceUpdate.available ? page.kinds.filter(k => IriceUpdate.conflicts.some(c => c.kind === k.kind)) : []
        Group {
            id: section
            required property var modelData
            width: page.width
            title: modelData.title
            footer: modelData.footer
            Repeater {
                model: IriceUpdate.conflicts.filter(c => c.kind === section.modelData.kind)
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
                        current: conflict.modelData.choices.indexOf(IriceUpdate.choices[conflict.modelData.id])
                        onSelected: i => IriceUpdate.choose(conflict.modelData.id, conflict.modelData.choices[i])
                    }
                }
            }
        }
    }

    // ── The plan: Hyprland newer than this iRice supports ───────────────
    Group {
        visible: page.showPlan && SoftwareUpdate.hyprTooNew
        width: parent.width
        title: "Hyprland"
        footer: SoftwareUpdate.hold === 1 ? "Held for this update only: " + SoftwareUpdate.held.join(", ") + ". Asked again next time." : SoftwareUpdate.hold === 0 ? "If the Liquid Glass plugin can't be built for it, glass falls back to frosted until iRice catches up." : "This iRice is tested with Hyprland " + (SoftwareUpdate.compat?.hyprland?.max ?? "") + "."
        Row {
            label: "Hyprland " + SoftwareUpdate.hyprShort + " isn't supported by this iRice yet"
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
    // System (name · old → new, repo or AUR, held ones greyed), what iRice
    // adds (and why) and drops; long lists show 8 and "and N more"
    component PkgLine: Item {
        id: line
        property string name
        property string from
        property string to
        property string badge // AUR | held for iRice | a reason
        property bool dim: false
        width: parent?.width ?? 0
        height: 26
        opacity: dim ? 0.5 : 1
        Text {
            id: pkgName
            anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
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
            anchors { right: parent.right; rightMargin: 14; left: badgeBox.visible ? badgeBox.right : pkgName.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
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
        leftPadding: 14
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
        leftPadding: 14
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
        visible: page.showPlan && (page.plan.system || page.plan.aur || IriceUpdate.packages.missing.length > 0 || IriceUpdate.packages.dropped.length > 0)
        width: parent.width
        title: "Packages affected"
        footer: SoftwareUpdate.snapshot !== "" ? "A snapshot is taken first (" + SoftwareUpdate.snapshot + "), so you can roll back." : "Without a snapshot tool, pacman keeps the old packages in its cache, and iRice backs up your settings."
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
                    badge: isHeld ? "held for iRice" : modelData.aur ? (SoftwareUpdate.includeAur ? "AUR" : "AUR · not included") : ""
                    dim: isHeld || (modelData.aur && !SoftwareUpdate.includeAur)
                }
            }
            More {
                id: sysMore
                total: parent.sys.length
            }
            // iRice adds
            PkgHeading {
                visible: IriceUpdate.packages.missing.length > 0
                text: "iRice adds"
            }
            Repeater {
                model: addMore.open ? IriceUpdate.packages.missing : IriceUpdate.packages.missing.slice(0, 8)
                PkgLine {
                    required property string modelData
                    name: modelData
                    to: IriceUpdate.packages.reasons?.[modelData] ?? ""
                }
            }
            More {
                id: addMore
                total: IriceUpdate.packages.missing.length
            }
            // iRice drops
            PkgHeading {
                visible: IriceUpdate.packages.dropped.length > 0
                text: "iRice drops"
            }
            Repeater {
                model: dropMore.open ? IriceUpdate.packages.dropped : IriceUpdate.packages.dropped.slice(0, 8)
                PkgLine {
                    required property string modelData
                    name: modelData
                    badge: SoftwareUpdate.removeDropped ? "removed" : "kept"
                    dim: !SoftwareUpdate.removeDropped
                }
            }
            More {
                id: dropMore
                total: IriceUpdate.packages.dropped.length
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
            visible: IriceUpdate.packages.dropped.length > 0
            label: "Remove packages iRice no longer uses"
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
        footer: "Your system is updated first, then iRice, which restarts the shell; your apps keep running. Your password is asked by iRice."
        Row {
            label: SoftwareUpdate.undecided > 0 ? page.plural(SoftwareUpdate.undecided, "thing") + " left to decide" : "Ready to update"
            detail: SoftwareUpdate.undecided > 0 ? "Choose for each item above; nothing changes until you update." : [page.plan.system || page.plan.aur ? "the system" : "", page.plan.install.length > 0 || page.plan.remove.length > 0 ? "iRice's packages" : "", page.plan.irice ? "iRice" : page.plan.rebuild.length > 0 ? "the glass plugin (rebuilt for the new Hyprland)" : ""].filter(x => x).join(", ").replace(/^./, c => c.toUpperCase())
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
            label: Updates.interrupted ? "The last update was interrupted" : SoftwareUpdate.running ? (IriceUpdate.applying ? "Updating iRice" : page.job?.text || "Updating") : page.job?.state === "done" ? ((page.job.upgraded?.length ?? 0) > 0 ? page.plural(page.job.upgraded.length, "package") + " updated" : "Updated") : page.job?.text ?? ""
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
                anchors { left: parent.left; leftMargin: 14; right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
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
            Rectangle {
                anchors { left: parent.left; leftMargin: 14; right: pct.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                height: 6
                radius: 3
                color: Theme.color.fill
                Rectangle {
                    width: parent.width * Math.max(0.02, Updates.progress)
                    height: parent.height
                    radius: 3
                    color: Theme.accent
                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
            }
            Text {
                id: pct
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
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
            Rectangle {
                anchors { fill: parent; margins: 10 }
                radius: 8
                color: Theme.dark ? "#0E0E10" : "#18181B"
                Flickable {
                    id: logView
                    anchors { fill: parent; margins: 10 }
                    clip: true
                    contentHeight: logText.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds
                    // Follows the end, unless you scrolled up to read
                    property bool follow: true
                    onMovementEnded: follow = contentY >= contentHeight - height - 4
                    onContentHeightChanged: if (follow) contentY = Math.max(0, contentHeight - height)
                    Text {
                        id: logText
                        width: logView.width
                        text: Updates.log === "" ? "Waiting for output…" : Updates.log.replace(/^@@step .*\n/gm, "")
                        wrapMode: Text.WrapAnywhere
                        textFormat: Text.PlainText
                        color: "#D4D4D8"
                        font { family: Theme.font.mono; pixelSize: 11 }
                    }
                }
                ScrollIndicator {
                    target: logView
                    anchors { right: parent.right; top: parent.top; bottom: parent.bottom; rightMargin: 2 }
                }
            }
        }
    }

    // ── After an iRice update: what's new, until Done ───────────────────
    Group {
        visible: !!IriceUpdate.result && !SoftwareUpdate.running && ((IriceUpdate.result.changelog?.length ?? 0) > 0 || (IriceUpdate.result.notes?.length ?? 0) > 0)
        width: parent.width
        title: "What's new"
        UpdateNotes {
            x: 14
            width: parent.width - 28
            topPadding: 14
            bottomPadding: 6
            entries: IriceUpdate.result?.changelog ?? []
            fallback: IriceUpdate.result?.notes ?? []
        }
        Row {
            label: "Updated to " + (IriceUpdate.result?.now ?? "")
            SettingsButton {
                text: "Done"
                onClicked: IriceUpdate.dismissResult()
            }
        }
    }
}
