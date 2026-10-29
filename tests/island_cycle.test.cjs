// Scrolling over the island to switch its live activity (shell/services/IslandCycle.js)
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
const C = load("IslandCycle.js");

function run(events) {
    let state = null;
    const steps = [];
    for (const [dx, dy, t] of events) {
        const r = C.wheel(state, dx, dy, t);
        state = r.state;
        if (r.step)
            steps.push(r.step);
    }
    return steps;
}

test("a wheel notch down moves on, up moves back", () => {
    assert.deepStrictEqual(run([[0, -120, 1000]]), [1]);
    assert.deepStrictEqual(run([[0, 120, 1000]]), [-1]);
});

test("notches in a quick row switch once per lock, then again", () => {
    assert.deepStrictEqual(run([[0, -120, 1000], [0, -120, 1100], [0, -120, 1400]]), [1, 1]);
});

test("a touchpad swipe of many small deltas is one step, momentum included", () => {
    const ev = [];
    for (let i = 0; i < 60; i++)
        ev.push([0, -15, 1000 + i * 16]);
    assert.deepStrictEqual(run(ev), [1]);
});

test("small deltas below a notch do nothing", () => {
    assert.deepStrictEqual(run([[0, -30, 1000], [0, -30, 1016], [0, -30, 1032]]), []);
});

test("a new swipe after a pause switches again", () => {
    const ev = [];
    for (let i = 0; i < 20; i++)
        ev.push([0, -15, 1000 + i * 16]);
    for (let i = 0; i < 20; i++)
        ev.push([0, -15, 2000 + i * 16]);
    assert.deepStrictEqual(run(ev), [1, 1]);
});

test("horizontal swipes: left moves on, right moves back", () => {
    const left = [], right = [];
    for (let i = 0; i < 12; i++) {
        left.push([-15, 2, 1000 + i * 16]);
        right.push([15, -2, 1000 + i * 16]);
    }
    assert.deepStrictEqual(run(left), [1]);
    assert.deepStrictEqual(run(right), [-1]);
});

test("changing direction mid-gesture starts the count over", () => {
    assert.deepStrictEqual(run([[0, -100, 1000], [0, 100, 1016], [0, 60, 1032]]), [-1]);
});
