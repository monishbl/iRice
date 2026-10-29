// iRice bug reports: POST /api/bug-reports from iRice's Settings → Report a
// Bug (shell/assets/scripts/bug-report.py), stored in D1 and filed as a
// GitHub issue in monishbl/iRice. A report's screenshot is served at
// /api/bug-reports/<id>/screenshot, where the id is 128 random bits, so only
// the issue's link finds it.
//
// Nothing that identifies the sender is kept: the IP address is only used,
// hashed with a secret salt and the hour, to allow a few reports an hour from
// one network. The shell scrubs the log and system info before sending.

const REPO = "monishbl/iRice";
const MAX_BODY = 2.5 * 1024 * 1024; // the client caps itself at 2 MB
const MAX_SHOT = 1.5 * 1024 * 1024; // decoded bytes (D1 rows top out at 2 MB)
const PER_HOUR = 5;
const SHOT_TYPES = ["image/png", "image/webp", "image/jpeg"];

const json = (data, status = 200) =>
    new Response(JSON.stringify(data), { status, headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" } });
const fail = (status, error) => json({ ok: false, error }, status);

function hex(bytes) {
    return [...new Uint8Array(bytes)].map(b => b.toString(16).padStart(2, "0")).join("");
}
async function sha256(text) {
    return hex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
}
function newId() {
    return hex(crypto.getRandomValues(new Uint8Array(16)));
}

// The report as sent, checked; throws a message that says how to fix it
export function validate(r) {
    if (!r || typeof r !== "object" || Array.isArray(r))
        throw new Error("The report isn't valid JSON.");
    const str = (k, max, need) => {
        const v = r[k] ?? "";
        if (typeof v !== "string")
            throw new Error(`"${k}" must be text.`);
        if (need && v.trim() === "")
            throw new Error(`"${k}" is required.`);
        if (v.length > max)
            throw new Error(`"${k}" is too long (at most ${max} characters).`);
        return v.trim();
    };
    const out = {
        title: str("title", 120, true),
        happened: str("happened", 20000, true),
        steps: str("steps", 20000, false),
        expected: str("expected", 20000, false),
        version: str("version", 200, false),
        log: str("log", 200000, false),
        sysinfo: {},
        screenshot: null,
    };
    if (r.sysinfo !== undefined) {
        if (!r.sysinfo || typeof r.sysinfo !== "object" || Array.isArray(r.sysinfo))
            throw new Error('"sysinfo" must be an object.');
        for (const [k, v] of Object.entries(r.sysinfo).slice(0, 20))
            out.sysinfo[String(k).slice(0, 40)] = String(v).slice(0, 300);
    }
    if (r.screenshot !== undefined && r.screenshot !== null) {
        const s = r.screenshot;
        if (typeof s !== "object" || !SHOT_TYPES.includes(s.type) || typeof s.data !== "string")
            throw new Error("The screenshot must be a PNG, WebP or JPEG.");
        let bytes;
        try {
            bytes = Uint8Array.from(atob(s.data), c => c.charCodeAt(0));
        } catch (e) {
            throw new Error("The screenshot isn't valid base64.");
        }
        if (bytes.length > MAX_SHOT)
            throw new Error("The screenshot is too large.");
        out.screenshot = { type: s.type, bytes };
    }
    return out;
}

// GitHub would turn "@someone" into a mention and notify them: break those
const quiet = t => t.replace(/@(?=[\w-])/g, "@​");
// Text inside a fenced block can't close the fence
const fenced = t => "~~~~text\n" + t.replace(/~{4,}/g, "~~~") + "\n~~~~";

export function issueBody(r, id, origin) {
    const parts = ["### What happened", quiet(r.happened)];
    if (r.steps)
        parts.push("### Steps to reproduce", quiet(r.steps));
    if (r.expected)
        parts.push("### What was expected", quiet(r.expected));
    const info = Object.entries(r.sysinfo);
    if (info.length)
        parts.push("### System", "| | |\n|---|---|\n" + info.map(([k, v]) => `| ${k.replace(/\|/g, "/")} | ${quiet(v).replace(/\|/g, "/")} |`).join("\n"));
    if (r.screenshot)
        parts.push("### Screenshot", `![screenshot](${origin}/api/bug-reports/${id}/screenshot)`);
    if (r.log) {
        // GitHub caps an issue body at 65,536 characters: keep the end of the log
        const room = 60000 - parts.join("\n\n").length;
        const log = r.log.length > room ? "…\n" + r.log.slice(r.log.length - room) : r.log;
        parts.push("<details><summary>iRice log (scrubbed)</summary>\n\n" + fenced(log) + "\n\n</details>");
    }
    parts.push(`<sub>Sent from iRice · report ${id.slice(0, 8)}</sub>`);
    return parts.join("\n\n");
}

async function limited(env, ip) {
    const hour = Math.floor(Date.now() / 3600000);
    const bucket = await sha256((env.RATE_SALT ?? "irice") + "|" + ip + "|" + hour);
    await env.DB.prepare("DELETE FROM rate WHERE hour < ?").bind(hour - 24).run();
    const row = await env.DB.prepare("INSERT INTO rate (bucket, hour, count) VALUES (?, ?, 1) ON CONFLICT(bucket) DO UPDATE SET count = count + 1 RETURNING count").bind(bucket, hour).first();
    return (row?.count ?? 1) > PER_HOUR;
}

async function fileIssue(env, r, id, origin) {
    if (!env.GITHUB_TOKEN)
        return null;
    const res = await fetch(`https://api.github.com/repos/${env.GITHUB_REPO ?? REPO}/issues`, {
        method: "POST",
        headers: {
            Authorization: `Bearer ${env.GITHUB_TOKEN}`,
            Accept: "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "irice-bug-reports",
            "Content-Type": "application/json",
        },
        body: JSON.stringify({ title: quiet(r.title), body: issueBody(r, id, origin), labels: ["bug-report"] }),
    });
    if (!res.ok)
        return null;
    return (await res.json()).html_url ?? null;
}

async function create(request, env) {
    const length = Number(request.headers.get("Content-Length") ?? 0);
    if (length > MAX_BODY)
        return fail(413, "The report is too large. Untick the screenshot or the log, then send again.");
    const text = await request.text();
    if (text.length > MAX_BODY)
        return fail(413, "The report is too large. Untick the screenshot or the log, then send again.");
    let r;
    try {
        r = validate(JSON.parse(text));
    } catch (e) {
        return fail(400, e instanceof SyntaxError ? "The report isn't valid JSON." : e.message);
    }
    const ip = request.headers.get("CF-Connecting-IP") ?? "unknown";
    if (await limited(env, ip))
        return fail(429, "Too many reports from this network. Wait an hour, or open it on GitHub instead.");
    const id = newId();
    await env.DB.prepare("INSERT INTO reports (id, created, title, happened, steps, expected, version, sysinfo, log, screenshot, screenshot_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id, Date.now(), r.title, r.happened, r.steps, r.expected, r.version, JSON.stringify(r.sysinfo), r.log,
            r.screenshot ? r.screenshot.bytes : null, r.screenshot ? r.screenshot.type : null)
        .run();
    const issue = await fileIssue(env, r, id, new URL(request.url).origin).catch(() => null);
    if (issue)
        await env.DB.prepare("UPDATE reports SET issue_url = ? WHERE id = ?").bind(issue, id).run();
    return json({ ok: true, id, issue_url: issue }, 201);
}

async function screenshot(id, env) {
    if (!/^[0-9a-f]{32}$/.test(id))
        return new Response("Not found", { status: 404 });
    const row = await env.DB.prepare("SELECT screenshot, screenshot_type FROM reports WHERE id = ?").bind(id).first();
    if (!row?.screenshot)
        return new Response("Not found", { status: 404 });
    return new Response(new Uint8Array(row.screenshot), {
        headers: { "Content-Type": row.screenshot_type, "Cache-Control": "public, max-age=31536000, immutable", "X-Content-Type-Options": "nosniff" },
    });
}

export default {
    async fetch(request, env) {
        const url = new URL(request.url);
        if (url.pathname === "/api/bug-reports") {
            if (request.method !== "POST")
                return fail(405, "Send reports with POST.");
            return create(request, env);
        }
        const m = url.pathname.match(/^\/api\/bug-reports\/([^/]+)\/screenshot$/);
        if (m && request.method === "GET")
            return screenshot(m[1], env);
        return new Response("Not found", { status: 404 });
    },
};
