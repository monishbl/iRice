// Focus that follows the pointer (shell/services/FocusDecision.js), run in
// Node the way the shell runs it.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = vm.createContext({});
const src = fs.readFileSync(__dirname + '/../shell/services/FocusDecision.js', 'utf8').replace(/^\.pragma library\s*/m, '');
vm.runInContext(src, context);
const plain = x => JSON.parse(JSON.stringify(x));
const ev = (name, data = '') => ({ name, data });

test('a window that opened gets the pointer', () => {
    assert.deepEqual(plain(context.afterEvents([ev('activewindowv2', 'ab12'), ev('openwindow', 'ab12,1,kitty,~')])), { kind: 'warp', address: '0xab12' });
    assert.deepEqual(plain(context.afterEvents([ev('openwindow', '0xCD,1,a,b')])), { kind: 'warp', address: '0xCD' });
});

test('an opening window wins over the space change it caused', () => {
    assert.deepEqual(plain(context.afterEvents([ev('workspacev2', '2,2'), ev('openwindow', 'ff,2,zen,x')])), { kind: 'warp', address: '0xff' });
});

test('a scene change under a still pointer refocuses', () => {
    for (const name of ['workspace', 'workspacev2', 'closewindow', 'movewindowv2', 'activespecial', 'changefloatingmode'])
        assert.deepEqual(plain(context.afterEvents([ev(name, 'x')])), { kind: 'refocus' }, name);
});

test('focus changes alone are left to Hyprland and the plugin', () => {
    // (hover focus, a click, an app brought forward on the same space)
    assert.deepEqual(plain(context.afterEvents([ev('activewindow', 'kitty,t'), ev('activewindowv2', 'ab')])), { kind: 'none' });
    assert.deepEqual(plain(context.afterEvents([])), { kind: 'none' });
    assert.deepEqual(plain(context.afterEvents(undefined)), { kind: 'none' });
});

test('a malformed openwindow address is ignored', () => {
    assert.deepEqual(plain(context.afterEvents([ev('openwindow', 'not-hex,1,a,b')])), { kind: 'none' });
});

test('asking for attention right after a click or a key brings the window forward', () => {
    assert.equal(context.onUrgent(0, false), 'bring');
    assert.equal(context.onUrgent(2999, false), 'bring');
    assert.equal(context.onUrgent(3000, false), 'bring');
});

test('asking for attention on its own only notifies', () => {
    assert.equal(context.onUrgent(3001, false), 'notify');
    assert.equal(context.onUrgent(60000, false), 'notify');
    assert.equal(context.onUrgent(Infinity, false), 'notify');
    assert.equal(context.onUrgent(NaN, false), 'notify');
    assert.equal(context.onUrgent(-5, false), 'notify');
});

test('a window that is already in front needs nothing', () => {
    assert.equal(context.onUrgent(0, true), 'none');
});
