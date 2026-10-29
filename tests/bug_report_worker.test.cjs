// The bug report Worker (site/bugs/src/index.js) against an in-memory stand-in
// for D1 and a fake GitHub API: validation, the rate limit, storage, the
// issue it files, and the screenshot link.
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");

// Just the SQL the Worker uses, over plain arrays
class FakeD1 {
    constructor() {
        this.reports = [];
        this.rate = new Map();
    }
    prepare(sql) {
        const db = this;
        let args = [];
        const stmt = {
            bind(...a) {
                args = a;
                return stmt;
            },
            async run() {
                if (sql.startsWith("DELETE FROM rate")) {
                    for (const [k, v] of db.rate) if (v.hour < args[0]) db.rate.delete(k);
                } else if (sql.startsWith("INSERT INTO reports")) {
                    const [id, created, title, happened, steps, expected, version, sysinfo, log, screenshot, screenshot_type] = args;
                    db.reports.push({ id, created, title, happened, steps, expected, version, sysinfo, log, screenshot, screenshot_type, issue_url: null });
                } else if (sql.startsWith("UPDATE reports SET issue_url")) {
                    db.reports.find(r => r.id === args[1]).issue_url = args[0];
                } else {
                    throw new Error("unexpected SQL: " + sql);
                }
                return { success: true };
            },
            async first() {
                if (sql.startsWith("INSERT INTO rate")) {
                    const [bucket, hour] = args;
                    const row = db.rate.get(bucket) ?? { hour, count: 0 };
                    row.count++;
                    db.rate.set(bucket, row);
                    return { count: row.count };
                }
                if (sql.startsWith("SELECT screenshot")) {
                    const r = db.reports.find(x => x.id === args[0]);
                    return r ? { screenshot: r.screenshot, screenshot_type: r.screenshot_type } : null;
                }
                throw new Error("unexpected SQL: " + sql);
            },
        };
        return stmt;
    }
}

let worker, validate, issueBody;
test.before(async () => {
    const mod = await import(path.join(__dirname, "../site/bugs/src/index.js"));
    worker = mod.default;
    validate = mod.validate;
    issueBody = mod.issueBody;
});

const PNG = Buffer.from("89504e470d0a1a0a0000000d49484452", "hex").toString("base64");
const post = (body, ip = "203.0.113.7") =>
    new Request("https://irice-bugs.monishbl.in/api/bug-reports", {
        method: "POST",
        headers: { "Content-Type": "application/json", "CF-Connecting-IP": ip },
        body: typeof body === "string" ? body : JSON.stringify(body),
    });

function fakeGithub(t, answer = { status: 201, body: { html_url: "https://github.com/monishbl/iRice/issues/42" } }) {
    const calls = [];
    const real = globalThis.fetch;
    globalThis.fetch = async (url, init) => {
        calls.push({ url, init });
        return new Response(JSON.stringify(answer.body), { status: answer.status });
    };
    t.after(() => (globalThis.fetch = real));
    return calls;
}

test("a report is stored and filed as an issue, without the sender's IP", async t => {
    const calls = fakeGithub(t);
    const env = { DB: new FakeD1(), GITHUB_TOKEN: "ghp_test", RATE_SALT: "salt" };
    const res = await worker.fetch(post({
        title: "Dock vanishes",
        happened: "After waking, the dock is gone. cc @someone",
        steps: "1. Sleep 2. Wake",
        sysinfo: { iRice: "abc123", Hyprland: "0.56.2" },
        log: "line 1\n~~~~ fence\nline 3",
        screenshot: { type: "image/png", data: PNG },
    }), env);
    assert.equal(res.status, 201);
    const out = await res.json();
    assert.equal(out.ok, true);
    assert.match(out.id, /^[0-9a-f]{32}$/);
    assert.equal(out.issue_url, "https://github.com/monishbl/iRice/issues/42");
    assert.equal(env.DB.reports.length, 1);
    assert.equal(env.DB.reports[0].issue_url, out.issue_url);
    assert.ok(!JSON.stringify(env.DB.reports[0]).includes("203.0.113.7"), "IP stored");

    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "https://api.github.com/repos/monishbl/iRice/issues");
    assert.equal(calls[0].init.headers.Authorization, "Bearer ghp_test");
    const issue = JSON.parse(calls[0].init.body);
    assert.deepEqual(issue.labels, ["bug-report"]);
    assert.ok(!issue.body.includes("203.0.113.7"));
    assert.ok(!issue.body.includes("@someone"), "mention not broken");
    assert.ok(issue.body.includes("| Hyprland | 0.56.2 |"));
    assert.ok(issue.body.includes(`/api/bug-reports/${out.id}/screenshot`));
    assert.ok(issue.body.includes("<details>"));
    // The log's own ~~~~ can't close the fence
    assert.equal(issue.body.match(/~~~~/g).length, 2);
});

