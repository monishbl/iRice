# Parity: shell/modules/menubar/RustBridge.qml

Beside the QML shell: the state the Rust bar needs (a "state" line on change,
the island's live activities each second) and the `rustui act <what> <arg>`
IPC the bar calls for what only the shell can do. Standalone, the host
provides both: state fields via `h.set_state(...)` in many host crates, and
`rustui act` in `host/shell/src/panels.rs:521-560` (`rustui_act`); live
activities in `host/shell/src/island.rs:532-567`.
Consumer: `surfaces/menubar/src/lib.rs` (`ShellState` reads; `bridge::call`).
Paths relative to `native/tessera-ui/`.

## State to the bar

| Field | QML line | Rust place (standalone producer) | Status |
|---|---|---|---|
| Resources sampling while the bar shows CPU/memory (`menubarActive`) | 25-30 | host/system/src/lib.rs:99 (resources state) | same (verify sampling is on while `menubar.resources`) |
| open / screen (the open panel) | 36-37 | host/shell/src/panels.rs (sync) | same |
| dark | 38 | host/shell/src/appearance.rs:114 | same |
| locked | 39 | host/shell/src/bridges.rs:327 | same |
| focus {on, icon, name, status} | 40 | host/desktop/src/wm/focusmodes.rs:127-128 | same |
| caffeine {on, status} | 41 | host/system/src/sysinfo/system.rs:168-169 | same |
| unread | 42 | host/desktop/src/today/notif.rs:112 | same |
| recording {on, started} | 43 | host/desktop/src/screen/service.rs:181 | same |
| shelf {on, count, opened, zone} | 44 | host/sharing/src/lib.rs:587 | same |
| audio {icon, volume, muted, sink, micMuted} | 45 | host/system/src/lib.rs:69 | same |
| privacy {any, camera, mic, location, share} | 46 | host/system/src/sysinfo/privacy.rs:227-228… | same |
| weather {ready, icon, temp, condition, high, low, place} | 47 | host/desktop/src/today/weather.rs:38-44 | same |
| resources {cpu, memory} (zeros when the item is off) | 49 | host/system/src/lib.rs:99 | same |
| keyboard {multiple, short, name} | 50 | host/system/src/input/keyboard_layout.rs:55 | same |
| updates | 51 | host/system/src/lib.rs:123 | same |
| battery {summary, lowPower} | 52 | host/system/src/lib.rs:89 | same |
| power {ppd, profile, performance} | 53 | host/system/src/lib.rs:75-76 | same |
| tools {hyprpicker} | 54 | host/shell/src/probes.rs:345 | same |
| wifi {elsewhere, short, linkUp} | 55 | host/shell/src/probes.rs:350 | same |
| island {pinned, hot, fsMode, history} | 56 | host/shell/src/island.rs:717 | same |
| spaces {bases, labels} | 58 | host/desktop/src/wm/spaces.rs:368 | same |
| Sent on change only (JSON compared), coalesced with `Qt.callLater` | 60-74 | host `set_state` (tells only on change) | same |
| Live activities each second while any (`pushLive`, 1 s Timer, on change) | 84-120 | host/shell/src/island.rs:532-567 | same (verify 1 s cadence) |

## `rustui act` (what the Rust bar asks)

| What | QML line | Rust place | Status |
|---|---|---|---|
| toggleAt <panel> <screen> <x> → Panels.toggle (250 ms re-open guard) | 129-130 | host/shell/src/panels.rs:525-528, 97-104 | same |
| page <ccPage> <screen> → Control Center on a page | 131-134 | panels.rs:529-533 | same |
| about / session <screen> | 135-138 | panels.rs:534-537 | same |
| settings, installUpdates, repairShell, power, darkMode, mic, record, stopRecording, shelf, keyboardLayout, powerProfile | 139-162 | panels.rs:538-549 | same |
| launch, quit, mergeTabs, moveSpace, go | 163-172 | panels.rs:550-554 | same |
| islandAct / islandRemove / islandHold / islandPin, live, hud | 173-200 | panels.rs:555-557 | same (behaviour in host island.rs) |
| Unknown → warning | 202 | panels.rs:558 | same |

## Summary (missing / different, by priority)

No behavioural gaps found in the bridge itself; two things to verify in the
sandbox: resources sampling while `menubar.resources` is on (QML Binding 25-30),
and the live activities' 1 s refresh cadence.

## Fixed in batch 5 (fixer, 8 Nov)

- No gaps to fix (every state field and act has its host counterpart); the two things to verify are in the tests below.

## Test plan (batch 5)

- `menubar.resources: true`: the bar's resources item updates once a second. A timer running: the pill's time counts each second.

## Test results (batch 5)
