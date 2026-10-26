pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common

// Desktop entries joined with open windows: names, icons, launching, and the
// dock's list of pinned + running apps.
Singleton {
    id: root

    readonly property var active: ToplevelManager.activeToplevel
    // The focused window as a HyprlandToplevel. Hyprland.activeToplevel stays
    // empty until focus changes after the shell starts, so match the
    // Wayland-level active window instead.
    readonly property var activeHyprland: Hyprland.toplevels.values.find(t => t.wayland && t.wayland === active) ?? Hyprland.activeToplevel
    readonly property var activeEntry: {
        DesktopEntries.applications.values; // re-evaluate once entries load
        return active ? entryFor(active.appId) : null;
    }
    readonly property string activeName: active?.appId === "org.quickshell" ? "System Settings" : activeEntry?.name ?? (active?.appId || "Desktop")

    function entryFor(appId) {
        if (!appId)
            return null;
        return DesktopEntries.byId(appId) ?? DesktopEntries.heuristicLookup(appId);
    }

    function iconFor(entryOrId) {
        if (entryOrId === "org.quickshell" || entryOrId?.id === "org.quickshell")
            return IconLookup.path("preferences-system", "application-x-executable");
        const entry = typeof entryOrId === "string" ? entryFor(entryOrId) : entryOrId;
        const name = entry?.icon || (typeof entryOrId === "string" ? entryOrId : "");
        if (name.startsWith("/")) // some desktop files give a file path
            return "file://" + name;
        return IconLookup.path(name, "application-x-executable");
    }

    function windowsOf(entryId) {
        return ToplevelManager.toplevels.values.filter(t => entryFor(t.appId)?.id === entryId || t.appId === entryId);
    }

    // [{ id, entry, windows, pinned }] — pinned first in config order, then
    // running unpinned apps in the order they were opened.
    readonly property var dockApps: {
        DesktopEntries.applications.values; // re-evaluate once entries (re)load
        const pinned = Config.options.dock.pinned;
        const out = [];
        const seen = new Set();
        for (const id of pinned) {
            const entry = DesktopEntries.byId(id);
            if (!entry)
                continue;
            seen.add(entry.id);
            out.push({ id: entry.id, entry: entry, pinned: true });
        }
        for (const t of ToplevelManager.toplevels.values) {
            const entry = entryFor(t.appId);
            const id = entry?.id ?? t.appId;
            if (!id || seen.has(id))
                continue;
            seen.add(id);
            out.push({ id: id, entry: entry, pinned: false });
        }
        return out;
    }

    // Runs a Hyprland window dispatcher on the focused window, e.g.
    // windowAction("float", { action: "toggle" }).
    function windowAction(name, args) {
        windowActionFor(activeHyprland, name, args);
    }

    // Same, for a given HyprlandToplevel.
    function windowActionFor(toplevel, name, args) {
        const addr = toplevel?.address;
        if (!addr)
            return;
        const fields = Object.assign({}, args ?? {}, { window: "address:" + (addr.startsWith("0x") ? addr : "0x" + addr) });
        const lua = Object.entries(fields).map(([k, v]) => k + " = " + (typeof v === "string" ? JSON.stringify(v) : v)).join(", ");
        Hyprland.dispatch("hl.dsp.window." + name + "({ " + lua + " })");
    }

    // Minimize: Hyprland has none, so windows park on a hidden special
    // workspace; the dock lists them and restores them on click.
    readonly property var minimized: Windows.minimized
    function minimize(toplevel) {
        windowActionFor(toplevel ?? activeHyprland, "move", { workspace: "special:minimized", follow: false });
    }
    function restore(toplevel) {
        const ws = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 1;
        windowActionFor(toplevel, "move", { workspace: ws, follow: true });
        Qt.callLater(() => toplevel?.wayland?.activate());
    }

    // Recently used apps (for the dock's Recent section), most recent first
    onActiveEntryChanged: {
        const id = activeEntry?.id;
        if (!id || id === "org.quickshell" || UiState.data.recentApps[0] === id)
            return;
        const r = UiState.data.recentApps.filter(x => x !== id);
        UiState.data.recentApps = [id].concat(r).slice(0, 12);
    }
    function recentFor(limit) {
        const pinned = Config.options.dock.pinned;
        const running = new Set(ToplevelManager.toplevels.values.map(t => entryFor(t.appId)?.id ?? t.appId));
        return UiState.data.recentApps.filter(id => !pinned.includes(id) && !running.has(id) && DesktopEntries.byId(id)).slice(0, limit).map(id => ({ id: id, entry: DesktopEntries.byId(id), pinned: false, recent: true }));
    }

    function pin(id) {
        if (!Config.options.dock.pinned.includes(id))
            Config.options.dock.pinned = Config.options.dock.pinned.concat([id]);
    }
    function unpin(id) {
        Config.options.dock.pinned = Config.options.dock.pinned.filter(p => p !== id);
    }

    function quit(appId) {
        for (const t of windowsOf(entryFor(appId)?.id ?? appId))
            t.close();
    }

    function launch(entry) {
        Launch.app(entry);
    }

    // Window tabs: the focused window becomes a tab group, and the same
    // app's other windows on this space join it (each steps toward the
    // group until it's in; Hyprland merges by direction)
    function mergeIntoTabs() {
        const t = activeHyprland;
        const o = t?.lastIpcObject;
        if (!o)
            return;
        const sel = a => "address:0x" + a.replace(/^0x/, "");
        if ((o.grouped?.length ?? 0) === 0)
            Hyprland.dispatch("hl.dsp.group.toggle()");
        const others = Hyprland.toplevels.values.filter(w => w !== t && w.workspace === t.workspace && w.lastIpcObject?.class === o.class && (w.lastIpcObject?.grouped?.length ?? 0) === 0);
        const cx = o.at[0] + o.size[0] / 2, cy = o.at[1] + o.size[1] / 2;
        for (const w of others) {
            const p = w.lastIpcObject;
            const dx = cx - (p.at[0] + p.size[0] / 2), dy = cy - (p.at[1] + p.size[1] / 2);
            const dir = Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "r" : "l") : (dy > 0 ? "d" : "u");
            Hyprland.dispatch("hl.dsp.focus({ window = \"" + sel(w.address) + "\" })");
            Hyprland.dispatch("hl.dsp.window.move({ into_group = \"" + dir + "\" })");
        }
        Hyprland.dispatch("hl.dsp.focus({ window = \"" + sel(t.address) + "\" })");
    }

    // Focus the app's most recent window, or launch it.
    function activateOrLaunch(app) {
        const windows = windowsOf(app.id);
        if (windows.length > 0)
            windows[windows.length - 1].activate();
        else
            launch(app.entry);
    }
}
