// Sharing.devices: LocalSend peers and Quick Share devices as one list
// (shell/services/SharingDevices.js), run in Node the way the shell runs it.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = vm.createContext({});
const src = fs.readFileSync(__dirname + '/../shell/services/SharingDevices.js', 'utf8').replace(/^\.pragma library\s*/m, '');
vm.runInContext(src, context);
const plain = x => JSON.parse(JSON.stringify(x));

test('a phone seen both ways is one device, sent to over Quick Share', () => {
    const d = plain(context.merge(
        [{ id: 'ls1', alias: 'Pixel 8', type: 'mobile' }, { id: 'ls2', alias: 'Work Laptop', type: 'laptop' }],
        [{ id: 'qs1', name: 'pixel-8 ', type: 'phone' }]));
    assert.equal(d.length, 2);
    const phone = d.find(x => x.ids.quickshare === 'qs1');
    assert.equal(phone.ids.localsend, 'ls1');
    assert.equal(phone.type, 'mobile');
    assert.equal(phone.prefer, 'quickshare');
    assert.deepEqual(phone.via.sort(), ['localsend', 'quickshare']);
});

test('computers go over LocalSend, a Quick Share-only device over Quick Share', () => {
    const d = plain(context.merge(
        [{ id: 'ls2', alias: 'Work Laptop', type: 'laptop' }],
        [{ id: 'qs9', name: 'Galaxy Tab', type: 'tablet' }, { id: 'qs8', name: 'Desk PC', type: 'desktop' }]));
    assert.equal(d.find(x => x.name === 'Work Laptop').prefer, 'localsend');
    assert.equal(d.find(x => x.name === 'Galaxy Tab').prefer, 'quickshare');
    assert.equal(d.find(x => x.name === 'Desk PC').prefer, 'quickshare');
});

test('different devices with different names stay apart; nameless ones never merge', () => {
    const d = plain(context.merge([{ id: 'a', alias: '', type: 'mobile' }, { id: 'b', alias: '', type: 'mobile' }], [{ id: 'c', name: 'Pixel 7', type: 'phone' }]));
    assert.equal(d.length, 3);
    assert.ok(d.every(x => x.name));
});

test('progress is found by the device name, outgoing only', () => {
    const items = [{ kind: 'receive', peer: 'Pixel 8', done: 1, total: 2 }, { kind: 'send', peer: 'PIXEL 8', done: 5, total: 10 }];
    assert.equal(context.progressFor(items, { name: 'Pixel 8' }).done, 5);
    assert.equal(context.progressFor(items, { name: 'Other' }), null);
});

test('one visibility level for both protocols: the more open one wins', () => {
    assert.equal(context.combine('off', 'off'), 'off');
    assert.equal(context.combine('10min', '10min'), '10min');
    assert.equal(context.combine('off', '10min'), '10min');
    assert.equal(context.combine('everyone', '10min'), 'everyone');
});
