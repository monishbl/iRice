# Shots

Sources:
- **F**: footage recorded in the hidden sandbox;
- **S**: a still from the sandbox (2×);
- **R**: rebuilt in HTML;
- **A**: abstract.

Match points are measured from the captured frames in the build (boxAt).

## Version A: "Two halves" (calm, split screen)

Transition families (four): the **seam sweep** (the seam moves and re-frames), **match cut** (the same element in the same place), **shared element** (an object crosses the seam), **liquid warp** (two act breaks: after the opener, and before the close).

| # | t | Shot | Source | Camera | Carries out → transition |
|---|---|---|---|---|---|
| A1 | 0–2.8 | Frosted glass (left, refracting a wallpaper) and a tonal Material surface (right, cookie-shaped highlights) drift in and meet. The seam lights; the Badge forms on it. "One desktop." under it | A | slow push +4% | the seam (a line) → it rotates to horizontal |
| A2 | 2.8–6.0 | The seam becomes the menu bar. Pull back: the full desktop, Glass left and Material right, the same wallpaper. "Two looks." | S+S | pull 1.25→1.0 | the seam, still vertical at centre → seam sweep |
| A3 | 6.0–13.0 | The seam sweeps across; behind it Glass becomes Material: the real switch screen, then the settled desktop. "Switch in a second." | F | hold-drift 2% | the menu bar at the top → push in |
| A4 | 13.0–20.0 | Both menu bars cropped 2.5×, side by side: the earbuds alert springs open in both, the rings fill, both settle. "The island comes alive." | F+F | push 2.0→2.5 | the status icons (same place) → match cut |
| A5 | 20.0–27.0 | Both Control Centers open; a slider drags (Material tiles bounce). "Everything in one place." | F+F | hold-drift | a photo on the desktop → whip |
| A6 | 27.0–34.0 | Cause → effect: left (Glass) a scribble circles the lighthouse photo; right (Material) the sidebar opens and the answer streams in. "Circle anything." | F+F | slow push on the left, then right | the sidebar → shared element (it slides across the seam) |
| A7 | 34.0–41.0 | The sidebar fills the frame, in both looks: a formatted answer; the clock opens past chats. "Ask. Keep every chat." | F+F | hold-drift | a screenshot thumbnail → it drops into the next shot |
| A8 | 41.0–47.0 | Quick Preview of a screenshot (a café menu) in both looks: words are dragged over and selected, then Copy. "Copy text from pictures." | F+F | push into the selection 1.0→1.6 | a photo → shared element (it flies into the phone) |
| A9 | 47.0–53.0 | Cause → effect: left, a phone (rebuilt device) taps Share; right, the laptop's Quick Share card counts up and the photo opens. "Phones and laptops, together." | R+F | hold | the wallpaper → match cut |
| A10 | 53.0–58.0 | Both desktops: the wallpaper changes; Material's palette and Glass's accent follow it, twice. "Colours that follow you." | F+F | pull 1.1→1.0 | the whole frame → liquid warp |
| A11 | 58.0–62.0 | The halves slide apart; the seam and the Badge remain. "Tessera", held. The halves drift back: the opening frame (the loop) | A | hold | the loop → frame 0 |

## Version B: "Tiles" (fast, alternating)

Transition families (four): the **tile shatter** (the frame breaks into tiles that re-form as the next shot; between chapters), **jump cut** on the beat (inside chapters), **whip** with motion blur (inside chapters), **kinetic type card** (titles slam in and out on the beat).

| # | t | Shot | Source | Camera | Carries out → transition |
|---|---|---|---|---|---|
| B1 | 0–2.6 | Tiles (glass and tonal) drift on black, rush together, lock into the Badge. "Tessera" | A | punch-in on the lock | the Badge → it shrinks to the menu bar logo |
| B2 | 2.6–5.0 | The login intro: the wallpaper comes into focus, the bar and dock settle (Glass). "One desktop. Two looks." | F | hold | tiles → shatter |
| B3 | 5–11 | Card "The island" → Glass: earbuds alert, rings fill → jump cut: a timer live activity | F | push 2.5× | shatter |
| B4 | 11–17 | Card "Your controls" → Material Quick Settings: springs open, tiles bounce, a slider | F | whip in | shatter |
| B5 | 17–23 | Card "Two looks" → the switch screen, Glass → Material | F | hold, then whip | shatter |
| B6 | 23–29 | Card "Circle anything" → Glass: scribble → the answer in the sidebar | F | punch-in on the scribble | shatter |
| B7 | 29–35 | Card "Ask anything" → Material sidebar: an answer streams → jump cut: past chats | F | hold-drift | shatter |
| B8 | 35–41 | Card "Text in pictures" → Glass Quick Preview: select, Copy | F | push 1.0→1.8 | shatter |
| B9 | 41–48 | Card "Share across" → a phone sends → Material: the card counts up, the photo opens | R+F | whip between them | shatter |
| B10 | 48–54 | Card "Your colours" → Material, three wallpapers in jump cuts on the beat, the colours following | F | hold | tiles → the Badge |
| B11 | 54–60 | The tiles form the Badge. "Tessera", held. The Badge breaks back into drifting tiles (the loop) | A | hold | the loop → frame 0 |

## Capture list (shared by both versions)

| Take | Look | Region | Event | Lead / tail |
|---|---|---|---|---|
| desktop | G, M | full (2× still) | – | – |
| look-switch | G→M | full | Settings → Look → Material | 0.5 / 1.0 s |
| island-earbuds | G, M | top 1920×300 | `ipc island demo headphones` | 0.3 / 1.0 s |
| island-timer | G | top | timer live activity | 0.3 / 0.5 s |
| control-center | G, M | right 700×900 | open; brightness slider glide; one tile | 0.3 / 0.6 s |
| circle | G | full | `ipc screen circle`, a scribble with the pointer | 0.3 / 0.5 s |
| sidebar-answer | M (and G) | right 520×1080 | an answer streams (canned) | 0.3 / 0.5 s |
| sidebar-chats | G, M | right | the clock → past chats | 0.3 / 0.5 s |
| livetext | G, M | centre | Quick Preview of a generated picture; drag-select; Copy | 0.3 / 0.5 s |
| quickshare | M (and G) | top right card | an incoming transfer (demo) | 0.3 / 1.0 s |
| colour | M, G | full | wallpaper changes ×3 | 0.3 / 1.0 s |
| login-intro | G | full | `ipc loginIntro play` | 0 / 0.5 s |

Footage comes from the hidden sandbox only, with seeded demo data (account "Maya", Wi-Fi "Studio", demo earbuds, canned AI answers). Nothing from the real screen.
