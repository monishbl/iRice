const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../shell/common/Markdown.js'), 'utf8').replace(/^\.pragma library\s*/, '');
const md = vm.runInNewContext(source + '; ({split, html, escape})');
const st = { text: '#eeeeee', muted: '#999999', link: '#8888ff', codeBg: '#222222', rule: '#444444', mono: 'Geist Mono', body: 14 };
const plain = value => JSON.parse(JSON.stringify(value));

test('fenced code is its own part, with its language; prose around it stays', () => {
    const parts = plain(md.split('Run this:\n\n```bash\nls -la\n```\n\nThen this.'));
    assert.deepEqual(parts.map(p => p.kind), ['text', 'code', 'text']);
    assert.equal(parts[1].lang, 'bash');
    assert.equal(parts[1].code, 'ls -la');
    assert.equal(parts[1].open, false);
});

test('a code block still streaming is open and keeps what came so far', () => {
    const parts = plain(md.split('Here:\n```python\nprint(1)\nprint('));
    assert.equal(parts[1].open, true);
    assert.equal(parts[1].code, 'print(1)\nprint(');
});

test('HTML in a reply is shown as text, never as markup', () => {
    const out = md.html('<img src="https://x/y.png"> <b>hi</b> <script>alert(1)</script>', st);
    assert.ok(!out.includes('<img'));
    assert.ok(!out.includes('<script'));
    assert.ok(out.includes('&lt;img'));
});

test('links open only for http(s) and mailto', () => {
    assert.ok(md.html('[site](https://example.org/a?b=1&c=2)', st).includes('href="https://example.org/a?b=1&amp;c=2"'));
    assert.ok(md.html('[mail](mailto:a@b.c)', st).includes('href="mailto:a@b.c"'));
    for (const bad of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,x', 'qrc:/x']) {
        const out = md.html('[x](' + bad + ')', st);
        assert.ok(!out.includes('href'), bad);
    }
});

test('images are never fetched: their text becomes a link', () => {
    const out = md.html('![a chart](https://example.org/c.png)', st);
    assert.ok(!out.includes('<img'));
    assert.ok(out.includes('>a chart</a>'));
});

test('emphasis, strike, inline code untouched inside', () => {
    const out = md.html('**bold** *it* _it2_ ~~gone~~ `a*b*c` snake_case_name', st);
    assert.ok(out.includes('<b>bold</b>'));
    assert.ok(out.includes('<i>it</i>'));
    assert.ok(out.includes('<i>it2</i>'));
    assert.ok(out.includes('<s>gone</s>'));
    assert.ok(out.includes('a*b*c'));
    assert.ok(out.includes('snake_case_name'));
});

test('headings stay within the product type scale', () => {
    const out = md.html('# Title\n## Sub', st);
    const sizes = [...out.matchAll(/font-size:(\d+)px/g)].map(m => +m[1]);
    assert.ok(sizes.length === 2 && sizes.every(s => s <= 24));
});

test('lists: nested, numbered from their start, tasks', () => {
    const out = md.html('- one\n  - inner\n- two\n\n3. three\n4. four\n\n- [ ] todo\n- [x] done', st);
    assert.equal((out.match(/<ul/g) || []).length, 3);
    assert.ok(out.includes('<ol start="3"'));
    assert.ok(out.includes('☐ todo') && out.includes('☑ done'));
});

test('tables with alignment', () => {
    const out = md.html('| a | b |\n|:--|--:|\n| 1 | 2 |', st);
    assert.ok(out.includes('<table'));
    assert.ok(out.includes('<th align="left"'));
    assert.ok(out.includes('<td align="right"'));
});

test('quotes and rules', () => {
    const out = md.html('> quoted **text**\n\n---', st);
    assert.ok(out.includes('<blockquote'));
    assert.ok(out.includes('<b>text</b>'));
    assert.ok(out.includes('<hr'));
});
