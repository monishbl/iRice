// Battery & Power decisions (shell/services/PowerPolicy.js), run in Node the
// way the shell runs them.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = vm.createContext({});
const src = fs.readFileSync(__dirname + '/../shell/services/PowerPolicy.js', 'utf8').replace(/^\.pragma library\s*/m, '');
vm.runInContext(src, context);
const P = context;
const req = (owner, mode, since, extra = {}) => Object.assign({ owner, mode, since, until: 0, priority: { cooldown: 40, battery: 30, manual: 20, game: 20 }[owner], why: owner }, extra);

test('nothing asked: balanced', () => {
    assert.equal(P.resolve([], 1000).mode, 'balanced');
    assert.equal(P.resolve([], 1000).owner, 'default');
});

test('cool-down beats everything', () => {
    const r = P.resolve([req('manual', 'performance', 5), req('game', 'performance', 9), req('battery', 'power-saver', 7), req('cooldown', 'power-saver', 1)], 10);
    assert.equal(r.owner, 'cooldown');
});

test('battery saver beats your choice and Game Mode', () => {
    assert.equal(P.resolve([req('manual', 'performance', 9), req('battery', 'power-saver', 1)], 10).owner, 'battery');
    assert.equal(P.resolve([req('game', 'performance', 9), req('battery', 'power-saver', 1)], 10).owner, 'battery');
});

test('your choice and Game Mode: the newest wins', () => {
    assert.equal(P.resolve([req('manual', 'balanced', 5), req('game', 'performance', 9)], 10).owner, 'game');
    assert.equal(P.resolve([req('game', 'performance', 5), req('manual', 'power-saver', 9)], 10).owner, 'manual');
});

test('expired requests are gone', () => {
    const r = P.resolve([req('manual', 'balanced', 1), req('game', 'performance', 5, { until: 8 })], 10);
    assert.equal(r.owner, 'manual');
    assert.equal(P.resolve([req('game', 'performance', 5, { until: 20 })], 10).owner, 'game');
});

test('performance without a performance profile is balanced', () => {
    assert.equal(P.fit('performance', false), 'balanced');
    assert.equal(P.fit('performance', true), 'performance');
    assert.equal(P.fit('power-saver', false), 'power-saver');
});

test('critical battery stages only on battery', () => {
    assert.equal(P.batteryStage(50, true), 0);
    assert.equal(P.batteryStage(10, true), 1);
    assert.equal(P.batteryStage(6, true), 1);
    assert.equal(P.batteryStage(5, true), 2);
    assert.equal(P.batteryStage(3, true), 3);
    assert.equal(P.batteryStage(1, true), 3);
    assert.equal(P.batteryStage(2, false), 0);
    assert.equal(P.batteryStage(NaN, true), 0);
});

test('lid: docked on the charger has its own setting', () => {
    const cfg = { lidAc: 'sleep', lidBattery: 'lock', lidExternal: 'nothing' };
    assert.equal(P.lidAction(true, 1, cfg), 'nothing');
    assert.equal(P.lidAction(true, 0, cfg), 'sleep');
    assert.equal(P.lidAction(false, 1, cfg), 'lock'); // on battery, a display doesn't count
    assert.equal(P.lidAction(false, 0, cfg), 'lock');
});

test('the lid is only taken from logind when a setting differs from its default', () => {
    assert.equal(P.lidNeedsInhibit({ lidAc: 'sleep', lidBattery: 'sleep', lidExternal: 'nothing' }), false);
    assert.equal(P.lidNeedsInhibit({ lidAc: 'lock', lidBattery: 'sleep', lidExternal: 'nothing' }), true);
    assert.equal(P.lidNeedsInhibit({ lidAc: 'sleep', lidBattery: 'sleep', lidExternal: 'sleep' }), true);
});

test('idle listeners: per source, in order, never-ones left out', () => {
    const l = P.idleListeners({ idleDimAc: 0, idleLockAc: 300, idleOffAc: 600, idleSleepAc: 0,
        idleDimBattery: 120, idleLockBattery: 60, idleOffBattery: 300, idleSleepBattery: 900 });
    const plain = JSON.parse(JSON.stringify(l));
    assert.deepEqual(plain, [
        { source: 'ac', action: 'lock', timeout: 300 },
        { source: 'ac', action: 'off', timeout: 600 },
        { source: 'battery', action: 'dim', timeout: 120 },
        { source: 'battery', action: 'lock', timeout: 120 }, // never before the dim
        { source: 'battery', action: 'off', timeout: 300 },
        { source: 'battery', action: 'sleep', timeout: 900 },
    ]);
});
