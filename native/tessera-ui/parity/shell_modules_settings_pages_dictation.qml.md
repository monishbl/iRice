# shell/modules/settings/pages/dictation.qml ↔ surfaces/settings (src/pages/input/dictation.rs)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Dictation, a part of Keyboard (page `shortcuts`, PagePart "Dictation"). One
popup: the Language Dropdown (Rust `Row::choice`). No timers or animations.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Dictation.refresh() each time the page is made (Component.onCompleted) (15) | dictation.rs 56–59: ask `dictRefresh` once per `dict.refreshed` local | **same (fixed before 17 Nov: dict.refreshed in PAGE_TRANSIENT: refreshed every time the page is made)** [12 Nov: **different**: once per Settings window (local survives page switches, lib.rs 581); QML refreshes every time Keyboard is opened] |
| 2 | Footer "Press Super+H, speak, then press it again…" (17–19) | 69–70 | same |
| 3 | "Speech recognition" (only without the binary): detail installError or "Install whisper.cpp…"; label red on an error; primary "Install"/"Installing…" (download-simple), busy while installing, disabled while Updates.running → Dictation.install() (22–35) | 71–75 `.tone(2)`, `.buttons([btn.icon.primary.on.off])`, ask dictInstall | same |
| 4 | "Typing into apps needs wtype" (binary, Tools ready, no wtype) + detail; "Install"/"Installing…" plain, busy, disabled while updating (36–46) | 76–79 | same |
| 5 | Status row (with the binary): "Ready"/"Download a speech model to start", detail "Super+H to dictate"/"Speech recognition is installed", label secondary when not ready; icon check-circle filled green / info secondary, 18 (47–58) | 80–85 (`icon` ctl, style 3 success / 1 secondary, 18 px) | same |
| 6 | Group "Speech model" + footer; a row per Dictation.models: "<label> · in use" for the chosen (Dictation.model: cfg.model if downloaded, else "base"); detail "Downloading… N%" / download error for the last one tried / "<what> · <size> · downloaded" (61–78) | 88–106, 136–142 (`MODELS` tested against Dictation.qml) | same |
| 7 | Buttons, 8 apart: "Use" (have, not chosen) → config model; "Remove" danger (have) → removeModel; "Download" (download-simple; primary when chosen and binary) / "Cancel" (while loading); enabled while loading or nothing downloading (79–105) | 107–135 | same |
| 8 | Group: "Language" Dropdown 220 (Dictation.languages) ↔ dictation.language; "Play sounds" switch + detail (111–128) | 144–156 (`LANGUAGES` tested) | same (list = Dropdown parity) |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open shortcuts`, scroll
to the "Dictation" heading (`run 'P move 1100 600; P scroll 0 40'`), `shot d`.
Read y from the shot. Never let a model download finish over a metered link:
cancel within a few seconds.

D1 Part: PASS: same rows, details, buttons and footers; status row matches
what's installed in the arena.

D2 Download/cancel: click "Download" on Tiny. PASS: detail "Downloading… N%"
counting up, other Download buttons greyed; click "Cancel": back to
"Fastest, makes more mistakes · 75 MB".

D3 Language: click the Language control, shot the list, pick "English":
config dictation.language "en".

D4 Play sounds: toggle; config dictation.sounds follows.

D5 Refresh: open Keyboard, switch to another page, back to Keyboard; check the
shell log for Dictation.refresh (add `run 'grep -c refresh …'` if logged). PASS
(QML): a refresh each time Keyboard opens. Rust must do the same.

## Fixed in batch 3 (fixer)

- Dictation refreshes on every visit (dict.refreshed starts over with the page).

## Test results

(tester)
