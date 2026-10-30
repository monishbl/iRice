// Spaces per display (shell/services/SpaceMap.js)
const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

function load(file) {
    const src = fs.readFileSync(path.join(__dirname, "..", "shell", "services", file), "utf8").replace(/^\.pragma library\s*$/m, "");
    const ctx = {};
    vm.runInNewContext(src, ctx);
    return ctx;
}
const M = load("SpaceMap.js");
const plain = v => JSON.parse(JSON.stringify(v));

const laptop = { name: "eDP-1", description: "BOE 0x0BCA", model: "0x0BCA", x: 0, y: 0 };
const dell = { name: "DP-1", description: "Dell Inc. DELL U2720Q 8LXMZ13", model: "DELL U2720Q", x: 1920, y: 0 };
const lg = { name: "HDMI-A-1", description: "LG Electronics LG ULTRAFINE 123", model: "LG ULTRAFINE", x: 4480, y: 0 };

test("the built-in panel gets 0, externals the next free block, kept by identity", () => {
    let d = M.assign({}, [dell, laptop], false);
    assert.strictEqual(d[M.keyOf(laptop)].base, 0);
    assert.strictEqual(d[M.keyOf(dell)].base, 100);
    assert.strictEqual(d[M.keyOf(laptop)].label, "Built-in");
    assert.strictEqual(d[M.keyOf(dell)].label, "DELL U2720Q");
    // the Dell on another port keeps its block; a new display takes the next
    d = M.assign(d, [laptop, Object.assign({}, dell, { name: "DP-2" }), lg], false);
    assert.strictEqual(d[M.keyOf(dell)].base, 100);
    assert.strictEqual(d[M.keyOf(dell)].name, "DP-2");
    assert.strictEqual(d[M.keyOf(lg)].base, 200);
    // unplugged, it keeps it for when it's back
    d = M.assign(d, [laptop], false);
    assert.strictEqual(d[M.keyOf(lg)].base, 200);
});

test("without a built-in panel the first display in the arrangement counts from 1", () => {
    const d = M.assign({}, [lg, dell], false);
    assert.strictEqual(d[M.keyOf(dell)].base, 0);
    assert.strictEqual(d[M.keyOf(lg)].base, 100);
});

test("ids and numbers", () => {
    assert.strictEqual(M.numberOf(3), 3);
    assert.strictEqual(M.numberOf(103), 3);
    assert.strictEqual(M.numberOf(110), 10);
    assert.strictEqual(M.baseOfId(110), 100);
    assert.strictEqual(M.nextFree(100, [101, 102, 104]), 103);
    assert.strictEqual(M.nextFree(0, [2, 3]), 1);
});

test("a display lists its own spaces from 1, no gaps, then guests, then others", () => {
    const g = plain(M.groups({
        name: "DP-1",
        bases: { "eDP-1": 0, "DP-1": 100 },
        workspaces: [{ id: 1, monitor: "eDP-1" }, { id: 101, monitor: "DP-1" }, { id: 104, monitor: "DP-1" }, { id: 201, monitor: "DP-1" }],
        active: { "eDP-1": 1, "DP-1": 104 },
        count: 2, others: false, labels: { 0: "Built-in", 100: "DELL", 200: "LG" }
    }));
    assert.deepStrictEqual(g.map(x => [x.kind, x.ids]), [["own", [101, 102, 103, 104]], ["guest", [201]]]);
    assert.strictEqual(g[1].label, "LG");
    const o = plain(M.groups({
        name: "eDP-1",
        bases: { "eDP-1": 0, "DP-1": 100 },
        workspaces: [{ id: 1, monitor: "eDP-1" }, { id: 101, monitor: "DP-1" }, { id: 201, monitor: "DP-1" }],
        active: { "eDP-1": 1, "DP-1": 101 },
        count: 3, others: true, labels: {}
    }));
    // the LG's space stays on the Dell: it isn't the laptop's guest
    assert.deepStrictEqual(o.map(x => [x.kind, x.ids]), [["own", [1, 2, 3]], ["other", [101]]]);
});

test("the external display's spaces are listed even when Hyprland's monitor objects were stale", () => {
    // (the bug: only the current space showed on the external display)
    const g = plain(M.groups({
        name: "DP-1",
        bases: { "eDP-1": 0, "DP-1": 100 },
        workspaces: [{ id: 101, monitor: "DP-1" }, { id: 102, monitor: "DP-1" }, { id: 105, monitor: "DP-1" }],
        active: { "DP-1": 102 }, count: 1, others: false, labels: {}
    }));
    assert.deepStrictEqual(g[0].ids, [101, 102, 103, 104, 105]);
});

