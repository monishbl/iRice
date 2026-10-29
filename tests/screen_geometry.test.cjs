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
