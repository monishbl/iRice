const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../shell/services/AiChats.js'), 'utf8').replace(/^\.pragma library\s*/, '');
const c = vm.runInNewContext(source + '; ({validId, newId, titleFrom, cleanTitle, previewOf, worthKeeping, stripPictures, session, entryOf, upsert, without, patch, parseIndex, indexText, migrate, prune, matches, filter, groupName, group, relativeTime, DAY})');

// (local times, so the tests don't depend on the time zone)
const at = (y, m, d, h = 12, min = 0) => new Date(y, m - 1, d, h, min).getTime();
const NOW = at(2026, 11, 4, 15, 30);
const entry = (id, updated, extra = {}) => Object.assign({ id, title: id, created: updated, updated, model: '', modelName: '', pinned: false, count: 2, preview: '' }, extra);
const plain = x => JSON.parse(JSON.stringify(x));
// (values from the vm context are another realm's: compare them as JSON)
const same = (actual, expected, msg) => assert.deepEqual(plain(actual), expected, msg);

test('the title is the first thing asked, on one line, about 60 characters', () => {
    assert.equal(c.titleFrom('  How do I   resize a window?  '), 'How do I resize a window?');
    assert.equal(c.titleFrom('\n\n  first line\nsecond line'), 'first line');
    const long = 'Explain the difference between tiling and floating windows in Hyprland and when to use each';
    const t = c.titleFrom(long);
    assert.ok(t.endsWith('…'));
    assert.ok(t.length <= 61, t);
    assert.ok(long.startsWith(t.slice(0, -1)), 'cut at a word: ' + t);
    assert.equal(c.titleFrom('x'.repeat(100)), 'x'.repeat(60) + '…');
    assert.equal(c.titleFrom(''), 'New chat');
    assert.equal(c.titleFrom('   \n  '), 'New chat');
});

test('renamed titles are one line and capped', () => {
    assert.equal(c.cleanTitle('  Trip\nplans  '), 'Trip plans');
    assert.equal(c.cleanTitle('y'.repeat(300)).length, 120);
});

test('ids are file-safe, and anything else is refused', () => {
    const id = c.newId(NOW, 0.5);
    assert.ok(c.validId(id), id);
    for (const bad of ['', '../x', 'a/b', 'A', 'x'.repeat(65), null, 5, 'index.json'])
        assert.equal(c.validId(bad), false, String(bad));
});

test('a chat is kept only once something was sent in it', () => {
    assert.equal(c.worthKeeping([]), false);
    assert.equal(c.worthKeeping([{ role: 'note', text: 'Add your key' }]), false);
    assert.equal(c.worthKeeping([{ role: 'user', text: 'hi' }]), true);
});

test('pictures are never stored, only a note that there were some', () => {
    const out = c.stripPictures([
        { role: 'user', text: 'what is this', images: [{ data: 'AAAA', mediaType: 'image/jpeg' }, { data: 'BBBB', mediaType: 'image/jpeg' }] },
        { role: 'user', text: 'and this', image: { data: 'CCCC' } },
        { role: 'assistant', text: 'a cat', blocks: [{ type: 'text', text: 'a cat' }] }
    ]);
    assert.equal(out[0].text, '[2 pictures were attached here]\nwhat is this');
    assert.equal(out[1].text, '[A picture was attached here]\nand this');
    assert.ok(!JSON.stringify(out).includes('AAAA') && !JSON.stringify(out).includes('CCCC'));
    same(out[2].blocks, [{ type: 'text', text: 'a cat' }]);
});

test('a session and its index entry', () => {
    const rows = [{ role: 'user', text: 'Hello\nthere' }, { role: 'assistant', text: 'Hi!  How can I help?' }, { role: 'note', text: 'Switched' }];
    const s = c.session({ id: 'a', title: 'Hello', created: 1, updated: 2, model: 'k', modelName: 'Claude', pinned: false }, rows, [{ role: 'user', text: 'x', images: [{ data: 'Z' }] }]);
    assert.ok(!JSON.stringify(s).includes('"Z"'));
    const e = c.entryOf(s);
    same(e, { id: 'a', title: 'Hello', created: 1, updated: 2, model: 'k', modelName: 'Claude', pinned: false, count: 2, preview: 'Hello there Hi! How can I help?' });
    assert.ok(c.previewOf([{ role: 'user', text: 'w '.repeat(400) }]).length <= 300);
});

test('index upkeep: add, replace, remove and change entries, newest first', () => {
    let list = [];
    list = c.upsert(list, entry('a', 10));
    list = c.upsert(list, entry('b', 30));
    list = c.upsert(list, entry('c', 20));
    same(list.map(x => x.id), ['b', 'c', 'a']);
    list = c.upsert(list, entry('a', 40, { title: 'new' }));
    same(list.map(x => x.id), ['a', 'b', 'c']);
    assert.equal(list.length, 3);
    assert.equal(list[0].title, 'new');
    list = c.patch(list, 'b', { pinned: true });
    assert.equal(list[1].pinned, true);
    assert.equal(list[0].pinned, false);
    list = c.without(list, 'c');
    same(list.map(x => x.id), ['a', 'b']);
});