test("first run: spaces on an external display are renumbered into its block, windows and all", () => {
    const ops = plain(M.plan({
        bases: { "eDP-1": 0, "DP-1": 100 },
        workspaces: [{ id: 1, monitor: "DP-1", windows: 2 }, { id: 2, monitor: "eDP-1", windows: 1 }, { id: 5, monitor: "DP-1", windows: 1 }],
        active: { "eDP-1": 2, "DP-1": 1 }, known: [1, 2, 5], migrated: false, returns: true
    }));
    assert.deepStrictEqual(ops, [{ op: "renumber", from: 1, to: 101 }, { op: "renumber", from: 5, to: 102 }]);
});

test("unplugged: its spaces stay on the laptop as they are", () => {
    const ops = M.plan({
        bases: { "eDP-1": 0 },
        workspaces: [{ id: 1, monitor: "eDP-1", windows: 1 }, { id: 101, monitor: "eDP-1", windows: 2 }, { id: 102, monitor: "eDP-1", windows: 1 }],
        active: { "eDP-1": 1 }, known: [1, 101, 102], migrated: true, returns: true
    });
    assert.strictEqual(ops.length, 0);
});

test("plugged back in: the new empty space Hyprland gave it is renumbered, its spaces go home when asked", () => {
    const ctx = {
        bases: { "eDP-1": 0, "DP-1": 100 },
        workspaces: [{ id: 1, monitor: "eDP-1", windows: 1 }, { id: 101, monitor: "eDP-1", windows: 2 }, { id: 102, monitor: "eDP-1", windows: 1 }, { id: 3, monitor: "DP-1", windows: 0 }],
        active: { "eDP-1": 1, "DP-1": 3 }, known: [1, 101, 102], migrated: true, returns: false
    };
    assert.deepStrictEqual(plain(M.plan(ctx)), [{ op: "renumber", from: 3, to: 103 }]);
    ctx.returns = true;
    assert.deepStrictEqual(plain(M.plan(ctx)), [
        { op: "move", id: 101, monitor: "DP-1" },
        { op: "move", id: 102, monitor: "DP-1" },
        { op: "renumber", from: 3, to: 103 }
    ]);
});

test("the sandbox's test output plays the built-in panel", () => {
    assert.strictEqual(M.isBuiltin("IRICE-TEST", true), true);
    assert.strictEqual(M.isBuiltin("IRICE-TEST", false), false);
    assert.strictEqual(M.isBuiltin("eDP-1", false), true);
    assert.strictEqual(M.keyOf({ name: "DP-9", description: "" }), "name:DP-9");
});

test("drop zones sit on the edge that faces each other display", () => {
    const lap = { x: 0, y: 0, w: 1920, h: 1080 };
    assert.strictEqual(M.sideOf(lap, { x: 1920, y: 0, w: 2560, h: 1440 }), "right");
    assert.strictEqual(M.sideOf(lap, { x: -2560, y: -200, w: 2560, h: 1440 }), "left");
    assert.strictEqual(M.sideOf(lap, { x: 0, y: -1440, w: 2560, h: 1440 }), "top");
    assert.strictEqual(M.sideOf(lap, { x: 200, y: 1080, w: 1920, h: 1080 }), "bottom");
    // diagonal: the larger relative gap wins
    assert.strictEqual(M.sideOf(lap, { x: 1920, y: 1080, w: 1920, h: 200 }), "right");
    assert.strictEqual(M.sideOf(lap, { x: 1800, y: -1080, w: 400, h: 1080 }), "top");
    // the sandbox's far-off headless output
    assert.strictEqual(M.sideOf({ x: 20000, y: 20000, w: 1920, h: 1080 }, { x: 21920, y: 20000, w: 1920, h: 1080 }), "right");
    const z = plain(M.edgeZones(lap, [
        { name: "B", x: 1920, y: 540, w: 1920, h: 1080 },
        { name: "A", x: 1920, y: -900, w: 1920, h: 1080 },
        { name: "T", x: 0, y: -1080, w: 1000, h: 1080 }
    ]));
    assert.deepStrictEqual(z, [
        { name: "A", side: "right", index: 0, count: 2 },
        { name: "B", side: "right", index: 1, count: 2 },
        { name: "T", side: "top", index: 0, count: 1 }
    ]);
});
