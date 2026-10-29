// Browsers' partial downloads followed between looks (shell/services/DownloadTracker.js)
const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const src = fs.readFileSync(path.join(__dirname, "..", "shell", "services", "DownloadTracker.js"), "utf8").replace(/^\.pragma library\s*$/m, "");
const D = {};
vm.runInNewContext(src, D);

const dir = "/home/u/Downloads";
function look(tracked, entries, now) {
    return D.update(tracked, entries.map(([name, bytes]) => ({ path: dir + "/" + name, bytes })), now);
}

test("a growing download is shown by its real name, with speed", () => {
    let r = look(null, [["film.mkv.part", 1000]], 0);
    r = look(r.tracked, [["film.mkv.part", 3000]], 1000);
    assert.strictEqual(r.items.length, 1);
    assert.strictEqual(r.items[0].name, "film.mkv");
    assert.strictEqual(r.items[0].done, 3000);
    assert.strictEqual(r.items[0].speed, 2000);
});

test("Chromium's Unconfirmed name shows as Download", () => {
    const r = look(null, [["Unconfirmed 123456.crdownload", 500]], 0);
    assert.strictEqual(r.items[0].name, "Download");
    assert.strictEqual(D.displayName(dir + "/.hidden.zip.crdownload"), "hidden.zip");
});

test("an empty placeholder is never shown", () => {
    let r = look(null, [["Unconfirmed 99.crdownload", 0]], 0);
    assert.strictEqual(r.items.length, 0);
    r = look(r.tracked, [["Unconfirmed 99.crdownload", 0]], 5000);
    assert.strictEqual(r.items.length, 0);
    r = look(r.tracked, [], 6000);
    assert.strictEqual(r.gone.length, 0, "an empty file that went away isn't a finished download");
});

test("a download that stops growing is hidden after the stall time, and shown again when it resumes", () => {
    let r = look(null, [["a.iso.part", 100]], 0);
    r = look(r.tracked, [["a.iso.part", 200]], 1000);
    r = look(r.tracked, [["a.iso.part", 200]], 1000 + D.STALL_MS - 1);
    assert.strictEqual(r.items.length, 1);
    r = look(r.tracked, [["a.iso.part", 200]], 1000 + D.STALL_MS + 1);
    assert.strictEqual(r.items.length, 0);
    r = look(r.tracked, [["a.iso.part", 300]], 1000 + D.STALL_MS + 2000);
    assert.strictEqual(r.items.length, 1);
});

test("a file that went away after growing is reported once", () => {
    let r = look(null, [["film.mkv.part", 100]], 0);
    r = look(r.tracked, [["film.mkv.part", 900]], 1000);
    r = look(r.tracked, [], 2000);
    assert.strictEqual(r.items.length, 0);
    assert.strictEqual(r.gone.length, 1);
    assert.strictEqual(r.gone[0].name, "film.mkv");
    assert.strictEqual(r.gone[0].bytes, 900);
    r = look(r.tracked, [], 3000);
    assert.strictEqual(r.gone.length, 0);
});

test("the finished file: same name first", () => {
    const g = { path: dir + "/film.mkv.part", name: "film.mkv", unconfirmed: false, bytes: 900, at: 1000 };
    const files = [
        { path: dir + "/other.bin", bytes: 5000, mtime: 1500 },
        { path: dir + "/film.mkv", bytes: 1200, mtime: 1400 },
    ];
    assert.strictEqual(D.finalFile(g, files), dir + "/film.mkv");
});

test("the finished file for an Unconfirmed download: newest big enough, never a partial", () => {
    const g = { path: dir + "/Unconfirmed 5.crdownload", name: "Unconfirmed 5", unconfirmed: true, bytes: 900, at: 1000 };
    const files = [
        { path: dir + "/tiny.txt", bytes: 10, mtime: 1600 },
        { path: dir + "/next.zip.crdownload", bytes: 5000, mtime: 1700 },
        { path: dir + "/old.pdf", bytes: 5000, mtime: 0 },
        { path: dir + "/report.pdf", bytes: 950, mtime: 1500 },
    ];
    assert.strictEqual(D.finalFile(g, files), dir + "/report.pdf");
});

test("nothing fits: it was cancelled", () => {
    const g = { path: dir + "/film.mkv.part", name: "film.mkv", unconfirmed: false, bytes: 900, at: 1000 };
    assert.strictEqual(D.finalFile(g, [{ path: dir + "/small.txt", bytes: 3, mtime: 1500 }]), "");
});
