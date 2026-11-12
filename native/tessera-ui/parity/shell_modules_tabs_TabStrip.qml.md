# shell/modules/tabs/TabStrip.qml → surfaces/tabs (src/lib.rs, ui/tabs.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | A bar over each window group, at the group's x/y/w | 60-110 | `place`, `Strip` | same |
| 2 | Fades in once settled (150 ms), out quickly (60 ms) | 120-123 | `animate opacity { settled ? 150 : 60 }` | same |
| 3 | Bar: Material pill surfaceContainer; Glass r 10, translucent, 1 px border | 125-132 | bar | same |
| 4 | Tabs: icon 14 px + title (class when no title), the front one secondaryContainer (Material) / raised (Glass r 7); dividers between back tabs; hover 8 % / press 10 % | 140-180 | `TbTab` | same |
| 5 | × on a hovered tab closes that window; + opens a new one in the group | 180-230 | `close_tab`, `new_tab` | same |
| 6 | Titles update live | — | `push` | **was** a new model on every title change (a terminal's title changes often), losing the hover on a tab and its ×; now one `Rows`, set in place |

## Test plan (tester)
Both looks. Open two foot windows, group them (`hyprctl dispatch togglegroup` then move the second in), so the bar shows.
1. Shot: two tabs, the focused one front-styled; icons 14 px.
2. Hover tab 2 → × shows; in that window run `while :; do printf '\e]0;%s\a' $RANDOM; sleep 0.2; done` (title changes 5×/s) → the hover and × stay steady. Record 2 s.
3. Click × → that window closes; click + → a new window in the group.
