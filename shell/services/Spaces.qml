pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Reordering spaces (Overview, Mission Control: drag a space), like macOS:
// a space dropped before another takes that position and the ones between
// shift along by one. Hyprland's spaces keep their numbers, so it's their
// windows that move: in one Lua chunk (hyprctl eval), in order, through
// hidden holding spaces so nothing lands on top of anything mid-way. If you
// were on a space that moved, you follow it; Super+number follows the new
// order by itself.
Singleton {
    id: root

    // order: the spaces as shown (ids, in order); from: the id dragged;
    // to: the id it was dropped on (it takes that one's place)
    function move(order, from, to) {
        const list = order.filter(id => id > 0);
        const a = list.indexOf(from), b = list.indexOf(to);
        if (a < 0 || b < 0 || a === b)
            return;
        const next = list.slice();
        next.splice(a, 1);
        next.splice(b, 0, from);
        // position i keeps its id list[i], and gets next[i]'s windows
        const lo = Math.min(a, b), hi = Math.max(a, b);
        const winsOf = id => Hyprland.toplevels.values.filter(t => (t.workspace?.id ?? 0) === id).map(t => t.address.startsWith("0x") ? t.address : "0x" + t.address);
        const moves = []; // [address, target id]
        for (let i = lo; i <= hi; i++)
            for (const addr of winsOf(next[i]))
                moves.push([addr, list[i]]);
        if (moves.length === 0)
            return;
        const step = (addr, ws) => "hl.dispatch(hl.dsp.window.move({ workspace = " + ws + ", follow = false, window = \"address:" + addr + "\" }))";
        let lua = "";
        for (const [addr] of moves) // everything aside first…
            lua += step(addr, "\"special:irice-reorder\"") + "\n";
        for (const [addr, ws] of moves) // …then into its new place
            lua += step(addr, ws) + "\n";
        // Follow the space you were on
        const here = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 0;
        const i = next.indexOf(here);
        if (here > 0 && i >= lo && i <= hi && list[i] !== here)
            lua += "hl.dispatch(hl.dsp.focus({ workspace = " + list[i] + " }))\n";
        Quickshell.execDetached(["hyprctl", "eval", lua]);
        Qt.callLater(Hyprland.refreshToplevels);
    }
}
