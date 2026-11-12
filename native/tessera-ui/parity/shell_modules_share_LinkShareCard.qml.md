# shell/modules/share/LinkShareCard.qml → surfaces/share (src/lib.rs `update_link`, `link_key`; ui/share.slint `LinkCard`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | While LinkShare.mode; on the focused screen; overlay, keyboard on demand | 375-387 | `update_link` | same |
| 2 | Enter 300 ms × motionScale OutCubic; opacity + scale 0.96 → 1; gone at once | 389-417 | `animate shown`, `card_glass` | same |
| 3 | Card 320, padding 24; Glass glass / Material surfaceContainerHigh r 28 | 393-428 | `card` | same |
| 4 | Esc / Enter close (stops sharing) | 419-420 | `link_key` | same (Tab toggles the button's ring; Space on it closes) |
| 5 | Title 18 px 600 (Share “x” / Share n files / Receive from a phone), elided in the middle; help small secondary, hidden on error | 429-445 | `link_title`, `elide_middle` | same |
| 6 | Code 200 on white r 14, image inset 10, unsmoothed; loading: circle-notch 24 turning every 800 ms; error: danger text + "Install qrencode" (Installing…, disabled while updating) | 449-501 | stage blocks | same (QR read on the UI thread, as the QML's non-async Image; the install button is a fill chip, not SettingsButton) |
| 7 | Status (Sending… n % / Saved n times / Received n files / same Wi-Fi), label colour while busy or counted; the 10-minute warning 12 px tertiary | 503-521 | `link_status` | same |
| 8 | Button 40 r 20 accent (darker pressed), scale 0.97 over 100 ms linear; "Stop Sharing" while busy, else "Done" | 525-557 | done button | same |
| 9 | qrencode installed → the code is made again | 366-373 | (shell side) | same |

## Test plan (tester)
Arena 1920×1080, sandbox. A QR png: `qrencode -o /tmp/qr.png https://example.org` (without qrencode, any small png). Card x 800..1120, centred.
1. `rec ln-enter 0.5 "ipc linkshare demo ready /tmp/qr.png"` → fade + scale 0.96 → 1 in 300 ms OutCubic; the code is crisp (no smoothing). QML vs Rust, both looks.
2. `inbox 'ipc linkshare demo busy /tmp/qr.png'` → "Sending… 40%" in label colour, button "Stop Sharing".
3. `inbox 'ipc linkshare demo loading x'` → spinner, a turn per 0.8 s.
4. `inbox 'ipc linkshare demo error x'` → red "Making the code needs qrencode.", "Install qrencode" chip (don't click it: it installs).
5. Press the button: `P move 960 <button y>`, `rec ln-press 0.2 "P down"` → 0.97 in 100 ms; `P up` → closes.
6. Reopen, click the card body, `wtype -k Escape` → closes.
7. A phone scanning the code: not testable here.
