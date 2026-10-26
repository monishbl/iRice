pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Keybinds read live from Hyprland. Binds with a description of the form
// "Group: Action" are listed under that group (end4's convention).
Singleton {
    id: root

    property var groups: [] // [{ name, binds: [{ keys: ["Super", "Space"], description }] }]

    readonly property var modNames: [[64, "Super"], [4, "Ctrl"], [8, "Alt"], [1, "Shift"]]

    function refresh() {
        proc.running = true;
    }

    function prettyKey(k) {
        const map = { "Return": "Enter", "space": "Space", "Space": "Space", "Tab": "Tab", "Escape": "Esc", "Print": "Print Screen", "Period": ".", "period": ".", "Comma": ",", "Slash": "/", "mouse:272": "Left Click", "mouse:273": "Right Click", "mouse:274": "Middle Click", "SUPER_L": "", "SUPER_R": "", "mouse_down": "Scroll Down", "mouse_up": "Scroll Up" };
        if (k in map)
            return map[k];
        if (k.startsWith("XF86"))
            return k.slice(4).replace(/([a-z])([A-Z])/g, "$1 $2");
        return k.length === 1 ? k.toUpperCase() : k;
    }

    // "Go to space 1" … "Go to space 9" → one row "Go to space 1–9" with keys "1–9"
    function mergeNumbered(binds) {
        const out = [];
        const seen = {};
        for (const b of binds) {
            const m = b.description.match(/^(.*) (\d+)$/);
            const last = b.keys[b.keys.length - 1];
            if (!m || last !== m[2]) {
                out.push(b);
                continue;
            }
            const key = m[1] + "|" + b.keys.slice(0, -1).join("+");
            if (seen[key]) {
                const e = seen[key];
                e.max = Math.max(e.max, +m[2]);
                e.min = Math.min(e.min, +m[2]);
                e.row.description = m[1] + " " + e.min + "–" + e.max;
                e.row.keys = b.keys.slice(0, -1).concat([e.min + "–" + e.max]);
                continue;
            }
            const row = { description: b.description, keys: b.keys.slice() };
            seen[key] = { row: row, min: +m[2], max: +m[2] };
            out.push(row);
        }
        return out;
    }

    Process {
        id: proc
        running: true
        command: ["hyprctl", "-j", "binds"]
        stdout: StdioCollector {
            onStreamFinished: {
                let binds;
                try {
                    binds = JSON.parse(text);
                } catch (e) {
                    return;
                }
                const byGroup = {};
                const order = [];
                for (const b of binds) {
                    const desc = b.description ?? "";
                    if (!desc)
                        continue;
                    const idx = desc.indexOf(": ");
                    const group = idx > 0 ? desc.slice(0, idx) : "General";
                    const what = idx > 0 ? desc.slice(idx + 2) : desc;
                    const keys = root.modNames.filter(([bit]) => b.modmask & bit).map(([, n]) => n);
                    const key = root.prettyKey(b.key ?? "");
                    if (key)
                        keys.push(key);
                    if (!byGroup[group]) {
                        byGroup[group] = [];
                        order.push(group);
                    }
                    if (!byGroup[group].some(x => x.description === what))
                        byGroup[group].push({ keys: keys, description: what });
                }
                root.groups = order.map(g => ({ name: g, binds: root.mergeNumbered(byGroup[g]) }));
            }
        }
    }
}
