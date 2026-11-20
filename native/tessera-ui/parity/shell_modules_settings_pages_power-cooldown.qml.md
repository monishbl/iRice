# shell/modules/settings/pages/power-cooldown.qml ↔ surfaces/settings (src/pages/system/power.rs `cooldown`)

Cool-down and performance (subpage power/cooldown). Live state: QML reads
CoolDown.active / CoolDown.why; Rust reads `feed.system.cooldown {active, why}`
and toggles with `ask system coolDownToggle` (→ CoolDown.toggle / `cooldown.toggle`).
No popups on this page.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Page title "Cool-down and performance" (12) | power.rs:282 `p.title` | same |
| 2 | Group "Cool-down" with footer "Less work for the processor …" (14–16) | power.rs:301–302 | same |
| 3 | Cool-down switch = CoolDown.active, toggles CoolDown.toggle(); detail "On · <why>" / <why> / "Off" (17–21) | power.rs:285–287, 303 | same |
| 4 | "When the CPU is hot": Off/Ask/Automatic (width 240) from cooling.whenHot + hotAction; Ask/Auto set hotAction, all set whenHot; detail per state (22–40) | power.rs:288–315 | same |
| 5 | "On battery" switch cooling.onBattery (41–44) | power.rs:299–300, 316 | same |
| 6 | More options (48) | `p.more` / `end_more` | same |
| 7 | Glass row: Keep/Frosted/Solid (width 260) cooling.glass, "During cool-down"; hidden in Material (53–63) | power.rs:320–323 `.visible(!cx.material)` | same |
| 8 | Lighter blur switch, hidden in Material (64–68) | power.rs:325, 333 | same |
| 9 | Shorter animations, Lower refresh rate (with its detail), Pause live wallpaper switches (69–81) | power.rs:326–334 | same |
| 10 | Group "Performance": Keep panels loaded (performance.keepPanelsLoaded) with detail (83–94) | — | left out on purpose: nothing in the Rust shell reads it (QML PanelLoader only); Performance shows only for two or more GPUs |

Nothing missing or different found.

## Test plan (both looks; Material window at 440,205, page column from x≈772)

C1 Open: `setup qml material`, `open power/cooldown` (if that id doesn't open it,
`open power` and click the "Cool-down and performance" link), `shot cd-qml-m`; same with
`setup rust material <bin>`. PASS: title + back arrow, the three rows, footer text identical.

C2 Hot action: click "Ask" in "When the CPU is hot" (read its x/y from the shot).
PASS: detail becomes "The island asks first; …"; config.json cooling.whenHot = true,
hotAction = "ask" (`run 'cat ~/.config/tessera/config.json'`). Click "Off": detail
"At the island's temperature alert …", whenHot = false, hotAction still "ask". Restore.

C3 Cool-down switch: click it on, shot, then off. PASS: detail "On · <why>" while on
(same why text in both), "Off" (or the why) after; Quick Settings' Cool-down tile follows.

C4 More options: click "More options". PASS Material: only Shorter animations, Lower
refresh rate, Pause live wallpaper, then "Performance" → Keep panels loaded (no Glass,
no Lighter blur). Glass (`setup … glass`): Glass segmented and Lighter blur also show,
first in the group.

C5 Toggle "Shorter animations" on and off. PASS: config cooling.shortAnimations flips
in both.

## Test results

(tester)
