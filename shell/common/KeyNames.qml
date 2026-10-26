pragma Singleton
import QtQuick
import Quickshell

// How shortcuts are written in menus, Settings and the cheatsheet:
//   mac      ⌃⌘F        (symbols, no separators, macOS order ⌃ ⌥ ⇧ ⌘)
//   windows  Win+Ctrl+F
//   linux    Super+Ctrl+F
// "auto" follows the menu bar's icon: Apple → mac, Windows → windows,
// anything else → linux.
Singleton {
    id: root

    readonly property string style: {
        const s = Config.options.appearance.keySymbols;
        if (s !== "auto")
            return s;
        const icon = Config.options.appearance.menuIcon;
        return icon === "apple" ? "mac" : icon === "windows" ? "windows" : "linux";
    }

    readonly property var mods: ["CTRL", "ALT", "SHIFT", "SUPER"] // macOS order
    readonly property var names: ({
            mac: { SUPER: "⌘", CTRL: "⌃", ALT: "⌥", SHIFT: "⇧", Escape: "⎋", Return: "↩", BackSpace: "⌫", Tab: "⇥", Space: "Space", Left: "←", Right: "→", Up: "↑", Down: "↓" },
            windows: { SUPER: "Win", CTRL: "Ctrl", ALT: "Alt", SHIFT: "Shift", Escape: "Esc", Return: "Enter", BackSpace: "Backspace", Tab: "Tab", Space: "Space", Left: "←", Right: "→", Up: "↑", Down: "↓" },
            linux: { SUPER: "Super", CTRL: "Ctrl", ALT: "Alt", SHIFT: "Shift", Escape: "Esc", Return: "Enter", BackSpace: "Backspace", Tab: "Tab", Space: "Space", Left: "←", Right: "→", Up: "↑", Down: "↓" }
        })

    // One key's label: "SUPER" / "Super" / "super" / "Super_L" all work
    function name(k) {
        const t = names[style] ?? names.linux;
        const up = String(k).replace(/_[LR]$/, "").toUpperCase();
        const key = up === "CONTROL" ? "CTRL" : up === "META" || up === "MOD4" || up === "WIN" ? "SUPER" : up;
        if (t[key] !== undefined)
            return t[key];
        const exact = Object.keys(t).find(x => x.toUpperCase() === up);
        if (exact)
            return t[exact];
        return k.length === 1 ? k.toUpperCase() : k;
    }

    // ["SUPER", "CTRL", "F"] (any order) -> labels in the platform's order
    function keys(list) {
        const up = list.map(k => String(k).toUpperCase().replace(/_[LR]$/, "").replace("CONTROL", "CTRL"));
        const m = mods.filter(x => up.includes(x));
        const rest = list.filter((k, i) => !mods.includes(up[i]));
        const ordered = style === "mac" ? m : ["SUPER", "CTRL", "ALT", "SHIFT"].filter(x => m.includes(x));
        return ordered.concat(rest).map(name);
    }

    // "SUPER CTRL F" -> "⌃⌘F" / "Win+Ctrl+F" / "Super+Ctrl+F"
    function combo(spec) {
        const k = keys(spec.split(/[\s+]+/).filter(x => x !== ""));
        return style === "mac" ? k.join("") : k.join("+");
    }
}
