# CORE-NEEDS (host/desktop)

---

# CORE-NEEDS (wm)

1. **A surface for the find-pointer ring** (FindPointer.qml's overlay): a click-through overlay
   layer on the display under the pointer, a 150 px accent ring (12% fill, 3 px border) that scales
   1.6 -> 0.5 over 450 ms (ease-out cubic) with opacity 0 -> 1 in 200 ms, holds 550 ms, fades 300 ms,
   following the pointer. Feed: `{"to":"findpointer","show":true,"style":"ring","x":…,"y":…}` lines
   (global layout coordinates, many per second) and `{"to":"findpointer","show":false}` at the end.
   Where: a new `surfaces/findpointer` crate + name in src/main.rs KNOWN.
2. lib.rs of this crate: `mod wm;` and `wm::services()` appended in `services()`.

---

# CORE-NEEDS (screen group)

Only `src/lib.rs` (the lead's): `mod screen;` and `v.extend(screen::services());` inside
`services()`. Nothing else outside the crate.

---

# CORE-NEEDS (today/)

## 1. DONE: Surface events (stdout lines) must reach the host when standalone

The notifications surface says its list and Notification Center's acts with
`shell::tell("notifications", {list})` / `tell("nc", {act,…})`, which only
writes to stdout. Standalone nothing reads it, so the host never sees new
notifications (no unread count, no island card, no call card) and the center's
to-dos/timer/calendar buttons do nothing. Same for any `tell` kind a host
service wants.

Where: `core/src/shell.rs`, `pub fn tell`. Add at the top, before the stdout write:

```rust
pub fn tell(kind: &str, fields: serde_json::Value) {
    // (standalone: the host answers these, crate::host)
    if matches!(kind, "notifications" | "nc") && crate::host::route_ask(kind, &fields) {
        return;
    }
    ...
}
```

`route_ask` sends `Ask { call: kind, args: fields }`, and the `Today` service
answers the prefixes `notifications` (args = `{"list":[…]}`) and `nc`
(args = `{"act":…}`), with `Hub::call`. Other kinds the host-shell wants
(`closed`, `osd`, `call`) can be added to the list by that agent.

## 2. host-shell: island actions for alerts made here

Alerts posted by this group have uids from 1_000_000_000 (`common::UID_BASE`).
In the island's `rustui act islandAct "<uid> <which>"` handler, for
uid >= 1_000_000_000 call `today.islandAct` with `["<uid> <which>"]`
(`h.ask("today.islandAct", json!([text]))`) instead of looking up an Island
event. The ringing card's Answer/Decline: `calls answer` / `calls decline`
(ipc target `calls`).
