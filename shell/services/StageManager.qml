pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common

// Stage Manager: each space shows one stage (a group of windows) at a time;
// the others wait as thumbnails in a strip on the left. A new app opens in a
// new stage; more windows of the same app join it. Hidden stages park their
// windows on the special workspace "stage". Focusing a parked window (dock,
// Mission Control) brings its stage forward.
Singleton {
    id: root

    readonly property bool enabled: Config.options.stageManager.enabled
    readonly property string park: "special:stage"

    // wsId -> [{ id, wins: [address] }], most recently used first
    property var stages: ({})
    // wsId -> stage id shown on that space
    property var active: ({})
    property int nextId: 1

    // Addresses are kept without "0x", as Hyprland's events send them
    function key(addr) {
        return (addr ?? "").replace(/^0x/, "");
    }
    function sel(addr) {
        return "address:0x" + key(addr);
    }
    // Our own moves shift focus around; ignore focus events until they settle
    property real quietUntil: 0

    function moveWin(addr, ws) {
        quietUntil = Date.now() + 500;
        if (typeof ws === "string" && /^\d+$/.test(ws))
            ws = Number(ws); // object keys come back as strings
        Hyprland.dispatch("hl.dsp.window.move({ workspace = " + (typeof ws === "string" ? JSON.stringify(ws) : ws) + ", follow = false, window = \"" + sel(addr) + "\" })");
    }
    function classOf(t) {
        return t?.lastIpcObject?.class ?? t?.wayland?.appId ?? "";
    }

    // Stages for a space, other than the one shown: what the strip lists
    function waiting(wsId) {
        return (stages[wsId] ?? []).filter(s => s.id !== active[wsId]);
    }
    function toplevel(addr) {
        return Hyprland.toplevels.values.find(t => key(t.address) === key(addr)) ?? null;
    }

    function setStages(wsId, list) {
        const s = Object.assign({}, stages);
        s[wsId] = list;
        stages = s;
    }
    function setActive(wsId, id) {
        const a = Object.assign({}, active);
        a[wsId] = id;
        active = a;
    }

    // Show stage `id` on space `wsId`; park the current one
    function show(wsId, id, focusAddr) {
        const list = stages[wsId] ?? [];
        const target = list.find(s => s.id === id);
        if (!target)
            return;
        const cur = list.find(s => s.id === active[wsId]);
        if (cur && cur.id !== id)
            for (const a of cur.wins)
                moveWin(a, park);
        for (const a of target.wins)
            moveWin(a, wsId);
        setActive(wsId, id);
        setStages(wsId, [target].concat(list.filter(s => s !== target)));
        const f = focusAddr ?? target.wins[target.wins.length - 1];
        if (f)
            Qt.callLater(() => Hyprland.dispatch("hl.dsp.focus({ window = \"" + sel(f) + "\" })"));
    }

    // Bring another stage's windows into the one shown (Shift+click in the strip)
    function merge(wsId, id) {
        const list = stages[wsId] ?? [];
        const from = list.find(s => s.id === id), into = list.find(s => s.id === active[wsId]);
        if (!from || !into)
            return show(wsId, id);
        for (const a of from.wins)
            moveWin(a, wsId);
        into.wins = into.wins.concat(from.wins);
        setStages(wsId, list.filter(s => s !== from));
    }

    function wsOfStage(addr) {
        for (const [ws, list] of Object.entries(stages))
            if (list.some(s => s.wins.includes(addr)))
                return Number(ws);
        return null;
    }

    // Bring stages in line with the windows that exist now
    function reconcile(openedAddr) {
        if (!enabled)
            return;
        const tops = Hyprland.toplevels.values;
        const alive = new Set(tops.map(t => key(t.address)));
        const next = {};
        for (const [ws, list] of Object.entries(stages))
            next[ws] = list.map(s => ({ id: s.id, wins: s.wins.filter(a => alive.has(a)) })).filter(s => s.wins.length > 0);
        const known = new Map();
        for (const [ws, list] of Object.entries(next))
            for (const s of list)
                for (const a of s.wins)
                    known.set(a, Number(ws));
        const act = Object.assign({}, active);
        let switchTo = null;

        for (const t of tops) {
            const ws = t.workspace;
            if (!ws)
                continue;
            const parked = ws.name === park;
            if (ws.id < 0 && !parked)
                continue; // other special workspaces (minimized, scratchpads)
            const home = known.get(key(t.address));
            if (parked) {
                if (home === undefined) {
                    // Parked before a shell restart: give it a stage on the focused space
                    const fw = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 1;
                    (next[fw] = next[fw] ?? []).push({ id: nextId++, wins: [key(t.address)] });
                }
                continue;
            }
            if (home === ws.id)
                continue;
            const list = (next[ws.id] = next[ws.id] ?? []);
            if (home !== undefined) {
                // Moved to another space by hand: leave its old stage
                for (const s of next[home] ?? [])
                    s.wins = s.wins.filter(a => a !== key(t.address));
                next[home] = (next[home] ?? []).filter(s => s.wins.length > 0);
            }
            const cur = list.find(s => s.id === act[ws.id]);
            const sameApp = cur && cur.wins.some(a => classOf(toplevel(a)) === classOf(t));
            if (cur && (sameApp || key(t.address) !== openedAddr)) {
                cur.wins.push(key(t.address));
            } else {
                const s = { id: nextId++, wins: [key(t.address)] };
                list.unshift(s);
                if (cur && key(t.address) === openedAddr)
                    switchTo = { ws: ws.id, prev: cur };
                act[ws.id] = s.id;
            }
            known.set(key(t.address), ws.id);
        }
        for (const ws of Object.keys(act))
            if (!(next[ws] ?? []).some(s => s.id === act[ws]))
                act[ws] = (next[ws] ?? [])[0]?.id ?? null;
        stages = next;
        active = act;
        // A new app took over the space: park the stage it replaced
        if (switchTo)
            for (const a of switchTo.prev.wins)
                moveWin(a, park);
    }

    // Turning it on groups each space's windows by app and keeps the focused one
    function start() {
        stages = {};
        active = {};
        const focused = Apps.activeHyprland;
        const next = {}, act = {};
        const focusedWs = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 1;
        for (const t of Hyprland.toplevels.values) {
            const ws = t.workspace;
            // Windows still parked from before a restart join the focused space
            const wsId = ws?.name === park ? focusedWs : ws?.id ?? -1;
            if (wsId < 0)
                continue;
            const list = (next[wsId] = next[wsId] ?? []);
            let s = list.find(s => classOf(toplevel(s.wins[0])) === classOf(t));
            if (!s)
                list.push(s = { id: nextId++, wins: [] });
            s.wins.push(key(t.address));
            if (t === focused && ws.name !== park)
                act[wsId] = s.id;
        }
        for (const ws of Object.keys(next)) {
            act[ws] = act[ws] ?? next[ws][0].id;
            next[ws].sort((a, b) => (b.id === act[ws]) - (a.id === act[ws]));
            for (const s of next[ws])
                for (const a of s.wins) {
                    const parked = toplevel(a)?.workspace?.name === park;
                    if (s.id !== act[ws] && !parked)
                        moveWin(a, park);
                    else if (s.id === act[ws] && parked)
                        moveWin(a, ws);
                }
        }
        stages = next;
        active = act;
    }

    // Turning it off brings every parked window back to its space
    function stop() {
        for (const [ws, list] of Object.entries(stages))
            for (const s of list)
                if (s.id !== active[ws])
                    for (const a of s.wins)
                        moveWin(a, Number(ws));
        for (const t of Hyprland.toplevels.values)
            if (t.workspace?.name === park && wsOfStage(key(t.address)) === null)
                moveWin(key(t.address), Hyprland.focusedMonitor?.activeWorkspace?.id ?? 1);
        stages = {};
        active = {};
    }

    onEnabledChanged: enabled ? startTimer.restart() : stop()
    Component.onCompleted: if (enabled) startTimer.restart()
    // Window info (workspaces, classes) arrives a moment after the shell starts
    Timer {
        id: startTimer
        interval: 600
        onTriggered: {
            Hyprland.refreshToplevels();
            if (Hyprland.toplevels.values.some(t => !t.workspace || !t.lastIpcObject?.class))
                restart();
            else
                root.start();
        }
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (!root.enabled)
                return;
            if (event.name === "openwindow") {
                root.pendingOpen = event.data.split(",")[0];
                refresh.restart();
            } else if (event.name === "closewindow" || (["movewindow", "movewindowv2"].includes(event.name) && Date.now() > root.quietUntil)) {
                refresh.restart();
            } else if (event.name === "activewindowv2" && Date.now() > root.quietUntil) {
                // Focused a parked window: bring its stage forward
                const addr = event.data;
                const t = root.toplevel(addr);
                if (t?.workspace?.name === root.park) {
                    const ws = root.wsOfStage(addr);
                    const s = (root.stages[ws] ?? []).find(s => s.wins.includes(addr));
                    if (s)
                        root.show(ws, s.id, addr);
                }
            }
        }
    }
    property string pendingOpen: ""
    Timer {
        id: refresh
        interval: 150 // toplevel workspace info arrives after the event
        onTriggered: {
            Hyprland.refreshToplevels();
            Qt.callLater(() => {
                root.reconcile(root.pendingOpen);
                root.pendingOpen = "";
            });
        }
    }
}
