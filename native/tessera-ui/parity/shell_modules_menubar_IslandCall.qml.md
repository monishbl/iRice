# shell/modules/menubar/IslandCall.qml

A ringing call in the open island: caller photo or initials with a pulsing
ring, name, app, round Decline (red) and Answer / Open (green) buttons.
Material inline. Rust: none. The ringing event is posted by
`host/desktop/src/today/calls.rs` (301-305, `kind: "ringing"`, `call: {name,
app, appIcon, photo, initials, openOnly, canAnswer}`); the island keeps it
(sticky, `src/island.rs` 42-44) and opens 440 px wide (`has_card`, 158-160)
but draws it as a generic `AlertView` row: bell glyph, name, app, no buttons.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Height 68 | 27 | island.slint AlertView `a.kind == "ringing" ? 68px` | same (fixed batch 2; re-checked 17 Nov) |
| Stagger: photo scale 0.7→1 at 0.1, name fades / slides 12 px at 0.15, buttons fade / scale 0.8→1 at 0.3 | 29-34, 43, 121-122, 156-157 | AlertView ringing `p1`/`p2`/`p3` (315 ms OutCubic after 70 / 105 / 210 ms); buttons `CallButton.grow` 0.8 + 0.2·p3 | same (fixed batch 3 + 17 Nov: the buttons' 0.8→1 scale) |
| Pulse: two rings swell to 1.45× and fade (0.55→0), 1.6 s a beat, half a beat apart, 30 fps Ticker; held when unseen or reduce motion | 47-66 | AlertView ringing `for i in reduce-motion ? 0 : 2` (animation-tick, 1.6 s, +0.5) | same (17 Nov: fade from 0.55, was 0.5); runs on the frame clock while shown |
| Reduce motion: one still ring at 0.6 | 67-76 | AlertView ringing `if reduce-motion` (60 px, alpha 0.6) | same (fixed 17 Nov; was a swollen ring at 0.27) |
| Photo 52 px round (crop), else initials 19 DemiBold on #636366 (Material tertiaryContainer), else a person icon | 78-107 | AlertView ringing photo | same (17 Nov: 19 px on #636366, was 20 on #8E8E93; the person icon added) |
| App badge 22 px on the photo | 109-114 | AlertView ringing `a.has-app-icon` Image 22 | same (fixed batch 2) |
| Name 17 DemiBold | 123-129 | AlertView ringing Text 17/600 | same (fixed batch 2) |
| App icon 14 + "App · incoming call" / "· on your phone" (body, ink2) | 130-148 | AlertView ringing subtitle row | same (17 Nov: the 14 px app icon before the text) |
| Decline: 44 px red circle, phone-disconnect, label under; Calls.decline() | 158-164 | `CallButton` Decline → `calls decline` | same (fixed batch 2) |
| Answer / Open (openOnly): 44 px green (Material primary), phone / arrow-square-out; Calls.answer() | 165-171 | `CallButton` Answer/Open → `calls answer` | same (fixed batch 2) |
| Button states: Glass darker 1.2 pressed, lighter 1.1 hovered, scale 0.94 pressed (100 ms OutCubic); Material state layer + ElasticControl | 182-199 | `CallButton` | same (Glass); Material: grows 6 % pressed (ElasticControl's press, 17 Nov), tonal state colours as Glass's |
| 52 px hit target (margins −4), pointer cursor; a11y "Decline call" / "Answer call" | 207-215 | `CallButton` TouchArea −4, `accessible-label: label + " call"` | same (fixed batch 2) |
| Not dismissable by drag or wheel (IslandEvent 133, 149) | — | the ringing card has no wheel area | same (fixed batch 2) |
| Esc declines (IslandView Keys / ipc dismissNewest) | Island.qml 705-707 | island.slint `keys` FocusScope → lib.rs `M::IslandEscape` → `calls decline` | same (fixed batch 2) |

## Summary

1. A ringing call can't be answered or declined from the Rust island (no buttons).
2. A wheel / swipe up removes the call card without declining.
3. No photo / initials / app badge; generic bell row; no pulse.
4. Esc doesn't decline (only `qs ipc call island dismissNewest`).

## Fixed in batch 2 (fixer, 7 Nov)

- The ringing call has its own card (`AlertView`, `if a.kind == "ringing"`), 68 px: the photo (or initials on grey / Material tertiary container) at 52 px with the app's badge (22 px), two rings swelling out of it 1.6 s a beat, half a beat apart (Glass green, Material primary), the name (17/600), "App · incoming call" ("· on your phone" for an Open-only call), round 44 px Decline (red; Material error) and Answer (green; Material primary; Open with arrow-square-out when openOnly) with their labels, lighter on hover, darker and 94 % on press (100 ms).
- Answer / Decline → `calls answer|decline` (host-desktop's Calls: the notification's own action, the ringtone stops).
- A wheel / swipe no longer removes the call (it isn't an ordinary alert row any more).
- Esc declines it once the island has the keyboard (a click on the island while it shows alerts gives it the keyboard until the pointer leaves: MenuBar.qml's Exclusive while `wantsKeys`).
- Not done: the stagger-in of photo / text / buttons (IslandEvent's clock), reduce-motion's still ring.

## Test plan (Sonnet tester)

1. Make a ringing call: in the arena, `notify-send -a "Signal" -h string:category:call.incoming -A answer=Answer -A decline=Decline "Maya" "Incoming call"` (Calls routes a notification categorised as a call to the island). Must see the call card: initials "M" (or a photo), "Maya", "Signal · incoming call", Decline (red) and Answer (green) circles with labels; two rings pulsing out of the photo.
2. Click Answer: the card goes, the ringtone stops; the notification's "answer" action fires (run notify-send with `--wait` in another try to see it print `answer`).
3. Ring again, click Decline: the card goes; `decline` fires.
4. Ring again, scroll up over it: it stays.
5. Ring again, click on the card's name (not a button), then `dev run wtype -k Escape`: declined.
6. Both looks; Material: Answer in primary, Decline in error, rings in primary.

## Test results

## Batch 3

The call card staggers in (photo 70 ms, text 105 ms, buttons 210 ms; 315 ms OutCubic each): see IslandRecent.qml.md batch 3 test plan (Call).

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Badge "i" (batch 2 FAIL 5): the call's appIcon is now also read as a path / file:// URL and tried lower-cased in the icon theme.

Retest the FAIL steps above.

## Fixed in batch 4 (fixer, 7 Nov)

- Reduce motion (appearance.reduceMotion): one still ring instead of two pulsing.

## Test plan (batch 4)

- Set `appearance.reduceMotion: true`, ring again: one still ring around the photo.

## Test results (batch 4)

## Re-check 17 Nov (B4)

Every row re-read against `surfaces/menubar/ui/island.slint` at 18628a5: the table above still said "missing" for what batch 2/3 had fixed. Ported now: the buttons' scale-in, the 0.55 ring fade, reduce motion's still ring, initials 19 on #636366, the person icon, the subtitle's app icon, Material's press growth. Before: 8 missing, 6 different; after: 14 same.
