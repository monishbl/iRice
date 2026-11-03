const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = vm.createContext({});
vm.runInContext(fs.readFileSync(__dirname + '/../shell/common/ScreenGeometry.js', 'utf8'), context);
const clip = (...args) => JSON.parse(JSON.stringify(context.clip(...args)));
test('drag across adjacent display remains inside starting screen', () => {
    assert.deepEqual(clip({x: 1500,y: 200,width: 900,height: 1100},1920,1080), {x:1500,y:200,width:420,height:880});
});
test('partially off-screen window is cropped on all edges', () => {
    assert.deepEqual(clip({x:-100,y:-30,width:400,height:200},1920,1080), {x:0,y:0,width:300,height:170});
});
test('logical dimensions work with scaled displays and negative global origins', () => {
    assert.deepEqual(clip({x:1400,y:800,width:500,height:300},1536,864), {x:1400,y:800,width:136,height:64});
});
test('invalid or wholly off-screen selections are empty', () => {
    assert.equal(clip({x:2100,y:0,width:30,height:30},1920,1080).width,0);
    assert.equal(clip({x:NaN,y:0,width:30,height:30},1920,1080).width,0);
});

test('half-pixel edges cannot round into the adjacent monitor', () => {
    const r = context.integerClip({x:1918.5,y:1078.5,width:1.5,height:1.5},1920,1080);
    assert.equal(r.x+r.width,1920);
    assert.equal(r.y+r.height,1080);
});

// Circle to search: strokes
const circle = (cx, cy, r, n = 48, turns = 1) => Array.from({length: n + 1}, (_, i) => ({x: cx + r * Math.cos(2 * Math.PI * turns * i / n), y: cy + r * Math.sin(2 * Math.PI * turns * i / n)}));
const line = (x0, y0, x1, y1, n = 20, wobble = 0) => Array.from({length: n + 1}, (_, i) => ({x: x0 + (x1 - x0) * i / n + (i % 2 ? wobble : -wobble), y: y0 + (y1 - y0) * i / n}));
const box = (...args) => JSON.parse(JSON.stringify(context.strokeBox(...args)));

test('a straight (or slightly shaky) drag is a box from end to end', () => {
    assert.equal(context.strokeKind(line(100, 100, 400, 300)), 'box');
    assert.equal(context.strokeKind(line(100, 100, 400, 300, 30, 5)), 'box');
    assert.deepEqual(box(line(400, 300, 100, 100), 12, 1920, 1080), {x: 100, y: 100, width: 300, height: 200});
});

test('a circle, an open loop or a scribble is a shape: its bounding box, padded', () => {
    assert.equal(context.strokeKind(circle(500, 400, 100)), 'circle');
    assert.equal(context.strokeKind(circle(500, 400, 100, 40, 0.8)), 'circle'); // (not quite closed)
    const scribble = [{x: 100, y: 100}, {x: 300, y: 120}, {x: 110, y: 160}, {x: 310, y: 190}, {x: 120, y: 230}];
    assert.equal(context.strokeKind(scribble), 'circle');
    assert.deepEqual(box(circle(500, 400, 100), 12, 1920, 1080), {x: 388, y: 288, width: 224, height: 224});
    // An L along two edges of a rectangle: the rectangle
    const ell = line(100, 100, 400, 100).concat(line(400, 100, 400, 300));
    assert.equal(context.strokeKind(ell), 'circle');
    assert.deepEqual(box(ell, 0, 1920, 1080), {x: 100, y: 100, width: 300, height: 200});
});

test('a shape padded past the edge stays on its screen', () => {
    assert.deepEqual(box(circle(30, 1060, 40), 12, 1920, 1080), {x: 0, y: 1008, width: 82, height: 72});
    assert.deepEqual(box(circle(1900, 20, 50), 12, 1920, 1080), {x: 1838, y: 0, width: 82, height: 82});
});

test('a click or a tiny wiggle selects nothing', () => {
    assert.equal(context.strokeKind([]), 'none');
    assert.equal(context.strokeKind([{x: 5, y: 5}]), 'none');
    assert.equal(context.strokeKind([{x: 5, y: 5}, {x: 8, y: 7}, {x: 6, y: 9}]), 'none');
    assert.deepEqual(box([{x: 5, y: 5}, {x: 7, y: 6}], 12, 1920, 1080), {x: 0, y: 0, width: 0, height: 0});
});
