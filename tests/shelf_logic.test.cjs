const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../shell/services/ShelfLogic.js'), 'utf8').replace(/^\.pragma library\s*/, '');
const api = vm.runInNewContext(source + '; ({kindOf, drop, mimeFor, shaken})');
const plain = value => JSON.parse(JSON.stringify(value));

test('files are added once each, with their names and kinds', () => {
    const r = api.drop([], ['file:///home/me/Pictures/A%20cat.png', 'file:///home/me/report.pdf', 'file:///home/me/report.pdf'], '');
    assert.equal(r.added, 2);
    assert.deepEqual(plain(r.items.map(i => [i.name, i.kind, i.path])), [
        ['A cat.png', 'image', '/home/me/Pictures/A cat.png'],
        ['report.pdf', 'file', '/home/me/report.pdf'],
    ]);
    const again = api.drop(r.items, ['file:///home/me/report.pdf'], '');
    assert.equal(again.added, 0);
});
test('text is kept when no files came, and only then', () => {
    const r = api.drop([], [], '  Meeting at 4,\n  room 2  ');
    assert.equal(r.added, 1);
    assert.equal(r.items[0].kind, 'text');
    assert.equal(r.items[0].text, 'Meeting at 4,\n  room 2');
    assert.equal(r.items[0].name, 'Meeting at 4, room 2');
    assert.equal(api.drop([], ['file:///a.txt'], 'file:///a.txt').items.length, 1);
    assert.equal(api.drop([], [], 'file:///a.txt').added, 0);
    assert.equal(api.drop([], ['https://example.com'], '').added, 0);
});
test('a drag out carries URIs, and text or paths', () => {
    const files = api.drop([], ['file:///a/one.png', 'file:///a/two.txt'], '').items;
    const m = api.mimeFor(files);
    assert.equal(m['text/uri-list'], 'file:///a/one.png\r\nfile:///a/two.txt\r\n');
    assert.equal(m['text/plain'], '/a/one.png\n/a/two.txt');
    const text = api.drop([], [], 'hello').items;
    assert.deepEqual(plain(api.mimeFor(text)), { 'text/plain': 'hello' });
    assert.deepEqual(plain(api.mimeFor([])), {});
});
test('a shake is four quick turns, not a drag across', () => {
    const at = xs => xs.map((x, i) => ({ t: i * 40, x }));
    assert.equal(api.shaken(at([0, 60, 120, 60, 0, 60, 120, 60, 0, 60, 120]), 40), true);
    assert.equal(api.shaken(at([0, 100, 200, 300, 400, 500]), 40), false);
    assert.equal(api.shaken(at([0, 10, 0, 10, 0, 10, 0, 10, 0]), 40), false); // (a tremble)
    assert.equal(api.shaken(at([0, 80, 0, 80]), 40), false); // (only two turns)
});
