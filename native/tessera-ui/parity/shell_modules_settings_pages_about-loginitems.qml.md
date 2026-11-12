# shell/modules/settings/pages/about-loginitems.qml ↔ surfaces/settings (src/pages/about/loginitems.rs `page`, ui/settings-about.slint item/hit/search)

Login Items (page `about/loginitems`, title "Login Items"): XDG autostart as
xdg-autostart.py sees it, run by Rust itself (off the UI thread). Popup: the
per-item ActionMenu (Start now, Remove).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | List on open (`list`); errors: unreadable JSON / Python missing (82–103, 124) | loginitems.rs 197–201 | same |
| 2 | Script calls **one at a time, queued**; each answer refreshes the list (49–66, 104–123) | `call` 63–69: each call its own thread, at once; answers 101–152 | different (small): changes run concurrently (a quick toggle + Undo can race) |
| 3 | Changes shown at once (patch) before the script writes (77–80) | patch 71–75, shown 43–59, 206–212 | same |
| 4 | setOn: toast "Turned on/off <name>" with Undo (quiet back); failure toast (127–135) | set_on 89–92, answers 113–121 | same |
| 5 | addApp: clears search, closes picking; toasts already-there / Added (Undo removes) / failure (136–147) | add_app 94–98, answers 122–131 | same |
| 6 | removeItem: gone at once; toast "Removed <name>" with Undo (restore; failure toast) (148–159) | menu 172–175, answers 132–144 | same |
| 7 | startNow: toast Started / Couldn't start (160–162) | 171, 145 | same |
| 8 | Item row 64/52: icon 32/28 at 20/14 (0.45 when skipped), name 16/body (secondary when skipped), status line 14/caption; switch (not skipped) "Start <name> when you log in"; dots IconButton at right 12/8 (171–227) | item_row 155–191; slint item 461–522 | same |
| 9 | Dots button: **tooltip "Options for <name>"**, icon label colour while its menu is open, else tertiary (218–225) | slint 505–519 (28 px, hover fill, tertiary always) | different (small): no tooltip, no "open" colour |
| 10 | Menu: "Start now" (play, enabled canStart), "Remove" (trash, danger, removable only) (163–168) | 178–182 | same |
| 11 | Status text: From you/System · Skipped: … / Turned off / Starts when you log in / Paused while login items are off (40–47) | status_text 29–39 | same |
| 12 | Master switch "Start them when you log in" + detail; footer by state (230–241) | 219–223 | same |
| 13 | "Loading login items…" text (inset 16/4) until loaded (244–250) | 226–228 `p.text` | same |
| 14 | Error group "Couldn't read your login items" (red), "Try again" → loaded=false, refresh (251–267) | 229–233 | same |
| 15 | Empty: "No login items" + "Add app…" (plus, primary) when not picking (268–281) | 234–237 | same |
| 16 | "Open at login" group (active) + footer (284–293) | 240–248 | same |
| 17 | "Add an app to open at login" + Add app… (loaded, no error, not empty, not picking) (301–314) | 251–253 | same |
| 18 | Picking: **the search field gets focus at once** (297–300) | — | **missing**: the field isn't focused when picking starts |
| 19 | "Search apps": field 240 "App name", Enter adds the first match, **Esc stops picking (text kept)**; Cancel clears and stops (323–344) | 270–284; slint search 662–680 (Esc = Cancel: clears) | same at a glance |
| 20 | Hint row (empty query, secondary); "No apps match “q”" + detail (345–354) | 300–303 | same |
| 21 | Up to 8 matches (not noDisplay, not already items, name or id contains q, sorted by name): row 56/48, icon, name, Glass hover/press fill, click or "Add" adds (355–391) | 255–297; slint hit 525–560 | same |
| 22 | "Not started by Tessera" group + footer (395–404) | 310–318 | same |

## Test plan

Setups as in pages_menubar. `open about/loginitems`; read y from a shot.
Flip a switch only on an item and **put it back with the toast's Undo**; never
Remove, never "Start now", never add an app for real (search, then Cancel).

LI1 `shot login`. PASS: same groups, rows, icons, status lines, footers.

LI2 Dots: hover a row's dots 1 s, `shot login-hover`. PASS (QML): tooltip
"Options for <name>"; Rust: note. Click it, `shot login-menu`. PASS: menu with
Start now (greyed if it can't), Remove only for yours; the dots turn label
colour in QML while open. Esc.

LI3 Switch + Undo: click an item's switch; `shot login-toast`. PASS: status
"… · Turned off" at once and toast "Turned off <name>" with Undo; click Undo
within 5 s → back on (quietly).

LI4 Add app…: click it, then type at once `run 'key -t sync'` without
clicking the field. PASS (QML): text lands in the field and matches show;
Rust: write whether it does. Then Cancel.

LI5 Esc in the field: Add app…, click the field, type "ma", Esc. PASS: the
search closes in both.

## Test results

(tester)
