pragma Singleton
import QtQuick
import Quickshell

// Tessera's old name in the QML shell: the one place that still knows it.
// Tessera was called iRice until 4 November 2026. What an installed machine
// kept under that name moves over once as the shell starts
// (assets/scripts/migrate-names.py); what can't move by itself is read here
// until it has: the system pieces until the root setup (tessera-root.py
// setup) replaces them, keyring entries and files you made (settings
// backups, exported shortcuts) under the old name.
// REMOVE AFTER 2027-02-01, with old-names.sh, migrate-names.py, tessera-root.py's
// own section and core's old_names.rs (docs/rename-to-tessera.md), and the
// lines that use these.
Singleton {
    id: root

    readonly property string removeAfter: "2027-02-01"
    // AI keys saved in the keyring under the old name (looked up after
    // Tessera's, cleared with it)
    readonly property string keyringApp: "irice"
    // A settings backup made under the old name: its extension
    readonly property string backupExt: "irice"
    // Shortcuts exported under the old name: their file's top key
    readonly property string keybindsKey: "irice-keybinds"
    // Settings values saved under the old name (menu icon, screenshot editor)
    readonly property string settingValue: "irice"
    // The root helper until the root setup installs Tessera's, its polkit
    // actions, and its folders
    readonly property string rootHelper: "/usr/lib/irice/irice-root"
    readonly property string rootPolicy: "/usr/share/polkit-1/actions/org.irice.root.policy"
    readonly property string etc: "/etc/irice"
    readonly property string plymouthBreathe: "/usr/lib/irice/plymouth/irice-breathe"
    // The shell's priority rule until the root setup replaces it
    readonly property string ananicyRule: "/etc/ananicy.d/irice.rules"
    // The login screen's copy until ./install.sh --only=login replaces it
    readonly property string loginShare: "/usr/share/irice"

    // sh: the helper that's there as $h and its polkit actions as $policy:
    // Tessera's (path, policy), else the old one while only it is installed
    function rootPick(path, policy) {
        return "h=" + path + "; policy=" + policy + "; "
            + "if [ ! -x \"$h\" ] && [ -x " + rootHelper + " ]; then h=" + rootHelper + "; policy=" + rootPolicy + "; fi; ";
    }
    // A helper's message without its name in front (either name)
    function withoutHelperPrefix(text) {
        return String(text ?? "").replace(/^(tessera|irice)-root: /, "");
    }
    // A boot animation's id from its Plymouth theme (either name), or ""
    function plymouthId(theme) {
        const m = /^(tessera|irice)-(.+)$/.exec(theme ?? "");
        return m ? m[2] : "";
    }
}
