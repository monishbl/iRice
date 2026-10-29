.pragma library
// Nearby devices from both protocols as one list (Sharing.devices). A phone
// running LocalSend that also has Quick Share open shows up twice, once from
// each; it's one device to you, so the two are merged by name. Each entry
// keeps how to reach it both ways: Android phones go over Quick Share (no
// app to open on the phone), everything else over LocalSend.
//   merge(LocalSend.peers, QuickShare.devices)
//   -> [{ key, name, type, via: ["quickshare", "localsend"], ids: { quickshare, localsend }, prefer }]

// "Pixel 8", "pixel-8 " and "PIXEL 8" are the same device
function nameKey(name) {
    // (spaces and punctuation out; no \p{…} classes: Qt's engine lacks them)
    return String(name ?? "").toLowerCase().replace(/[\s\-_.,:;'’"()\[\]]+/g, "");
}

function typeOf(t) {
    // Quick Share says "phone"; LocalSend "mobile". The sheet's icons use LocalSend's words
    if (t === "phone")
        return "mobile";
    return t || "desktop";
}

function merge(localsend, quickshare) {
    const out = [];
    const byName = {};
    const add = (name, type, via, id) => {
        const k = nameKey(name) || via + ":" + id;
        let d = byName[k];
        if (!d) {
            d = { key: k, name: String(name ?? "").trim() || "Unknown device", type: typeOf(type), via: [], ids: {} };
            byName[k] = d;
            out.push(d);
        }
        if (!d.via.includes(via))
            d.via.push(via);
        d.ids[via] = id;
        // A more specific kind wins (a phone over a generic desktop)
        if (d.type === "desktop" && type)
            d.type = typeOf(type);
    };
    for (const p of localsend ?? [])
        add(p.alias, p.type, "localsend", p.id);
    for (const q of quickshare ?? [])
        add(q.name, q.type, "quickshare", q.id);
    for (const d of out)
        d.prefer = d.ids.quickshare !== undefined && (d.type === "mobile" || d.type === "tablet" || d.ids.localsend === undefined) ? "quickshare" : "localsend";
    return out;
}

// The send in flight to this device, if any: { done, total, waiting }
// (LocalSend and Quick Share both name an outgoing transfer's peer)
function progressFor(items, device) {
    const k = nameKey(device?.name);
    for (const it of items ?? [])
        if (it.kind === "send" && nameKey(it.peer) === k)
            return it;
    return null;
}

// The three visibility levels, as one: what both say, or the more open one
// when they differ (the module then says "Custom")
function combine(a, b) {
    if (a === b)
        return a;
    if (a === "everyone" || b === "everyone")
        return "everyone";
    if (a === "10min" || b === "10min")
        return "10min";
    return "off";
}
