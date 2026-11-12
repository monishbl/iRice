# shell/modules/settings/pages/controlcenter.qml ↔ surfaces/settings (src/pages/look/menubar.rs `controlcenter`)

Quick Settings, a part of Menu Bar & Dock (`open controlcenter` lands there).
No Popup, no Timer.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "Layout" with the long footer ("Open Quick Settings and choose Edit Controls …Anti-Flash softens very bright screens.") (13–14) | menubar.rs 303–304 | same |
| 2 | "Edit Controls": detail "N controls, arranged by you" / "The default arrangement" (16–17) | 305 | same |
| 3 | "Edit…" button: accent fill, Material 40 px (+32), Glass 28 px (+24), small DemiBold / 14 Medium, pointing hand; click: Settings closes, Quick Settings opens in edit mode (Panels.settingsOpen=false, ccEditing=true, show controlCenter) (18–33) | `button("Edit…").primary()` → `ask("look","editControls")` → host/shell bridges.rs 119 (and RustBridgeLook.qml 79) | same |
| 4 | "Panel background" (Glass only): detail "Always on with Solid glass" / "Controls sit on one glass panel" / "Each control is its own glass chip"; switch on when background or tier solid; only the switch disabled with Solid (35–44) | 306–310 `.enabled(!solid)` | same (Rust's row-enabled only reaches the control) |
| 5 | "Reset to the default layout" only when a layout is saved; its control is a **text link**: "Reset" in accentInk, body size, DemiBold, no background, pointing hand; click → layout [] (45–58) | 311 `button("Reset").minor()` | **different**: Glass minor SButton is a grey fill pill with label-coloured small text; Material minor is a primary text button (40 px, 14 px Medium) – near, but QML is accentInk body DemiBold in both looks |

## Test plan

Setups as in pages_menubar. `open controlcenter`; read y from a shot.

Q1 Look: shot the Layout group in both looks. PASS: Glass shows "Panel
background", Material doesn't; Edit… is an accent pill.

Q2 Edit: click "Edit…". PASS: the Settings window closes and Quick Settings
opens in its edit mode (shot); both shells alike. Esc / Done there.

Q3 Reset: make a layout (`run 'jq ".controlCenter.layout=[\"wifi\",\"bluetooth\"]" …'` or
move a control in edit mode), `open controlcenter`. PASS: detail "2 controls,
arranged by you", the "Reset to the default layout" row shows with "Reset" as
accent-coloured text (no fill) in both looks; click it: the row goes, detail
"The default arrangement".

Q4 Solid glass (Glass): set the glass tier to Solid in Appearance, come back.
PASS: switch on and greyed, detail "Always on with Solid glass", label not greyed.

## Fixed in batch 2 (fixer)

- "Reset to the default layout": a text link (SButton style 4: no background, accent ink, body 600) in both looks.

## Test results

(tester)
