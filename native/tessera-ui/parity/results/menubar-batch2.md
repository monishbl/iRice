# Menu bar batch 2: tester results (arena 3, build a210899, Material dark, standalone Rust)

Scope covered: Material look, Rust only. Not run (host load average 40+ made every pointer step take minutes;
arena stopped): Glass look, QML timing comparison, IslandEvent card x, all of MenuBar.qml (tips, right-click,
privacy, tray, space menu, stable figures). Shots: scratchpad/t/out/*.png (e1c, j-all, k-escc, o-all, ar-all, ao1c, al-all, aw-all).
Harness notes: vptr starts each process at screen centre (chain `glide` hover + click in ONE call); a call
notification survives a shell reload (decline it first).

## IslandCall
- Call card (initials, name, "Signal · incoming call", Decline red / Answer primary, rings): PASS. Rings visibly pulse (faint). Badge shows "i" where QML (first QML shot) shows Signal's chat icon: minor DIFF.
- Answer: card goes, notify-send --wait prints `answer`: PASS
- Decline: card goes, prints `decline`: PASS
- Scroll over card: stays: PASS
- Click name then Esc: declines (needs a hovered glide-in, then click): PASS
- Glass look: not run.

## IslandFocusCard (own fakeplayer copy with Seek/Raise handled; stock one ignores Seek)
- Seek bar advances, elapsed/-remaining: PASS
- Click 50 %: jumps to 1:46: PASS. Drag: FAIL (below).
- Speaker pill widens (~230 ms measured incl. glide), slider click mid = 0.50, wheel up +0.05, down -0.05, mute -> 0, unmute -> 0.5: PASS
- Prev / next / play first click each: PASS
- Open-player widens to "Test Player"; click calls Raise and closes: PASS

## IslandRecent
- List under status line, newest first, Clear, "now"/"1m": shows, but layout FAIL (below)
- Hover line: time -> x, row highlight: PASS. Click x: slides right + fades, rows move up: PASS
- Drag 100 px left removes line: PASS. 30 px springs back: PASS
- Clear empties and hides list: PASS
- Click download line runs remembered Open: INCONCLUSIVE (demo Open is a no-op); row stayed, no error
- Top-left Recent button (with timer): FAIL, unreachable (below)

## IslandEvent / MenuBar
- Esc on ringing call: PASS (see IslandCall). Esc on demo alerts, two-alert order, card x: not run (alerts stuck, see FAIL 1).
- MenuBar.qml: not run.

## FAILs
1. After any click on the island while alerts show (e.g. click the alert, or Decline), then moving the pointer away: alerts never expire (>60 s), and the card x stays visible. Re-hover and leave does not free them. Fresh shell: demo wifi expires in <6 s.
2. Recent rows: the 24 px badge is drawn in the middle of the row on top of the detail text ("Drive conn[badge]ected"); the left of the row is empty and the title starts at x+34.
3. Top corner buttons (Recent clock at ~(783,20), pin at ~(1138,20)): moving the pointer onto them (from inside the card) collapses the island to the pill, so they can't be clicked. Mid-card (790,40) stays open.
4. Seek drag accumulates: relative Seek is sent per move with latency; drag 50->60 % ended ~139 s (expected ~128), drag 25->75 % ended at track end. Seek on release / SetPosition would fix it.
5. Demo alerts with buttons (download, usb) never left on their own in one run (QML: 3 s default); possibly the same cause as 1, not isolated.
