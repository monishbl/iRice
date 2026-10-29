.pragma library
// Battery & Power decisions (Power.qml), pure so the logic tests run in Node
// (tests/power_policy.test.cjs).

// ── Energy mode: one answer from everyone who asks ──────────────────────
// A request: { owner, mode: "power-saver" | "balanced" | "performance",
// priority, since (ms), until (ms, 0 = until withdrawn), why }.
// The highest priority wins; among equals, the newest. So:
//   40  Cool-down: the CPU is hot. Safety first, over everything.
//   30  Battery saver: on battery at or below its level.
//   20  Your choice, and Game Mode: whichever came last.
// An expired request counts as gone. With no request at all: balanced.
const PRIORITY = { cooldown: 40, battery: 30, manual: 20, game: 20 };

function resolve(requests, now) {
    let best = null;
    for (const r of requests ?? []) {
        if (!r || !r.mode || (r.until > 0 && r.until <= now))
            continue;
        if (!best || r.priority > best.priority || (r.priority === best.priority && r.since > best.since))
            best = r;
    }
    return best ?? { owner: "default", mode: "balanced", priority: 0, since: 0, until: 0, why: "" };
}

// The mode a request asks for, made to fit what this computer has (no
// Performance profile: balanced instead)
function fit(mode, hasPerformance) {
    return mode === "performance" && !hasPerformance ? "balanced" : mode;
}

// ── Critical battery ────────────────────────────────────────────────────
// 0 nothing, 1 low (10%: a warning), 2 very low (5%: urgent, Sleep now),
// 3 critical (3%: sleep, Caffeine or not). Only on battery.
function batteryStage(percent, onBattery) {
    if (!onBattery || !(percent >= 0))
        return 0;
    return percent <= 3 ? 3 : percent <= 5 ? 2 : percent <= 10 ? 1 : 0;
}

// ── Lid ─────────────────────────────────────────────────────────────────
// What closing the lid does: "sleep" | "lock" | "nothing". On the charger
// with another display connected, its own setting (like a docked laptop).
function lidAction(onAc, externalDisplays, cfg) {
    if (onAc && externalDisplays > 0)
        return cfg.lidExternal ?? "nothing";
    return (onAc ? cfg.lidAc : cfg.lidBattery) ?? "sleep";
}
// iRice only takes the lid over from logind (an inhibitor) when some setting
// isn't logind's own default: sleep, except docked
function lidNeedsInhibit(cfg) {
    return (cfg.lidAc ?? "sleep") !== "sleep" || (cfg.lidBattery ?? "sleep") !== "sleep" || (cfg.lidExternal ?? "nothing") !== "nothing";
}

// ── Idle ────────────────────────────────────────────────────────────────
// The hypridle listeners for both power sources: one per distinct (timeout,
// action), each told which source it's for (idle-action.sh skips it on the
// other). Seconds; 0 = never. Each step is clamped to come after the one
// before it on the same source (dim ≤ lock ≤ display off ≤ sleep).
function idleListeners(cfg) {
    const out = [];
    for (const src of ["ac", "battery"]) {
        const s = src === "ac" ? "Ac" : "Battery";
        let last = 0;
        for (const action of ["dim", "lock", "off", "sleep"]) {
            const key = "idle" + action.charAt(0).toUpperCase() + action.slice(1) + s;
            let t = Math.max(0, Math.round(cfg[key] ?? 0));
            if (t === 0)
                continue;
            t = Math.max(t, last);
            last = t;
            out.push({ source: src, action: action, timeout: Math.max(10, t) });
        }
    }
    return out;
}
