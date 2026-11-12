# shell/modules/settings/pages/accessibility.qml ↔ surfaces/settings (src/pages/system/access.rs `accessibility`)

Accessibility (page `accessibility`): live captions and a link to Shake to
find the pointer. One popup: the Language **Dropdown** (More options).
State: feed.system.captions (RustBridgeSystem.qml).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On open Dictation.refresh() (15) | RustBridgeSystem.qml 30–31 | same |
| 2 | Group "Live captions" + footer (Tiny model, 2 s behind, Super+Ctrl+L) (18–21) | access.rs 95–96 | same |
| 3 | "Speech recognition" (no binary): detail installError or install text, red on error; Install (download-simple, primary, busy, disabled while updates run) → Captions.install() (24–37) | 49–59 | same |
| 4 | "Tiny speech model" (binary, no model): "Downloading… N%" / downloadError (when last download tiny) / "75 MB…"; button Cancel (no icon, not primary) while downloading else Download (icon, primary), disabled when another Dictation download runs (38–51) | 60–77 | same |
| 5 | "Live captions": detail (needs binary / model / error / Off / per placement); label red on error; Segmented 380 Off/Floating/Over video/Island/Bottom, current 0 when off else max(1, index); enabled while ready or on; pick >0 sets placement, enabled = i>0 (53–74) | 78–94, captions_detail 14–32 | same |
| 6 | Group "Pointer": link "Shake to find the pointer" (cursor icon), detail "In Mouse & Trackpad", value Off/Bigger/Ring → mouse page, target row (78–89) | 101–111 | same |
| 7 | More options › "Live captions": "Language" Dropdown 220: Automatic + Dictation.languages (not ""/auto), value cfg.language or auto (96–105) | 115–141 (`.choice(..).width(220)`, placeholder when unknown) | same |
| 8 | "The bar's place" (placement bar and moved): "Put It Back" → barX = barY = −1 (106–117) | 132–141 | same |

## Test plan

Setups as in pages_menubar. `open accessibility`; read y from a shot. Never
press Install or Download (they install packages / fetch a model).

A1 `shot acc-top`; open More options (click it, y from shot), `shot acc-more`.
PASS: same rows/texts/footers; Segmented greyed when not installed in both.

A2 If the Segmented is enabled (installed): click "Island" then "Off".
PASS: detail "In the island at the top…" then "Off"; config captions.enabled
false and placement stays "island". Leave it as it was.

A3 Language Dropdown: click it, `shot acc-lang` (list open), Esc. PASS: the
list has "Automatic" first; QML item heights 36/28; compare Rust.

A4 Pointer link: click it. PASS: Mouse & Trackpad opens scrolled to "Shake to
find the pointer"; value text equals that page's style.

## Test results

(tester)
