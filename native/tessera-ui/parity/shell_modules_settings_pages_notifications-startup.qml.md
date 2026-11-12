# shell/modules/settings/pages/notifications-startup.qml ↔ surfaces/settings (src/pages/notify/notifications.rs `startup`)

Sound when you log in (page `notifications/startup`). No Popup, no Timer.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Page title "Sound when you log in" (16) | `p.title` 250 | same |
| 2 | current = "file" when startupFile is set, else startupSound (17) | 251 | same |
| 3 | Group "Original" + footer "Any of these can play when you log in, whichever sound style you use." with the Tessera choice ("The chime Tessera has always played") (48–53) | 268–269 | same |
| 4 | A group per style in StartupSounds.styles order (Soft, Marimba, Keys, Taps, Minimal, Glass, Material), 4 choices each (Rise/Hello/Dawn/Bell names and "about" lines) (54–66) | `STYLES`, `startup_sounds` (tested against the JS) | same |
| 5 | Each row: "Play" (icon play) → Sounds.playFile(the id's file, at startupVolume); 8 px; "Use" primary, or "In use" with a check, disabled (31–45) | 253–266 `buttons` → ask notify playStartup → `sound.playStartup` | same |
| 6 | Use: startupSound = id, startupFile = "", then plays "startup" at once (19–23) | 260–262 | same at a glance (plays 450 ms later, after the save: `play_when_still`) |
| 7 | After choosing, that row turns "In use" and the previous one "Use" (live) | rebuild | same |

## Test plan

Setups as in pages_menubar (both shells, both looks). `open notifications/startup`;
read y from a shot.

S1 Page: shot top, scroll and shot to the end. PASS: Original first, then the
7 style groups in the same order with the same 4 names each, "In use" on the
current one.

S2 Use: click "Use" on "Glass › Prism". PASS: it turns "In use" (check icon,
greyed), the old one "Use"; config sounds.startupSound "glass-hello",
startupFile ""; the log shows a startup play (note the delay in each shell).

S3 Play: click Play on "Keys › Lounge". PASS: a play of keys-hello logged, nothing saved.

S4 Back: the Back arrow / Esc returns to Notifications & Island with "Sound when
you log in" detail "Prism · Glass".

## Fixed in batch 2 (fixer)

- Its volume slider plays the sound as it's let go.

## Test results

(tester)
