// Fallback logic for setups without the usual hardware support: the recorder's
// "no hardware encoder" check, ddcutil output parsing, the lock screen's
// pam_faillock lockout message and Text Capture's languages.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const read = f => fs.readFileSync(path.join(__dirname, '..', f), 'utf8');
// The text of `function name(...) { ... }` in a QML file
function extract(src, name) {
    const i = src.indexOf('function ' + name + '(');
    assert.ok(i >= 0, name + ' not found');
    let depth = 0;
    for (let j = src.indexOf('{', i); j < src.length; j++) {
        if (src[j] === '{') depth++;
        else if (src[j] === '}' && --depth === 0) return src.slice(i, j + 1);
    }
    throw new Error('unbalanced ' + name);
}
// Run QML functions against `ctx`, standing in for the QML object (root.x and x)
function load(file, names, ctx) {
    const src = read(file);
    ctx.root = ctx;
    const code = 'with (ctx) {' + names.map(n => extract(src, n)).map((f, k) => 'ctx.' + names[k] + ' = ' + f.replace(/^function \w+/, 'function') + ';').join('\n') + '}';
    vm.runInNewContext(code, { ctx, Math, Date, JSON, parseInt, isNaN });
    return ctx;
}

test('recorder: hardware encoder failures are recognised, others are not', () => {
    const r = load('shell/services/Recorder.qml', ['hwEncodeFailure'], {});
    for (const e of [
        'gsr error: vaapi: failed to initialize',
        'Failed to create va display',
        'gsr error: no h264 encoder available on your system',
        'gsr error: your gpu does not support h264 encoding',
        'libva error: /usr/lib/dri/iHD_drv_video.so init failed'])
        assert.equal(r.hwEncodeFailure(e), true, e);
    for (const e of ['gsr error: monitor DP-9 not found', '', 'Permission denied'])
        assert.equal(r.hwEncodeFailure(e), false, e);
});

test('recorder: the CPU retry passes -encoder cpu with h264', () => {
    const src = read('shell/services/Recorder.qml');
    assert.match(src, /"-encoder", "cpu"/);
    assert.match(src, /cpu \? "h264"/);
    assert.match(src, /install intel-media-driver for Intel or libva-nvidia-driver for NVIDIA/);
});

const detect = `Invalid display
   I2C bus:          /dev/i2c-11
   DRM connector:    card1-eDP-1
   drm_connector_id: 513
   Monitor:          AUO::

Display 1
   I2C bus:          /dev/i2c-12
   DRM connector:    card1-DP-1
   drm_connector_id: 531
   Monitor:          BNQ:BenQ GW2786TC:ETEAS00726SL0

Display 2
   I2C bus:          /dev/i2c-5
   DRM connector:    card0-HDMI-A-1
   Monitor:          DEL:DELL U2415:XYZ
`;

test('brightness: ddcutil detect --brief is parsed per connector', () => {
    const reads = [];
    const b = load('shell/services/Brightness.qml', ['parseDetect'], { ddcScanned: false, ddcProblem: '', ddcDisplays: [], readDdc: o => reads.push(o) });
    b.parseDetect(detect);
    assert.deepEqual(JSON.parse(JSON.stringify(b.ddcDisplays)), [
        { output: 'DP-1', bus: 12, model: 'BenQ GW2786TC' },
        { output: 'HDMI-A-1', bus: 5, model: 'DELL U2415' }]);
    assert.deepEqual(reads, ['DP-1', 'HDMI-A-1']);
    assert.equal(b.ddcProblem, '');
    b.parseDetect('PROBLEM module\n');
    assert.equal(b.ddcProblem, 'module');
    assert.equal(b.ddcDisplays.length, 0);
    b.parseDetect('PROBLEM access\n');
    assert.equal(b.ddcProblem, 'access');
});

function lock(extra) {
    return load('shell/modules/lock/Lock.qml', ['lockedText', 'judgeLockout'], Object.assign({ fails: 1, error: 'Incorrect password. Try again.', faillockConf: { deny: 3, unlock: 600, interval: 900 } }, extra));
}
const stamp = ms => {
    const d = new Date(ms), p = n => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
};
const tally = times => 'CONF   \nuser:\nWhen                Type  Source                                           Valid\n' + times.map(t => stamp(t) + ' TTY   /dev/pts/1                                       V').join('\n') + '\n';

test('lock: a wrong password stays "Incorrect password" below the limit', () => {
    const l = lock();
    l.judgeLockout(tally([Date.now() - 5000]));
    assert.equal(l.error, 'Incorrect password. Try again.');
});

test('lock: three recent failures in the tally say how long to wait', () => {
    const now = Date.now();
    const l = lock({ fails: 3 });
    l.judgeLockout(tally([now - 62000, now - 61000, now - 60000]));
    assert.equal(l.error, 'Too many attempts: try again in about 9 minutes.');
});

