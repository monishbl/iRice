.pragma library
// Decisions behind focus that follows the pointer (Mouse.qml), pure so the
// logic tests run in Node (tests/focus_decision.test.cjs).

// After a burst of Hyprland events (collected for a moment, then decided
// once): what to do with the pointer.
//   { kind: "warp", address }  a window just opened and has focus: the
//                              pointer moves onto it (pointer-focus.lua)
//   { kind: "refocus" }        the scene changed under a still pointer (a
//                              space switched, a window closed or moved):
//                              focus goes to what's under the pointer again
//   { kind: "none" }           nothing that needs either
// A window brought forward some other way (an app asking, the dock, the
// switcher) is the plugin's business: it moves the pointer onto it right
// away (pointer-follow), so a refocus after that lands on that window.
const sceneChanges = ["workspace", "workspacev2", "activespecial", "activespecialv2", "closewindow", "movewindowv2", "changefloatingmode"];

function afterEvents(events) {
    let opened = "";
    let changed = false;
    for (const e of events ?? []) {
        if (e.name === "openwindow") {
            const address = String(e.data ?? "").split(",")[0];
            if (/^(0x)?[0-9a-f]+$/i.test(address))
                opened = address.startsWith("0x") ? address : "0x" + address;
        } else if (sceneChanges.includes(e.name)) {
            changed = true;
        }
    }
    if (opened !== "")
        return { kind: "warp", address: opened };
    return changed ? { kind: "refocus" } : { kind: "none" };
}

// A window asking for attention (Hyprland's "urgent": it asked to come
// forward without the right to). Right after the user clicked or typed
// (opening a link, "Open in the app"), that's the user's own request: bring
// it forward. Otherwise (a chat app pinging by itself) just show that it
// wants attention, without taking focus from what they're doing.
//   inputAgeMs: ms since the last key or button press (Infinity: unknown)
function onUrgent(inputAgeMs, alreadyActive) {
    if (alreadyActive)
        return "none";
    return Number.isFinite(inputAgeMs) && inputAgeMs >= 0 && inputAgeMs <= 3000 ? "bring" : "notify";
}