test('the index reads back what it wrote, and drops what is damaged', () => {
    const text = c.indexText('a', [entry('a', 5), entry('b', 9, { pinned: true })], ['z']);
    const back = c.parseIndex(text);
    assert.equal(back.current, 'a');
    same(back.trash, ['z']);
    same(back.chats.map(x => x.id), ['b', 'a']);
    assert.equal(back.chats[0].pinned, true);

    assert.equal(c.parseIndex('not json'), null);
    assert.equal(c.parseIndex('{"chats": 4}'), null);
    const odd = c.parseIndex(JSON.stringify({ current: 'gone', trash: ['../etc', 'ok'], chats: [entry('ok', 1), entry('ok', 2), { id: '../x', updated: 1 }, { id: 'n' }, null, { id: 't', updated: 3, title: '' }] }));
    same(odd.chats.map(x => x.id), ['t', 'ok']);
    assert.equal(odd.chats[0].title, 'New chat');
    assert.equal(odd.current, '');
    same(odd.trash, ['ok']);
});

test('the old single chat becomes the first session, or nothing', () => {
    const old = { rows: [{ role: 'user', text: 'Old question about fonts' }, { role: 'assistant', text: 'Answer', model: 'Gemini' }], history: [{ role: 'user', text: 'q', images: [{ data: 'P' }] }] };
    const s = c.migrate(old, 'm1', NOW);
    assert.equal(s.id, 'm1');
    assert.equal(s.title, 'Old question about fonts');
    assert.equal(s.modelName, 'Gemini');
    assert.equal(s.rows.length, 2);
    assert.ok(s.history[0].text.startsWith('[A picture was attached here]'));
    assert.equal(c.migrate({ rows: [{ role: 'note', text: 'n' }] }, 'm2', NOW), null);
    assert.equal(c.migrate(null, 'm3', NOW), null);
});

test('pruning by age keeps pinned chats and the open one', () => {
    const chats = [
        entry('fresh', NOW - 5 * c.DAY),
        entry('old', NOW - 40 * c.DAY),
        entry('older', NOW - 100 * c.DAY),
        entry('oldpin', NOW - 400 * c.DAY, { pinned: true }),
        entry('open', NOW - 200 * c.DAY)
    ];
    same(c.prune(chats, 0, NOW, []), []);
    same(c.prune(chats, 30, NOW, ['open']), ['old', 'older']);
    same(c.prune(chats, 90, NOW, ['open']), ['older']);
    same(c.prune(chats, 90, NOW, []), ['older', 'open']);
});

test('search matches every word in the title or the words of the chat, ignoring case', () => {
    const chats = [
        entry('a', 3, { title: 'Resize windows', preview: 'how do I resize a window in Hyprland' }),
        entry('b', 2, { title: 'Recipe', preview: 'Pasta with tomatoes' }),
        entry('c', 1, { title: 'Fonts', preview: 'install Inter' })
    ];
    same(c.filter(chats, '').map(x => x.id), ['a', 'b', 'c']);
    same(c.filter(chats, 'HYPRLAND').map(x => x.id), ['a']);
    same(c.filter(chats, 'pasta tomatoes').map(x => x.id), ['b']);
    same(c.filter(chats, 'pasta hyprland').map(x => x.id), []);
    same(c.filter(chats, '  fonts  ').map(x => x.id), ['c']);
    // (a match found in the whole file)
    same(c.filter(chats, 'kerning', ['c']).map(x => x.id), ['c']);
});

test('groups: Pinned, Today, Yesterday, Previous 7 days, then months', () => {
    assert.equal(c.groupName(at(2026, 11, 4, 0, 5), NOW), 'Today');
    assert.equal(c.groupName(at(2026, 11, 3, 23, 59), NOW), 'Yesterday');
    assert.equal(c.groupName(at(2026, 11, 2), NOW), 'Previous 7 days');
    assert.equal(c.groupName(at(2026, 10, 28), NOW), 'Previous 7 days');
    assert.equal(c.groupName(at(2026, 10, 27), NOW), 'October');
    assert.equal(c.groupName(at(2025, 12, 30), NOW), 'December 2025');

    const rows = c.group([
        entry('t1', at(2026, 11, 4, 9)),
        entry('p', at(2026, 1, 2), { pinned: true }),
        entry('y', at(2026, 11, 3)),
        entry('t2', at(2026, 11, 4, 14)),
        entry('w', at(2026, 10, 30)),
        entry('o1', at(2026, 9, 10)),
        entry('o2', at(2026, 9, 20))
    ], NOW);
    same(rows.map(r => r.header ?? r.chat.id), ['Pinned', 'p', 'Today', 't2', 't1', 'Yesterday', 'y', 'Previous 7 days', 'w', 'September', 'o2', 'o1']);
    same(c.group([], NOW), []);
});

test('relative times', () => {
    assert.equal(c.relativeTime(NOW - 10000, NOW), 'Just now');
    assert.equal(c.relativeTime(NOW - 5 * 60000, NOW), '5 min ago');
    assert.equal(c.relativeTime(at(2026, 11, 4, 12, 0), NOW), '3 h ago');
    assert.equal(c.relativeTime(at(2026, 11, 3, 22), NOW), 'Yesterday');
    assert.equal(c.relativeTime(at(2026, 11, 2), NOW), 'Monday');
    assert.equal(c.relativeTime(at(2026, 10, 3), NOW), '3 Oct');
    assert.equal(c.relativeTime(at(2025, 10, 3), NOW), '3 Oct 2025');
    assert.equal(c.relativeTime(NOW + 5000, NOW), 'Just now');
});
