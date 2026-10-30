.pragma library
// Each display has its own spaces, numbered from 1 (like macOS). Hyprland's
// workspace ids are global, so every display owns a block of 100 ids: its
// "base" (the built-in panel 0 → ids 1..99, the first external 100 →
// 101..199, the next 200 → 201..299 …). Space n on a display is id base + n,
// and an id's display is the block it falls in. Blocks of 100 rather than 10
// keep Super+0 (space 10) and any number of spaces per display apart from the
// next display's, and the id still reads as display + number (103 = the
// first external's space 3). Bases are kept per display identity (its
// description: make, model, serial), not the connector, so a monitor gets its
// spaces back whichever port it's plugged into.
// Pure logic, tested in Node (tests/space_map.test.cjs); Spaces.qml applies it.

var STRIDE = 100;

// The laptop's own panel (in the nested sandbox, its test output plays it)
function isBuiltin(name, sandbox) {
    return /^(eDP|LVDS|DSI)-/.test(name || "") || (!!sandbox && name === "IRICE-TEST");
}

// A display's identity: its description (make, model, serial), else its
// connector (virtual and headless outputs have no description)
function keyOf(m) {
    var d = String((m && m.description) || "").trim();
    return d !== "" ? "desc:" + d : "name:" + String((m && m.name) || "");
}

// A short name for marks and rows: "Built-in", else the model ("U2720Q"),
// else the first words of the description, else the connector
function labelOf(m, sandbox) {
    if (!m)
        return "";
    if (isBuiltin(m.name, sandbox))
        return "Built-in";
    var model = String(m.model || "").trim();
    if (model !== "" && !/^0x/i.test(model))
        return model.length > 18 ? model.slice(0, 18) : model;
    var d = String(m.description || "").trim();
    if (d !== "")
        return d.split(/\s+/).slice(0, 2).join(" ");
    return String(m.name || "");
}

function baseOfId(id) {
    return Math.floor(id / STRIDE) * STRIDE;
}
function numberOf(id) {
    return id - baseOfId(id);
}

// displays: { key: { base, label, name } } as kept; monitors: the connected
// ones ({ name, description, model, x, y }), any order. Returns a new map
// with a base for each (the built-in gets 0; with no built-in connected and 0
// unused, the first display in the arrangement does; others the next free
// block), labels and connector names brought up to date.
function assign(displays, monitors, sandbox) {
    var next = {};
    var k;
    for (k in displays)
        next[k] = { base: displays[k].base, label: displays[k].label, name: displays[k].name };
    var list = monitors.slice().sort(function (a, b) {
        var ba = isBuiltin(a.name, sandbox) ? 0 : 1, bb = isBuiltin(b.name, sandbox) ? 0 : 1;
        return ba - bb || (a.x || 0) - (b.x || 0) || (a.y || 0) - (b.y || 0);
    });
    var builtinHere = list.some(function (m) { return isBuiltin(m.name, sandbox); });
    function used(base) {
        for (var key in next)
            if (next[key].base === base)
                return true;
        return false;
    }
    for (var i = 0; i < list.length; i++) {
        var m = list[i];
        k = keyOf(m);
        if (!next[k]) {
            var base;
            if (!used(0) && (isBuiltin(m.name, sandbox) || !builtinHere))
                base = 0;
            else {
                base = STRIDE;
                while (used(base))
                    base += STRIDE;
            }
            next[k] = { base: base };
        }
        next[k].label = labelOf(m, sandbox);
        next[k].name = m.name;
    }
    return next;
}

// The connected displays' bases by connector: { name: base }
function basesByName(displays, monitors) {
    var out = {};
    for (var i = 0; i < monitors.length; i++) {
        var d = displays[keyOf(monitors[i])];
        if (d)
            out[monitors[i].name] = d.base;
    }
    return out;
}

// The first free number on a display (base), given every id in use
function nextFree(base, ids) {
    var n = 1;
    while (n < STRIDE - 1 && ids.indexOf(base + n) >= 0)
        n++;
    return base + n;
}

