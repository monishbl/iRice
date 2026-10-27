import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Software Update: iRice's own updates (services/IriceUpdate.qml). Shows
// what's new and every conflict the update would bring, each with its
// choices; "Install update" is only possible once all of them are decided.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var report: IriceUpdate.report
    readonly property var kinds: [
        { kind: "setting", title: "Settings with new defaults", footer: "You kept these at their defaults. Your current value stays unless you choose the new one." },
        { kind: "preset", title: "Your glass material", footer: "Keeping your look saves it as a Custom material." },
        { kind: "shortcut", title: "Shortcut clashes", footer: "The update brings a shortcut on keys you already use." },
        { kind: "removed", title: "Settings that change name or go away", footer: "You changed these, and the update renames or no longer has them." },
        { kind: "code", title: "Files you edited", footer: "You changed these iRice files, and the update changes them too." }
    ]

    // ── Status ───────────────────────────────────────────────────────────
    Group {
        width: parent.width
        footer: !IriceUpdate.allowed ? "Updates are off in the dev sandbox (they would change this copy of iRice)." : IriceUpdate.result ? "Installed. Your settings and shortcuts were backed up to " + IriceUpdate.result.backup.replace(Quickshell.env("HOME"), "~") + "." : ""
        Row {
            label: IriceUpdate.applying ? "Installing the update…" : IriceUpdate.result ? "Update installed" : IriceUpdate.checking ? "Checking for updates…" : IriceUpdate.available ? "An update is ready" : page.report && !page.report.error ? "iRice is up to date" : "iRice updates"
            detail: page.report ? "This copy: " + page.report.current + (IriceUpdate.available ? " · latest: " + page.report.latest : "") : "Checked automatically every six hours."
            SettingsButton {
                visible: IriceUpdate.allowed && !IriceUpdate.applying
                text: IriceUpdate.available ? "Check again" : "Check for updates"
                icon: "arrow-clockwise"
                busy: IriceUpdate.checking
                onClicked: IriceUpdate.check()
            }
        }
    }

    // Errors say what happened and what to do
    Group {
        visible: IriceUpdate.error !== ""
        width: parent.width
        Row {
            label: "The update can't go ahead"
            detail: IriceUpdate.error
            SettingsButton {
                text: "Try again"
                onClicked: IriceUpdate.check()
            }
        }
    }

    // ── What's new ───────────────────────────────────────────────────────
    Group {
        visible: IriceUpdate.available && (page.report?.notes?.length ?? 0) > 0
        width: parent.width
        title: "What's new"
        Repeater {
            model: page.report?.notes ?? []
            Row {
                required property string modelData
                label: modelData
            }
        }
    }

    // ── Conflicts, by kind ───────────────────────────────────────────────
    Repeater {
        model: IriceUpdate.available ? page.kinds.filter(k => IriceUpdate.conflicts.some(c => c.kind === k.kind)) : []
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
                    label: modelData.title
                    detail: modelData.detail
                    Segmented {
                        width: Math.max(implicitWidth, 260)
                        options: conflict.modelData.choices.map(ch => conflict.modelData.labels?.[ch] ?? ch)
                        // (-1 until chosen: nothing is picked for you)
                        current: conflict.modelData.choices.indexOf(IriceUpdate.choices[conflict.modelData.id])
                        onSelected: i => IriceUpdate.choose(conflict.modelData.id, conflict.modelData.choices[i])
                    }
                }
            }
        }
    }

    // ── Install ──────────────────────────────────────────────────────────
    Group {
        visible: IriceUpdate.available
        width: parent.width
        footer: (page.report?.keeps?.length ?? 0) > 0 ? "Your edits to " + page.report.keeps.length + " other file" + (page.report.keeps.length === 1 ? "" : "s") + " stay as they are (the update doesn't change them)." : "Your settings and shortcuts are backed up first. The shell restarts once it's installed; your apps keep running."
        Row {
            label: IriceUpdate.undecided > 0 ? IriceUpdate.undecided + " thing" + (IriceUpdate.undecided === 1 ? "" : "s") + " left to decide" : "Ready to install"
            detail: IriceUpdate.undecided > 0 ? "Choose for each item above; nothing changes until you install." : (page.report?.rebuild?.length ?? 0) > 0 ? "Also rebuilds: " + page.report.rebuild.join(", ") + "." : ""
            SettingsButton {
                text: "Install update"
                icon: "arrow-circle-up"
                primary: true
                enabled: IriceUpdate.ready && IriceUpdate.allowed
                opacity: enabled ? 1 : 0.45
                busy: IriceUpdate.applying
                onClicked: IriceUpdate.apply()
            }
        }
    }
}