test("the screenshot is served by its id, and only by it", async t => {
    fakeGithub(t);
    const env = { DB: new FakeD1(), GITHUB_TOKEN: "x" };
    const { id } = await (await worker.fetch(post({ title: "T", happened: "H", screenshot: { type: "image/png", data: PNG } }), env)).json();
    const shot = await worker.fetch(new Request(`https://irice-bugs.monishbl.in/api/bug-reports/${id}/screenshot`), env);
    assert.equal(shot.status, 200);
    assert.equal(shot.headers.get("Content-Type"), "image/png");
    assert.equal(Buffer.from(await shot.arrayBuffer()).toString("base64"), PNG);
    for (const bad of ["0".repeat(32), "../../etc", id.slice(0, 31)]) {
        const r = await worker.fetch(new Request(`https://irice-bugs.monishbl.in/api/bug-reports/${bad}/screenshot`), env);
        assert.equal(r.status, 404, bad);
    }
});

test("missing fields, wrong types, bad JSON and oversized reports are refused with a reason", async () => {
    const env = { DB: new FakeD1() };
    const cases = [
        ["{not json", 400, /valid JSON/],
        [{ happened: "x" }, 400, /title/],
        [{ title: "x" }, 400, /happened/],
        [{ title: 5, happened: "x" }, 400, /must be text/],
        [{ title: "x".repeat(121), happened: "x" }, 400, /too long/],
        [{ title: "x", happened: "x", screenshot: { type: "image/svg+xml", data: PNG } }, 400, /PNG, WebP or JPEG/],
        [{ title: "x", happened: "x", sysinfo: [1, 2] }, 400, /object/],
        [{ title: "x", happened: "x", log: "y".repeat(2.6 * 1024 * 1024) }, 413, /too large/],
    ];
    for (const [body, status, why] of cases) {
        const res = await worker.fetch(post(body), env);
        assert.equal(res.status, status, JSON.stringify(body).slice(0, 60));
        assert.match((await res.json()).error, why);
    }
    assert.equal(env.DB.reports.length, 0);
});

test("five reports an hour from one network, then 429; another network is unaffected", async t => {
    fakeGithub(t);
    const env = { DB: new FakeD1(), GITHUB_TOKEN: "x", RATE_SALT: "s" };
    for (let i = 0; i < 5; i++)
        assert.equal((await worker.fetch(post({ title: "T", happened: "H" }), env)).status, 201);
    const sixth = await worker.fetch(post({ title: "T", happened: "H" }), env);
    assert.equal(sixth.status, 429);
    assert.match((await sixth.json()).error, /Wait an hour/);
    assert.equal((await worker.fetch(post({ title: "T", happened: "H" }, "198.51.100.1"), env)).status, 201);
    // Buckets are hashes, not addresses
    for (const k of env.DB.rate.keys())
        assert.match(k, /^[0-9a-f]{64}$/);
});

test("without a GitHub token, or when GitHub fails, the report is still kept", async t => {
    const env = { DB: new FakeD1() };
    let res = await worker.fetch(post({ title: "T", happened: "H" }), env);
    assert.equal(res.status, 201);
    assert.equal((await res.json()).issue_url, null);

    fakeGithub(t, { status: 500, body: {} });
    const env2 = { DB: new FakeD1(), GITHUB_TOKEN: "x" };
    res = await worker.fetch(post({ title: "T", happened: "H" }), env2);
    assert.equal(res.status, 201);
    assert.equal((await res.json()).issue_url, null);
    assert.equal(env2.DB.reports.length, 1);
});

test("other paths and methods are not found or not allowed", async () => {
    const env = { DB: new FakeD1() };
    assert.equal((await worker.fetch(new Request("https://irice-bugs.monishbl.in/api/bug-reports"), env)).status, 405);
    assert.equal((await worker.fetch(new Request("https://irice-bugs.monishbl.in/"), env)).status, 404);
});

test("a long log is cut from the start to fit GitHub's issue size", () => {
    const r = validate({ title: "T", happened: "H", log: "old\n" + "x".repeat(150000) + "\nnewest line" });
    const body = issueBody(r, "a".repeat(32), "https://irice-bugs.monishbl.in");
    assert.ok(body.length < 65536);
    assert.ok(body.includes("newest line"));
    assert.ok(!body.includes("old\n"));
});