// What one display lists, in groups:
//   own    its spaces (1..count always, every one in use, no gaps)
//   guest  spaces of a display that isn't connected, living here for now
//          (one group per display they belong to)
//   other  (with others) the spaces of the other connected displays
// ctx: { name, bases: { name: base }, workspaces: [{ id, monitor }],
//        active: { name: id }, occupied: [ids], count, others,
//        labels: { base: label } }
// Returns [{ kind, base, label, ids }]
function groups(ctx) {
    var base = ctx.bases[ctx.name];
    if (base === undefined)
        base = 0;
    var connected = {};
    for (var n in ctx.bases)
        connected[ctx.bases[n]] = n;
    var own = {}, guests = {}, others = {};
    var i, id, b;
    for (i = 1; i <= (ctx.count || 0); i++)
        own[base + i] = true;
    var act = ctx.active[ctx.name];
    if (act > 0 && baseOfId(act) === base)
        own[act] = true;
    for (i = 0; i < ctx.workspaces.length; i++) {
        var w = ctx.workspaces[i];
        id = w.id;
        if (!(id > 0))
            continue;
        b = baseOfId(id);
        if (b === base)
            own[id] = true;
        else if (w.monitor === ctx.name && connected[b] === undefined)
            (guests[b] = guests[b] || {})[id] = true;
        else if (ctx.others && connected[b] !== undefined)
            (others[b] = others[b] || {})[id] = true;
    }
    // no gaps: an empty space between two in use stays reachable
    var ownIds = Object.keys(own).map(Number);
    var top = Math.max.apply(null, ownIds.concat([base + 1]));
    for (id = base + 1; id < top; id++)
        own[id] = true;
    var sorted = function (set) {
        return Object.keys(set).map(Number).sort(function (p, q) { return p - q; });
    };
    var labels = ctx.labels || {};
    var out = [{ kind: "own", base: base, label: labels[base] || "", ids: sorted(own) }];
    var keys = Object.keys(guests).map(Number).sort(function (p, q) { return p - q; });
    for (i = 0; i < keys.length; i++)
        out.push({ kind: "guest", base: keys[i], label: labels[keys[i]] || "Display " + (keys[i] / STRIDE + 1), ids: sorted(guests[keys[i]]) });
    keys = Object.keys(others).map(Number).sort(function (p, q) { return p - q; });
    for (i = 0; i < keys.length; i++)
        out.push({ kind: "other", base: keys[i], label: labels[keys[i]] || "", ids: sorted(others[keys[i]]) });
    return out;
}

// What to change so every space sits on the display it belongs to.
// ctx: { bases: { name: base } (connected displays), workspaces: [{ id,
//        monitor, windows }], active: { name: id }, known: [ids seen
//        before], migrated, returns (move spaces home now) }
// Returns [{ op: "renumber", from, to } | { op: "move", id, monitor }]:
//   - first run (not migrated): spaces on an external display that aren't
//     in its block are renumbered into it, windows and all (setups from
//     before per-display spaces keep their spaces where they are)
//   - a space whose display is connected but which is elsewhere goes home
//     (a display came back: its spaces return) when returns is set
//   - a new, empty space a display shows that isn't from its block (what
//     Hyprland gives a display that just connected) is renumbered into it
//   - spaces of a display that isn't connected stay where they are (guests)
function plan(ctx) {
    var ops = [];
    var connected = {};
    var name;
    for (name in ctx.bases)
        connected[ctx.bases[name]] = name;
    var ids = ctx.workspaces.map(function (w) { return w.id; }).filter(function (id) { return id > 0; });
    var known = ctx.known || [];
    for (var i = 0; i < ctx.workspaces.length; i++) {
        var w = ctx.workspaces[i];
        if (!(w.id > 0) || ctx.bases[w.monitor] === undefined)
            continue;
        var here = ctx.bases[w.monitor];
        var b = baseOfId(w.id);
        if (b === here)
            continue;
        var to;
        if (!ctx.migrated && here !== 0) {
            to = nextFree(here, ids);
            ids.push(to);
            ops.push({ op: "renumber", from: w.id, to: to });
            continue;
        }
        if (connected[b] === undefined)
            continue; // a guest
        var fresh = known.indexOf(w.id) < 0;
        if (fresh && !(w.windows > 0) && ctx.active[w.monitor] === w.id) {
            to = nextFree(here, ids);
            ids.push(to);
            ops.push({ op: "renumber", from: w.id, to: to });
        } else if (ctx.returns) {
            ops.push({ op: "move", id: w.id, monitor: connected[b] });
        }
    }
    return ops;
}