test('lock: faillock.conf deny and unlock_time are honoured', () => {
    const now = Date.now();
    const l = lock({ fails: 3 });
    l.judgeLockout('CONF 5 120 900\n' + tally([now - 3000, now - 2000, now - 1000]).replace('CONF   \n', ''));
    assert.equal(l.error, 'Incorrect password. Try again.'); // 3 of 5
    assert.equal(l.faillockConf.deny, 5);
    assert.equal(l.faillockConf.unlock, 120);
    const l2 = lock({ fails: 1 });
    l2.judgeLockout('CONF 3 never \nuser:\nWhen                Type  Source  Valid\n' + [3, 2, 1].map(s => stamp(now - s * 1000) + ' TTY /dev/pts/1 V').join('\n'));
    assert.match(l2.error, /stays locked/);
});

test('lock: old or invalidated failures do not count', () => {
    const now = Date.now();
    const l = lock({ fails: 3 });
    l.judgeLockout(tally([now - 2000000, now - 1990000, now - 1000]));
    assert.equal(l.error, 'Incorrect password. Try again.');
    const l2 = lock({ fails: 3 });
    l2.judgeLockout(tally([now - 3000, now - 2000, now - 1000]).replace(/ V\n/g, ' I\n'));
    assert.equal(l2.error, 'Incorrect password. Try again.');
});

test('lock: without a readable tally, three failures in a row count as a lockout', () => {
    assert.equal(lock({ fails: 2 }).judgeLockout('CONF   \nNOREAD\n') ?? 'x', 'x');
    const l = lock({ fails: 3 });
    l.judgeLockout('CONF   \nNOREAD\n');
    assert.equal(l.error, 'Too many attempts: try again in about 10 minutes.');
});

const ocr = vm.runInNewContext(read('shell/services/OcrLangs.js').replace(/^\.pragma library\s*/, '') + '; ({codesFor, pick})');
const plain = v => JSON.parse(JSON.stringify(v));
const langs = 'List of available languages in "/usr/share/tessdata/" (5):\ndeu\neng\nosd\nchi_tra\nsrp_latn\n';

test('ocr: the system language comes first when its data is installed', () => {
    assert.deepEqual(plain(ocr.pick({ LANG: 'de_DE.UTF-8' }, langs)), { langs: ['deu', 'eng'], missing: [] });
    assert.deepEqual(plain(ocr.pick({ LANG: 'en_US.UTF-8' }, langs)), { langs: ['eng'], missing: [] });
    assert.deepEqual(plain(ocr.pick({ LANG: 'C.UTF-8' }, langs)), { langs: ['eng'], missing: [] });
});

test('ocr: missing language data is reported, LANGUAGE and LC_* add languages', () => {
    assert.deepEqual(plain(ocr.pick({ LANG: 'fr_FR.UTF-8' }, langs)), { langs: ['eng'], missing: ['fra'] });
    assert.deepEqual(plain(ocr.pick({ LANG: 'en_GB.UTF-8', LC_MESSAGES: 'hi_IN', LANGUAGE: 'es:de' }, langs)), { langs: ['deu', 'eng'], missing: ['hin', 'spa'] });
    assert.deepEqual(plain(ocr.pick({ LANG: 'zh_TW.UTF-8' }, langs)).langs, ['chi_tra', 'eng']);
    assert.deepEqual(plain(ocr.pick({ LANG: 'sr_RS.UTF-8@latin' }, langs)).langs, ['srp_latn', 'eng']);
    assert.deepEqual(plain(ocr.codesFor('zh_CN.UTF-8')), ['chi_sim']);
});

test('ocr: without English data, only installed languages are passed', () => {
    assert.deepEqual(plain(ocr.pick({ LANG: 'de_DE.UTF-8' }, 'deu\nosd\n')).langs, ['deu']);
    assert.deepEqual(plain(ocr.pick({ LANG: 'de_DE.UTF-8' }, '')).langs, ['eng']);
});

test('keymap: volume and brightness work without media keys, on free combos', () => {
    const km = JSON.parse(read('hypr/irice/keymap.json'));
    const keys = km.map(b => b.keys);
    assert.equal(new Set(keys).size, keys.length, 'duplicate key combos');
    for (const [k, a] of [['$MOD + F12', 'volumeUp'], ['$MOD + F11', 'volumeDown'], ['$MOD + F2', 'brightnessUp'], ['$MOD + F1', 'brightnessDown']]) {
        const b = km.find(x => x.keys === k);
        assert.ok(b, k);
        assert.equal(b.group, 'Media');
        assert.equal(b.action.shell, a);
    }
});
