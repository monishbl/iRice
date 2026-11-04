# iRice changelog

## A quiet start, and any username at login · 2026-11-04

- **No more console text between the boot splash and your desktop.** The splash's last frame stays until the login screen draws, the login screen's compositor logs to the journal (`journalctl -t irice-greeter`) instead of onto the screen, and the console is cleared with its cursor hidden. On GRUB, `quiet` is added to the kernel line, so boot messages stay off the screen too (`/etc/default/grub` is kept as `.pre-irice`; other boot loaders get a note). Run `./install.sh --only=login` once to get it; `irice uninstall` puts it all back.
- **Log in as anyone:** on the Glass, Material and plain login screens your name is text that turns into a field when you click it (hovered, it shows a pencil), or press **Switch user** beside it. Type any username; Enter or Tab goes on to the password, Esc goes back. A typed name that's on the list is that person again, with their picture. The text login screen types the login straight in, as in ly. The login screen remembers a typed name for next time.

## Material and Glass, side by side · 2026-11-04

- **Material's controls everywhere in Material.** The Wi-Fi password field and Join button, the full Wi-Fi join form, Calendar's Add calendar, Report a Bug's boxes and the shortcut recorder were still Glass's; they're Material's filled pills and buttons now. Chips that turn things on and off (the menu bar's screens, a Focus schedule's days, the island's live pieces, a shortcut's modifier keys) are Material's filter chips, with a tick when on.
- **Settings line up.** In the Material look, lists of their own (shortcuts, gestures, login items, Software Update's packages, the speed test) sit on the same edges as every other row, with the same type; fields and drop-downs are as tall as the buttons beside them; a row whose button changes (Connect or Open Online Accounts, Install) keeps it on the row's edge in both looks.
- **Switched-off choices look switched off**, readable in Material and visible in Glass (they didn't look any different before).
- **Settings that do nothing in Material are hidden there:** dock magnification and the lock screen's Cards and field. The lock screen's Layout (Widgets or Classic) now shows in Material too, saying what each gives you there, so a Classic picked in Glass can be changed back from Material.
- **Glass catches up:** Escape leaves Edit Controls before it closes Quick Settings; a notification's buttons work by keyboard and are named for screen readers; the Downloads stack's Quick Preview button stays while you point at it.

## Live Text · 2026-11-04

- **Select the text in a screenshot or picture**, like on a phone: the screenshot editor's Select text tool (S) and Quick Preview's Text button (T) let you drag across words, double-click a line, copy with Ctrl+C or Copy text, or take everything with Copy all text. The floating thumbnail has Copy text too. Tesseract reads it on this computer; nothing is uploaded.
- **One-tap chips for what's in it:** links open in your browser (web links only), addresses start an email, numbers are copied (or called, when an app handles phone links), a Wi-Fi code offers to join. Nothing found in a picture ever runs as a command.
- **Search finds screenshots by their words**, with the shot as the row's picture and the words around the match; opening one shows it in Quick Preview with those words selected. The words are read in the background, one low-priority job at a time, only for new screenshots, and not on battery below 30% or in power saver; they're kept owner-only in `~/.local/state/irice`, and turning it off (Settings → Keyboard → More options → Text in screenshots) deletes them.
- **Your languages:** besides the system's, text is read in your keyboard layouts' languages (German for `de`, Hindi for `in`, …) once their data is installed. The same settings group lists them with Install buttons, through Software Update.

## Sharing's firewall ports open only while you're visible · 2026-11-04

- **With a firewall on, sharing's ports are open only while they're needed:** while you're visible to others in Quick Share or receiving with LocalSend, while the share sheet looks for devices, and while a QR-code share or a cast serves a file. They close a few seconds after. Only what's needed opens (Quick Share's ports, DHCP and mDNS; LocalSend's 53317; 53319–53320 for QR-code shares and casting).
- **Nothing stays open for good.** The rules last until the next restart (firewalld, nftables, iptables), and the shell opens them again if you're still visible. ufw has no such rules, so iRice removes its own when you stop being visible. The permanent rules from the old **Allow through firewall** button are removed automatically.
- **No password for an administrator at the computer**, as with the other fixed system jobs. System health and Settings → Privacy & Security → Firewall show what's open now, and why. They warn only when the ports didn't open (or close) as asked.

## Boot animations and a login intro · 2026-11-04

- **Pick what shows while your computer starts** (Settings → Lock & Login Screen → Starting up): your system's own animation (the default, as before), one of four of iRice's, or none (the boot messages as text). **Breathe**: the drop with a soft glow and a thin line that fills as it starts. **Orbit**: the drop in a thin ring with a light chasing round it. **Frost** (for Glass): the drop on a frosted glass tile with soft lights behind it. **Bloom** (for Material): the drop on a shape that morphs through Material's loading shapes. Each fits any screen, shows disk passwords and messages, and says when it's shutting down, restarting or installing updates. Changing it asks for your password and takes about a minute (the boot image is rebuilt); if the rebuild fails, nothing changes and Settings says why. Without Plymouth, Settings says how to get it.
- **Login intro** (off unless you turn it on): as your desktop first appears after you log in, with the startup sound. Glass: the wallpaper comes into focus and the menu bar and dock settle in. Material: the iRice shape opens onto your desktop. Never on a shell restart, never with Reduce motion, and it never stops you clicking or typing. Play Intro shows it.

## One codebase for both looks · 2026-11-03

- **Glass and Material are now one codebase.** Material used to copy 151 of Glass's files and change a few lines in each (31,938 lines repeated). Now each screen exists once, and where the looks differ it says so in place or picks its look's own view; Material shares 55 lines with Glass, in 2 files kept apart on purpose. A fix made once now reaches both looks, and a test keeps new copies out.
- **Material fixes that came with it:** the screen tools bar, the password prompt, the translation card and the overlay toolbar drew their Material background in the wrong place (nothing at all); Settings search in Material no longer lists rows Material doesn't show.
- **Old compiled shaders are cleaned up** on update, so a stale copy can't shadow the shared one.
- **For development:** `IRICE_ARENA=2 ./dev.sh up` runs another identical sandbox beside the first (up to 9), and `tools/smoke-looks.sh` uses whichever is free, so checks run side by side. It now also fails when the shell stops drawing or answering, and says whether the compositor or the shell was stuck.

## Licence, and names of our own · 2026-11-03

- **iRice is GPL-3.0-or-later.** The licence is in LICENSE; THIRD_PARTY_NOTICES.md lists the fonts, icons, protocol definitions and data that come from others, with their licence texts in LICENSES/.
- **Features renamed** so iRice no longer uses other companies' product names for its own features. Settings search still finds the old names, and nothing about your settings, shortcuts or IPC changes:

  | Was | Now |
  |---|---|
  | Spotlight | Search |
  | Mission Control | All Windows |
  | App Exposé | App Windows |
  | App Library | App Drawer |
  | Stage Manager | Stages |
  | StandBy | Bedside Mode |
  | Smart Stack | Widget Stack |
  | Quick Look | Quick Preview |
  | App Store | Software |
  | Control Center | Quick Settings (as in the Material look) |
  | Liquid Glass | Glass |
  | Overview style "macOS", lock layouts "iOS" / "macOS" | "Strip"; "Widgets" / "Classic" |
  | Window tabs "Finder", emoji styles "Android" / "Windows" | "iRice"; "Noto" / "Fluent" |
  | the built-in "Tahoe" wallpapers | the built-in set (Day, Dusk, Night) |

## Faster Quick Share, iPhones by QR code, apps' own logos · 2026-11-03

- **Quick Share takes the fastest way first.** Before any file moves it checks the link: a shared Wi-Fi that's on the busy 2.4 GHz band or answers slowly gives way to a direct Wi-Fi link (Wi-Fi Direct, next to your own Wi-Fi). A link that fails gets a second try before the next one, and one that keeps failing with a phone is skipped for that phone for a day. Tested with a phone on a router that keeps devices apart: 500 MB in under a minute one way and about ten seconds the other, where it crawled at 0.3 MB/s before.
- **It keeps going when the network won't.** A phone the Wi-Fi can't reach is found over Bluetooth instead; when the phone asks for a faster link mid-transfer, iRice now gives it one; a phone that's also plugged in by USB isn't sent files over the slow cable.
- **Fewer alerts.** You hear from Quick Share when someone wants to send you something, when it's done, and when it fails (saying how to fix it). Switching links, a Wi-Fi pause and the PIN while they decide show quietly in the transfer's line in the island.
- **iPhones and any other phone, by QR code.** Share → Share with a QR code: the iPhone's camera opens a page that saves the files. Settings → Phone & Sharing → Receive from iPhone: the page takes photos and files from it. Only on your own network, with a secret link that stops working after 10 minutes or when you close the card.
- **Apps' own logos.** Icons are each app's original logo, restyled to match: clear glass in the Glass look, Material You themed in the Material look. Icon themes (Colloid, Tela circle, Papirus and others) are optional, in Settings → Appearance. GTK apps use Adwaita, and the pointer is Bibata in both looks.
- **Window gaps are a setting.** Settings → Windows & Spaces: None, Compact (the default, tighter than before) or Comfortable.
- **Shake to find the pointer: one control.** Off, Bigger or Ring, and the bigger pointer shrinks back reliably and quickly.
- **Menus stay on screen.** A right-click menu near the bottom of the screen opens upward, and a long one scrolls; Settings' drop-down lists fit inside the window and scroll, starting at the current choice.
- **Pinch and spread were swapped** on the trackpad: pinch opens the App Drawer and spread shows the desktop again.

## Material 3 Expressive, round two; welcome, backups, 24-hour time · 2026-11-03

- **Transparency, as much as you like.** Settings → Appearance → Transparency (Material look) replaces the Blur switch: 0% is solid, the default; slide it up and panels, menus, tiles and tonal buttons let the blurred desktop through. Text and filled buttons stay solid so they stay readable.
- **Every press bounces.** Buttons and tiles swell a little while pressed and spring back past their size, their corners squaring off, as Material 3 Expressive does. In button groups the pressed button widens and its neighbours make room.
- **The spaces bar is quiet.** In the Material look the dots sit straight on the menu bar; the pill only appears for something live, an alert, or a hover.
- **Quick Settings in every size.** It packs the same layout as the Glass Control Center, in every size Edit controls offers: toggles small or wide, brightness and volume across or standing (tall sliders), the player half-width or full-width, readings wide or large.
- **Players stay where they are.** Choosing another player widens its card in place while the current one narrows to a slice; nothing swaps places. The lock screen's players work the same way.
- **The lock screen, redone (Material).** A new clock face, Dynamic: tall hours with the minutes beside them and AM/PM under the minutes (the date, with 24-hour time). The clock no longer shrinks into the corner by itself. At the bottom there's one pill, your picture, your name and the fingerprint glyph, that opens into the password field as soon as you type or move the mouse; the key you pressed is kept.
- **Notifications on the lock screen, three ways** (both looks): Newest (the apps' icons and the newest one under the clock, the player over your account), Beside (a column next to the clock, like a tablet) or Icons. Settings → Lock Screen → Notifications.
- **24-hour time everywhere.** Settings → Appearance → 24-hour time: the menu bar, the lock and StandBy clocks, widgets, events, alarms, timers and status lines follow it.
- **Split buttons, FAB menus and floating toolbars.** The screenshot editor's Save, the recorder's Record and the App Store's Install/Open are split buttons with the other choices in their menu; Quick Note and the app drawer have FAB menus; the clipboard's actions float over the list.
- **No more flash of the bundled wallpaper at start.** The desktop waited for nothing: before your settings were read it showed the default Tahoe picture for a moment, then yours. It now fades in straight to your own wallpaper.
- **Login screens, four ways.** Settings → Lock Screen → Login screen: your look (Glass, or Material with your clock and colours, whichever you used last), Text (like ly: a box of text, keyboard first) or Plain (black, your picture and the field). The Glass and Material ones have no wallpaper either, just a clean background, the clock a little lower. A fingerprint works at all of them where the reader is set up; if a style ever fails to start, the text one comes up instead. Preview shows each on your screen without logging out. (It's used once the iRice login screen is installed: ./install.sh --greeter.)
- **Settings' icons match** in the Material look: one tone, all filled, and Software Update's is an arrow, not a dot.
- **Material window controls.** In the Material look the controls at the top of each window (Settings → Appearance → Windows → window controls) are round icon buttons on a tonal pill: minimize, maximize and close, close turning red under the pointer. Glass keeps its traffic lights; switching looks switches them. (The compositor plugin is rebuilt with the update.)
- **A welcome screen.** On a new setup iRice asks three things: Glass or Material, light or dark, and a wallpaper. Settings → About shows it again.
- **Back up and restore your settings.** Settings → About → Back up and restore saves your setup (settings, shortcuts, Focus modes and your wallpapers) to one file, without passwords, keys or accounts, and loads it on another computer; restoring keeps an Undo copy.
- **Fixed:** a half-typed password stayed in the lock screen's field after unlocking with a finger or a face, and showed again at the next lock.

## The Material look, Android 17 · 2026-11-03

- **Lock screen clocks.** Pick a style in Settings → Lock Screen → Clock from a carousel over your wallpaper: Stacked, Wide, Overlap, Analog or Weather, with their thickness, width and rounded or sharp digits. The clock is dynamic, as Android 17's: large while the lock screen is empty, small in the top corner once notifications arrive or something's playing (or always large, or always small).
- **A media carousel.** With more than one player, the others wait beside the current one in Quick Settings and on the lock screen as narrow cards with their album art. Click one, or swipe the card, to switch.
- **Tinted blur.** Quick Settings, menus, the power menu, the volume panel and the app drawer show the desktop behind them, blurred and tinted by your colours. Settings → Appearance → Blur turns it off.
- **Quick Settings opens with more bounce**, and its edit button is in the bottom-right corner.
- **The app drawer's menu.** Right-click an app, or press and hold: its own shortcuts (New Window and the like), then Open and Keep in Dock. Settings → Dock can hide the names under the drawer's icons.

## A calmer Material lock screen; Edit controls stays open · 2026-11-02

- **The Material look's lock screen is quieter.** The clock is about a third smaller, and Now Playing, your picture, the name, the field and the fingerprint button are all more compact. The fingerprint is offered once, by its button: the field says "Enter password", and the line under it only appears when there's news (a finger that didn't match, the face camera looking).
- **The island stays dark in dark mode (Material look).** An alert in it (a notification, the charger, Wi-Fi) turned it the inverse colour, like an Android snackbar: a white island under a dark menu bar. It now goes a shade lighter instead, in the same colours.
- **Software Update works in the Material look.** It said "The installation source is missing": the Material look runs from a folder of links in your cache, and Software Update looked for iRice beside that folder. It (and the system part of the update, and the health check) now follows the links back to your iRice folder.
- **Quick Settings' Edit controls no longer closes when you click the brightness or volume row** (or any empty part of the panel). Those clicks went through the panel and counted as a click outside it.

## Smooth while apps are busy · 2026-11-02

- **The shell no longer stutters while other apps work hard.** CachyOS's process tuner (ananicy-cpp) files every program named "qs" as a background service and lowers its priority below apps like VS Code, so panels and animations stuttered whenever anything else was busy. The shell now runs as "irice-shell", which that rule doesn't match. Software Update's system step also installs iRice's own rule (where ananicy-cpp is installed), which gives it the same interactive priority as GNOME Shell and KWin. That step runs before the new version arrives, so the rule comes with the update after this one. This takes effect the next time the shell starts after the update (log in again, or press Super+Ctrl+Escape).

## The fingerprint works after sleep · 2026-11-02

- **The lock screen keeps the fingerprint reader after the laptop sleeps.** The lock goes up as the laptop sleeps, and its fingerprint request started then. On waking, that request is hours old to its own clock and can still hold the reader, so the new requests were refused ("already claimed"). Each refusal counted as a missed finger, and five of them in about two seconds turned the finger off until you came back. Now a request that's refused before the reader asks for a finger means "busy": it doesn't count as a miss, and it tries again after a growing pause (0.5, 1, 2, then every 4 seconds, for about 40 seconds). Waking from sleep is noticed while locked (a jump in the clock), and the finger starts afresh with every try back. Coming back to the laptop also gives a reader that stayed busy another go.

## Smaller fixes · 2026-11-01

- **A long song title no longer makes the island's scrolling text re-measure itself over and over** (a binding loop, logged each time it scrolled).

## Adding a fingerprint: a guided popup, 11 sections · 2026-11-01

- **Adding a finger opens a popup that teaches you to cover the whole print.** The fingerprint is drawn like the familiar icon, a tilted loop with dashed ridges, and split into 11 sections: the centre, four around it, then six around the edges. The section to touch next is lit and named ("Again, a little higher up your finger", "Roll onto the upper right edge"…), so the reader records the whole finger rather than the same middle each time. Each touch it takes fills its section, with a small bump. When the reader wants another try, the print shakes, the section flashes red, and the reader's reason shows. Between the centre and the edges it says "Now the edges", as a phone does, and at the end every ridge turns green with a check: Done, or Add Another Finger. Escape or Cancel stops it. Dots underneath show all 11 sections, and nothing moves except on a touch.

## A signature sound for logging in, with its own volume · 2026-11-01

- **Logging in has one signature sound, in every style.** A quick glint runs up a D-flat major chord and opens into the chord itself, warm and wide, with an added ninth on top. Then a shimmer blooms out of it and lifts a little, in a long room. It's built to sound full on laptop speakers: the depth is in overtones rather than sub-bass that small speakers can't play. It's much louder than the other sounds (about −15 dB on average, against −30 to −36 dB for unlock and notifications in the Minimal style).
- **Its own volume and switch.** Settings → Notifications → Logging in has the sound's switch, Play and Choose…, and a volume of its own (90% to start). It plays even with sound effects off, and the effects' volume doesn't change it.

## Fingerprint: an animated set-up, no false "Not recognised", the reader in every password prompt · 2026-11-01

- **Each added finger has a fingerprint beside its name** (adding one: see the guided popup above).
- **No more false "Not recognised".** Dell's Broadcom reader can answer "no match" within a fraction of a second, for a finger still resting on it after waking the laptop. The lock screen and password prompts showed that as a failed finger, and it used up tries. A "no match" faster than a touch is now ignored, only real misses count (five on the lock screen), and "Not recognised" stays long enough to read. Password prompts and sudo now allow 5 tries instead of 2; turn their switches in Settings → Lock Screen → Fingerprint off and on once to take this up. A password prompt says "Incorrect password" only after you've typed one.
- **The reader shows wherever your password is asked.** On the lock screen, a fingerprint sits in the empty password field ("Fingerprint or Password"): it pulses when the reader is ready, shakes red on a miss and turns into a green check on a match; typing brings the arrow back. Password prompts show the same fingerprint while they wait for a finger. Nothing loops, so an open prompt draws no frames.

## Fingerprint reader set-up, a quieter shelf icon · 2026-11-01

- **The fingerprint reader can finish setting itself up.** On Dell's Broadcom ControlVault 3, the driver updates the reader's firmware the first time fprintd starts, which takes about a minute. CachyOS stops a service that hasn't started within 15 seconds, so the update was cut off at every start and "Restart" never helped. Settings → Lock Screen → Fingerprint now notices this and offers **Set Up Reader**, which gives fprintd five minutes to start and waits while the firmware updates. Until then, the page no longer starts fprintd just to look, which would cut the update off once more. The reader is named on every ID it uses (5841 while its firmware is still to be updated, through 5845).
- **The shelf's menu bar icon no longer pops up whenever a button is held and moved** (selecting text, dragging a slider). It shows while the shelf holds something, or during a real drag of files.
- **The session watchdog looks every 30 seconds, not 10.** Each look starts two small programs; a frozen shell is still restarted within a minute.

## Island fixes, captions over the video, tucked-away app icons · 2026-11-01

- **Buttons on island alerts:**
  - **A button's own follow-up stays on screen.** Pressing Connect on earbuds used to close every alert, including the "Connecting…" it had just shown, so the island fell back to its usual view until "Couldn't connect" appeared. Alerts a button posts now stay up. In the card under an open island, the button now closes the alert it belongs to rather than the newest one.
- **Fast Pair connects earbuds that are already paired.** Earbuds in pairing mode advertise from a nameless address of their own, so ones paired before (the realme Buds T100 here) weren't recognised and the pairing failed. They're now matched by the company the advert names, or by name or address, and simply connected. The card says "Paired with this computer, not connected", and nothing is offered when they're connected already.
  - **"Connecting…" stays until it's done** (pairing can take a minute), then the result replaces it.
  - **If earbuds that were reset won't take the old pairing, the card offers Pair again,** which forgets the old pairing and pairs them afresh.
- **One card per device, not a pile of them.** Connecting earbuds used to stack up to four alerts: "Connect", "Disconnected", "connected · Sound plays there now", and "Connected" again. Now the same card goes from Connect to Connecting… to Connected, battery included, and Recent keeps one line for it. Alerts can carry a key (a Bluetooth device's is its address), and an alert with the key of one already up replaces it.
  - **Fast Pair offers Connect once per pair of earbuds,** and not at all while they're connecting or connected (they keep advertising, from new addresses).
- **Switching to another output or microphone shows its name and level.** When headphones connect or you pick another output, the volume bar (in the island or the capsule) shows the new device's name over its own level, once that has arrived. With both switching, the output's is shown. The microphone bar now shows the input level, not just on or off. The level and mute arriving with the switch no longer pop up a bar of their own.
- **Device pictures in more places:** the Bluetooth lists in Control Center, the menu bar and Settings (a small accent dot marks connected ones), the desktop Batteries widget's rings, and the island's Recent list and Notification Center history.
- **A battery level that arrives late traces in.** Headphones often report their level seconds after connecting: the ring now draws itself round from empty and the number fades in, rather than popping up drawn.
- **Bluetooth devices warn when they run low,** at 20% and again at 10%, on the device's own card with its picture (once each, until it's charged past 25%).
- **Earbuds show left, right and case.** When paired earbuds broadcast separate levels (case open), their card shows three small rings, and the Batteries widget a ring each.
- **Spotlight understands plain words.** Sums and percentages ("18% of 2450", "2450 plus 18%", "half of 300", "twelve times four", "3 dozen", "120 is what percent of 480"), dates ("days until christmas", "3 weeks from today", "what day is 14 Feb", "how old is someone born 1998"), time zones ("time in Tokyo", "5pm London in Delhi"), units ("5 km in miles", "100 f in c", "2 cups in ml") and currency ("100 usd in inr", European Central Bank rates fetched at most once a day).
- **Answers and actions get a big card on top of Spotlight:** a coloured tile, the answer in large type, and what Enter does (copies it, or does it, for "remind me…", "timer 10 min" and the rest).
- **Quick Share shows how a transfer is arriving as an icon on the phone in its card** (the same Wi-Fi, a direct hotspot link, or Bluetooth) instead of words.
- **Dragging files to the shelf opens a big drop zone from the island.** A shake or an edge now drops in a black zone under the menu bar with two large targets, Keep on the shelf and Send to a phone (the share sheet: Quick Share or LocalSend), lit in the accent colour under the drag. Dropped on Keep, it shrinks to the shelf's card with your files. If the drag ends elsewhere it goes.
- **The drop shelf opens every time, and from the top edge too.** A drag of files swallows the mouse button's release, and Hyprland handed it over at the next press, so after the first drag the shelf thought each new drag had already ended and never opened again. That stale release is now ignored, and each press gets fresh drop targets. Holding a file drag at the top edge opens the shelf on the nearer side.
- **The calendar doesn't dot every day for daily events.** Something that repeats every day (on at least 80% of a week or more of days) is listed once as "Every day: …" under the month, instead of a dot on every date. It still shows in each day's list.
- **The drop shelf opens only when you're dragging files.** It used to treat any held button moved about as a drag, so selecting text, moving a scrollbar or slider, or dragging an image or link in the browser near a screen edge could pop it up. Now, while a button is held, invisible drop targets sit at the screen edges and under the pointer after a shake, and only a drag carrying files opens the shelf. Text and links can still be dropped on it once it's open.
- **No company logos in the menu icon picker.** Apple, Windows, Android, Steam and GitHub marks are gone. Tux, your distro's own logo and iRice's symbols remain, and an old choice of a removed mark shows the iRice drop. Auto shortcut symbols follow Command (⌘ ⌃ ⌥ ⇧), else Super Ctrl Alt.
- **Microphone in Control Center.** A new "Microphone level" slider is in Edit Controls (not added to your layout). Its arrow opens a Microphone page with the level and the inputs, each with its device's picture. The Sound menu (Control Center and the menu bar) shows the same microphone section under the outputs and apps, and Settings → Sound → "Microphone in the Sound menu" turns that off. The Sound page's outputs show device pictures too.
- **The volume, microphone and brightness bar works with the mouse.** Point at it and it stays up. Click, drag or scroll along it to set the level, and click its icon to mute. The button at its end opens a list underneath:
  - **Volume:** the outputs, with their device pictures and a tick on the current one, plus App volumes › (each app playing, with its own level) and Sound settings.
  - **Microphone:** the inputs.
  - **Brightness:** each display with its own level, built-in and external (DDC).
  It works the same in the island and in the separate capsule.
- **The volume and brightness bar no longer pops up for iRice's own sliders** (Control Center, the Sound menu, the island's player, the overlay mixer, Settings) or its mute buttons. Keys, scrolling on the menu bar, Spotlight and other apps still show it.
- **Bluetooth cards show a picture of the device.** Earbuds, over-ear headphones, a speaker, a soundbar, a phone, a watch, a game controller, a keyboard, a mouse or a laptop, chosen from the kind of device Bluetooth reports and its name (earbuds and over-ear headphones report the same kind, so "Buds" or "WH-" in the name decides). Anything else gets a Bluetooth tag. They show on connect and disconnect, batteries, and Fast Pair's card from Connect to Connected, in place of the coloured icon. The drawings are made by `tools/device-pictures.py`.
- **Touchpad scrolling glides after your fingers lift,** as on macOS, everywhere that scrolls: Settings, Spotlight, Notification Center, Control Center, the clipboard, emoji and app lists, and the rest. A slow scroll or a pause before lifting stops where you left it, and a mouse wheel works as before. (Qt on Wayland stops dead when the fingers lift; libinput leaves the glide to the app.)
- **Leaks and stray processes, found by checking a running session:**
  - **Clipboard pictures no longer pile up in memory.** Previews of copied images were kept in RAM (62 MB after a few days here) even after the entries left the clipboard history. They're now removed with their entries, and with Remove and Clear.
  - **No leftover process after the shell restarts.** Each restart left one finished-but-not-collected process behind.
  - **The screen recorder, dictation's microphone and the phone's camera stop if the shell ever crashes.** Before, a recording could carry on with no indicator, and the phone's webcam stayed on unseen.
  - **Fast Pair forgets Bluetooth devices that have gone away.** It kept every nearby phone and watch it had ever seen, and they change addresses every 15 minutes.
  - **Window snapping forgets closed windows,** so a new window doesn't get an old one's Restore size.
  - **Notification Center keeps the newest 200.** The oldest go, so a chatty app can't fill memory.
  - **Notifications with buttons from screenshots, recordings and the desktop's trash** no longer leave a waiting process behind for good. They wait up to 10 minutes.
  - **Live captions on a slow computer** drop sound more than 10 seconds behind, instead of queueing it without end.
  - **A Quick Share or LocalSend card no longer gets stuck on "Receiving"** if its helper stops.
  - **The translator says when translate-shell is missing,** instead of quietly piling up a failed request on each keystroke.
  - Checked for 10 minutes on a running session, and under a stress test of 100 alerts, 30 island open/close cycles and 40 track changes: the shell's memory stayed flat.

- **Album art in the island loads reliably.** Browsers like Zen save each video's picture under the same file name, so the island sometimes kept the previous picture or showed none. Each track now loads its own picture, a failed load is retried, and the picture is read again a moment after the track changes. The same fix applies to the lock screen, Control Center, the desktop widget and Now Playing.
- **Album art no longer goes missing after a browser restarts a video.** Zen can leave its old player behind, still saying it's playing, with its picture deleted, and the island showed that one. It now shows the newest player that's playing.
- **Album art (and the island's black) no longer vanish after the island closes and opens again.** The rounded-corner effect cutting the island to shape stopped drawing after the island shrank and grew. A small shader of iRice's own does that job now.
- **Alerts no longer take over the open island.** When the island is open (pointed at or pinned) and an alert arrives, the island stays as it is, and the alert drops in as a card just under it. Newest on top; up to two more peek out behind it, with a count of any others. Pointing at the card holds their time. When the island is closed, alerts show in the island as before.
- **No more tall empty island** when an alert arrived while Recent was open.
- **Album art shows even when the browser stops sending it.** Zen sometimes drops a video's picture from what it tells the system (after a pause or a seek), or deletes the picture file. The island now keeps the picture it had for that track, and for YouTube videos falls back to the video's own thumbnail. Every place that shows album art uses this.
- **The island's player has new button places:**
  - **Volume on the left.** Click the speaker to mute. Point at it and a slider opens beside it for the app's own volume (or the system's).
  - **Open the app on the right,** where the speaker was, next to Cast. Point at it to see which app it opens (its name is cut short when long). The island's buttons are round when pointed at.
  - **Recent alerts moved to a button at the top left,** next to the pin's corner. It no longer takes a row at the bottom of the island.
- **The island no longer gets cut off at the bottom** when music, captions and the status row show together. It has more room to grow.
- **The compact island's buttons work on the first click.** Pointing at pause, stop or close no longer opens the island under the pointer, so the click reaches the button. Pointing anywhere else on the pill still opens it.
- **Live captions over the video.** A new place for them in Settings → Accessibility → Show them → **Over video**. The captions sit over the bottom of the window that's playing, like a film's subtitles, up to three lines, and clicks go through to the window. Without a playing window, they use the focused one.
- **Captions in the island show more:**
  - **Several lines,** instead of one line cut off at the end.
  - **They stay in view.** Music starting, or a video paused and played, no longer takes the island's focus away from captions.
- **Shake to find the pointer has a setting for how it shows:** a ring (the default), a bigger pointer, or both, in Settings → Accessibility. Both are smoother:
  - **The ring** follows the pointer 125 times a second instead of 25, and no longer jumps.
  - **The pointer** grows in one step, instead of four steps that each stuttered.
  - Apps that draw their own pointer, such as foot, keep its normal size.
- **Fast Pair connects instead of failing to pair.** Earbuds announce themselves over Bluetooth LE without a name, and Connect tried to pair with that announcement, which earbuds refuse. It now finds the earbuds' classic Bluetooth side, by name, by address, or as the one audio device in pairing mode close by, and pairs with that. If it can't find them, it tells you to hold the button on the case until the light flashes.
- **Tuck away app icons.** The icons apps put in the menu bar now wait behind an arrow. Click it to show them; they tuck away again after 12 seconds, but not while one of their menus is open. Turn it off in Settings → Menu Bar → Tuck away app icons.

## Live captions, faster Quick Share, going back after an update · 2026-10-31

- **Quick Share is much faster without Wi-Fi in common:**
  - **Over Bluetooth:** phones now use a direct Bluetooth channel (L2CAP), about 7 to 15 times faster than before (51 to 104 KB/s instead of 7 KB/s), in both directions.
  - **Over Wi-Fi Direct:** when a phone sends while you're not on the same Wi-Fi, this computer can now make a Wi-Fi Direct link for it without dropping your own Wi-Fi, so your internet stays up during the transfer. It asks for your password the first time.
  - **Sending to a phone no longer pauses your Wi-Fi:** this computer joins the phone's Wi-Fi Direct link as a second, Wi-Fi Direct connection beside your own (Wi-Fi chips allow one), where it used to drop your Wi-Fi for the transfer.
  - **Wider links:** links this computer makes now use a wide 80 MHz channel instead of 20 MHz.
  - **Checked with a real phone, 300 MB each way, byte-identical, with your Wi-Fi and internet up the whole time:** 95 MB/s to the phone, 35 MB/s from it (6.5 MB/s before the wider channel).
- **Fast transfers without asking.** A new switch in Settings → Sharing lets Quick Share set up its direct Wi-Fi link without asking for your password each time. Turning it on or off asks once. It applies to your account only, at this computer; updates and everything else still ask.
- **Live captions.** Subtitles for whatever this computer plays: videos, calls, podcasts. Turn them on in the new **Settings → Accessibility** page, with **Super+Ctrl+L**, or from a Control Center tile you can add.
  - **How it works:** captions are made on this computer with whisper.cpp's Tiny model (75 MB, downloaded once), about 2 seconds behind the speech. Nothing is saved or sent anywhere, and the microphone is never captioned.
  - **Where they show:** in a floating bar above the Dock that you can drag anywhere, in the island, or in a strip across the bottom of the screen that windows make room for. White on dark, so they read over any picture.
  - **Language:** Automatic, or choose one, which is more accurate.
  - **Cost:** silence isn't transcribed, so they cost almost nothing while nothing is playing.
- **Go back to before the last update.** If an update broke something, use **Settings → Software Update → Going back → Go Back**. The packages it changed return to the versions you had before. Those versions come from pacman's cache, or from Arch's archive when the cache no longer has them. Your files and settings aren't touched, and packages the update added stay. It asks for your password once and takes a snapshot first when you have snapper or Timeshift.
  - A version you went back from is skipped by later updates until a newer one comes out. You can include it anyway, per package.
  - If some earlier versions are gone from your computer, the page says which ones and nothing is changed.
- **When the desktop won't start after an update:**
  - If iRice's shell stops three times within 15 minutes, and an update ran in the last 3 days, a recovery window opens. It offers to go back to before that update, start the desktop again, or open a terminal.
  - From a text console (Ctrl+Alt+F3), `irice undo-update` does the same.
- **Shake to find the pointer.** Move the mouse (or your finger on the trackpad) quickly back and forth: the pointer grows for a moment and a ring closes in on it. Ordinary fast movement, dragging and drawing don't set it off. Turn it off in Settings → Mouse. Hyprland checks the pointer's position 5 times a second while it's still, and 25 times a second while it moves.
- **Sharing looks like AirDrop on iOS, in glass:**
  - **The share sheet** shows a preview of what you're sharing, nearby devices as round avatars in a row, and Copy and Show in Files as a grouped list. Under each avatar it says Waiting…, then Sending… as a ring fills, then Sent ✓ or Declined.
  - **Someone sending to you** gets an AirDrop-style card in the middle of the screen. It shows who it is, what they're sending, and the PIN their phone shows, with Decline and Accept. Accept, and a ring fills around them as it arrives. When it's done: Open or Show in Files. Hide leaves the transfer to the island.
  - Quick Share and LocalSend look the same.
- **"Allow through firewall" works again**, and every update now brings iRice's system helper up to date with it:
  - **Why it seemed broken:** an update installed the system helper before downloading the new iRice, so the helper stayed one update behind. Its firewall fix only knew the older rules, said "done", and added nothing new.
  - **What changed:** the update now installs the helper from the version it's downloading. An outdated helper alone also counts as something to update, so it can be brought up to date right away.
  - **Until then:** Settings → Sharing uses the current fix, with your password.
- **Smaller things:**
  - The lock screen still says "Touch the fingerprint reader" under a wrong-password message.
  - Settings buttons tell screen readers which row they belong to ("Remove" for the left thumb, not just "Remove"), and the Sound page's two sliders are named Output volume and Input volume.
  - With no microphone, the Sound page says so in its Input section instead of showing an empty card. iCloud and Microsoft 365 do the same while there are no calendars yet.
  - The To Do list chooser names a Google Tasks list chosen elsewhere instead of calling it "This computer only".
- **Lighter login:** six status checks (dictation, drives, AirPlay, missing packages, the phone's webcam, snapshots) now run one at a time, 15 seconds after the desktop is up, instead of all at once at login.
- **Snapshots in the boot menu.** On GRUB with btrfs and snapper, a switch in the same section makes GRUB list your snapshots (grub-btrfs; it installs grub-btrfs if needed). Start from one and the island says so. To keep that snapshot, restore it in Btrfs Assistant, which the page opens for you or offers to install.

## Fixes for today's features · 2026-10-31

- **Face unlock no longer undoes a lock you made by hand.** The camera starts looking when someone touches the keyboard or mouse after the lock appears, not the moment it locks while you're still in front of it. Its three tries, and the fingerprint reader's five, come back when you return after a while, not on every key you type. A fingerprint reader that waited 30 s without a finger no longer counts that as a failed try, so it keeps listening.
- **Dictation can't get stuck.** If the microphone couldn't be recorded, Super+H stopped working until a restart; now it says so and is ready again. Cancel also stops the speech recognition working in the background, and a cancelled model download no longer leaves a large unfinished file behind. Downloading a bigger model keeps the one you have working until the new one is here.
- **iCloud:**
  - Changing an event's notes no longer changes its reminder instead.
  - Two quick changes to the same event, or a change and then a delete, no longer fail with "This event changed on your other devices".
  - A change that was saved is no longer reported as "Change not saved" when only the refresh afterwards failed.
  - Each change refreshes less.
- **Google Drive and OneDrive:**
  - Both drives come back at login, not just the first.
  - A drive that can't be opened (an expired sign-in) stops retrying after a few tries instead of every 10 seconds.
  - A drive whose rclone stopped can be unmounted and opened again.
  - Connecting can be cancelled.
- **Watchers stay off when you turn a feature off.** Fast Pair, phones, casting and others kept running after being turned off if they had restarted once. They now wait longer between restarts when something keeps failing, and casting doesn't look for TVs while network discovery is off. Network discovery and NetworkManager are also recognised on systems without systemd.
- **Privacy:**
  - Dictated text and Wi-Fi passwords are passed to the typing and clipboard tools privately, not on their command line.
  - A Wi-Fi code that was still being made when you closed the card is now deleted.
  - Picking another network quickly can no longer show the first network's code.
  - Phone pairing codes aren't put on a command line either.
- **Screen mirroring:** Stop works while it's still connecting, and choosing another TV quickly no longer leaves the first one mirroring. Settings → Displays starts looking for TVs as soon as GNOME Network Displays is installed, and says if the install didn't work.
- **AirPlay** can't start a second copy of its PipeWire client when turned off and on quickly.
- **Phone:**
  - Pairing no longer sticks on "Pairing…" when the phone stops answering, and no longer says "Paired" when it didn't connect.
  - A failed webcam install says why.
  - The Dock shows the phone's current battery.
- **Fingerprint and Face settings** ask once more, naming the finger, before removing a fingerprint or your faces. Adding a finger can no longer add the same finger twice. Turning fingerprint off for password prompts goes back to your system's own file.
- **Other fixes:**
  - The phone's webcam setup says when another app's virtual camera (OBS) is in the way, instead of reporting success.
  - On Manjaro, Software Update's AUR step respects the packages you hold back.
  - Wi-Fi codes work for WEP networks.
  - Escape cancels a password prompt while it waits for a finger.
  - The iCloud and Microsoft 365 pages each show only their own errors and sign-in.
- **Lighter:** one sound-server listener instead of two, and Fast Pair asks Bluetooth about each nearby device once instead of on every signal it sends. With all of today's features in, the shell uses the same memory and processor at rest as before them.

## Works with your other devices · 2026-10-31

- **Works on any Arch-based system**, not only the one it was built on:
  - Features from the AUR (fingerprint drivers, face unlock) set up an AUR helper when there's none: yay or paru from your distribution's repositories (CachyOS, EndeavourOS, Garuda), pamac on Manjaro, or yay built from the AUR on plain Arch.
  - The phone's webcam builds its virtual camera for whichever kernel you run (linux, linux-lts, linux-zen…, with its headers) when the kernel doesn't include it.
  - Face unlock finds infrared cameras that don't say "IR" in their name (they're the ones that only see in greyscale), and says how to switch on an IR light that stays off (linux-enable-ir-emitter).
  - Fingerprint works with LightDM and LXDM login screens too (GDM already asks for a finger by itself), and a reader without a Linux driver is named as such instead of not shown.
  - Pages say what's missing on your system and offer to install it or turn it on: network discovery (avahi) for casting, AirPlay and phone pairing; a keyring for iCloud; GNOME Online Accounts and iRice's sign-in helper for Microsoft 365 (built from the page); qrencode for Wi-Fi codes; wtype for dictation. AirPlay says when the sound isn't going through PipeWire, and Miracast when NetworkManager isn't running.
  - Dictation records with PulseAudio's or ALSA's recorder when PipeWire's isn't there.
- **AirPlay speakers.** HomePods, Apple TVs and AirPlay speakers on your network can play this computer's sound: turn on **AirPlay speakers** in the new **Settings → Sound** page (it installs PipeWire's AirPlay support the first time, one password). They then appear as outputs there and in Control Center. Sound reaches them about 2 seconds late, which is fine for music but not for videos or calls. Some AirPlay 2-only speakers, and speakers with a password, may not accept it.
- **Settings → Sound**: the output and input you use, their volumes, AirPlay, and a way to the sound effects.
- **Face unlock for the lock screen**, on computers with an infrared camera (Windows Hello cameras). In Settings → Lock Screen → **Face**: install Howdy (howdy-next, from the AUR), set up the camera, and add your face; then look at the screen to unlock it, while your password and fingerprint keep working too. It's for the lock screen only (password prompts and sudo still ask for your password or finger), and it's less secure than a password: the page says so. Plain webcams aren't offered, since a photo can fool them.
- **Earbuds pop up when you open the case.** Earbuds and headphones with Google Fast Pair (Pixel Buds, many Sony, JBL, Bose, OnePlus, realme and others) in pairing mode get a card in the island with **Connect**: one click pairs and connects them. With your earbuds connected, opening the case shows each bud's and the case's battery. It only looks while Bluetooth is on, lightly (continuously only on the charger). Turn it off in Settings → Bluetooth → **Show earbuds nearby**.
- **Screen mirroring to a TV.** A new **Screen Mirroring** tile in Control Center (and **Wireless displays** in Settings → Displays) lists TVs and sticks nearby: Miracast (over Wi-Fi Direct: the TV needn't be on your Wi-Fi), Windows' Miracast over the network, and Chromecasts. Pick one to show your screen there; the island says it's mirroring, with Stop. Uses GNOME Network Displays (Install in Settings → Displays). If this computer's Wi-Fi can't do Wi-Fi Direct, it says so.
- **Your Android phone on the computer.** The new **Settings → Phone** page sets it up (USB debugging, once), then the phone shows in the Dock while it's connected: click it to see its screen in a window and use it with your mouse and keyboard (drop files on the window to copy them to the phone's Downloads). Pair it once with a QR code and it connects over Wi-Fi without the cable (Android 11+). Its camera works as a webcam for calls ("Phone camera", Android 12+; one-time setup with your password). Uses Google's Android tools and scrcpy (Install on the page).
- **iCloud Calendar and Contacts.** Sign in to iCloud in the new **Settings → iCloud** page with your Apple ID and an app-specific password (the page walks you through making one at account.apple.com; it's kept in your keyring). iCloud calendars then show in Calendar, Today and Spotlight, and you can add, change and delete iCloud events from Notification Center's calendar (choose the iCloud calendar when adding one). Repeating events show but are changed on your iPhone or at icloud.com. Your iCloud contacts show in Spotlight: pick one to copy their email or number.
- **Microsoft 365: Outlook calendar and To Do.** **Settings → Microsoft 365 → Sign in with Microsoft** (your browser opens at Microsoft's sign-in; personal and work or school accounts). Outlook calendars show with your others and can be added to and changed (repeating ones are changed in Outlook), and Microsoft To Do tasks show in To-do. New to-dos can go to a To Do list.
- **Google Drive and OneDrive as folders.** ~/Google Drive and ~/OneDrive, real folders in Files, file pickers and the terminal: connect them in Settings → Google and Settings → Microsoft 365 (installs rclone the first time; the sign-in opens in your browser). Files open on demand, the ones you use are kept on this computer, and changes upload in the background. They come back at every login.
- **Chromecast and Google TV.** Send what's playing to a TV or speaker on your Wi-Fi: the cast button in Control Center's Now Playing (and in the island's player) lists them, and **Cast to <TV>** in Spotlight does it in one go. Videos, songs and photos also go to a TV from the share sheet. The island shows what's playing there, with pause and stop, and the player on this computer pauses. Local files are played straight from this computer over your network: **Allow through firewall** in Settings → Sharing opens the port for it (53320). Videos from web pages (YouTube and others) need yt-dlp installed. Apps that don't say what they're playing (some browsers) can't be cast; Cast says so.

## Dictation, a drop shelf, Wi-Fi QR codes and fingerprints · 2026-10-31

- **Dictation.** Press **Super+H**, speak, and press it again: the words are typed where your cursor is. Or hold Super+H while you speak and let go when you're done. It runs on this computer with whisper.cpp; nothing is sent anywhere. Set it up in **Settings → Dictation**: install speech recognition (one password), and download a speech model (Base, 142 MB, suits most people; bigger ones make fewer mistakes but are slower). The island shows when it's listening, with Done and Cancel. Choose the language there too (the system's by default). Other keys or scripts can use `qs ipc call dictation toggle`.
- **Drop shelf.** A place to park files while you get to where they're going. Drag files (or text) and shake the pointer, or hold them at the left or right edge of the screen, and the shelf appears: drop them on it. Later, drag them out one at a time or all together; they stay on the shelf until you remove them, so they can go to more than one place. **Super+Alt+D** opens it too, and while it holds something a tray icon in the menu bar opens it (drop files on that icon as well). Share everything on it to a nearby device in one go. Choose what opens it in **Settings → Desktop & Widgets → Drop shelf**.
- **Share a Wi-Fi network as a QR code.** In Control Center → Wi-Fi, the network you're on has a QR button (and every saved network has **Share network…** in its menu). Point a phone's camera at the code to join. The password stays hidden until you ask for it, and you can copy it. Needs qrencode (setup installs it).
- **Wi-Fi codes in screenshots.** Take a screenshot with a Wi-Fi QR code in it (a router's sticker, a café's sign) and the island offers to join that network, or copy its password.
- **Fingerprints**, for computers with a fingerprint reader. Settings → Lock Screen → **Fingerprint** finds the reader, installs what it needs (one password; some readers, like Dell's Broadcom ControlVault, need packages from the AUR through yay or paru), and adds or removes fingers.
  - **Lock screen:** touch the reader or type your password, whichever comes first.
  - **Password prompts, sudo and the login screen:** a switch each. They wait a few seconds for a finger, then ask for your password. Your password is still needed once after a restart, at the login screen, to unlock your saved passwords.

## Quick Share that works without a shared Wi-Fi · 2026-10-31

Tested with a real Android phone, both ways, on the same Wi-Fi and without one.

- **Sending to a phone works again.** iRice never found phones to send to: it asked Bluetooth for devices that list Quick Share's service, and phones only carry it as data inside their announcement, so none ever matched. Phones on their Quick Share Receive screen now show up in a moment.
- **Fast without a shared Wi-Fi, no password.** When the phone and this computer aren't on the same network, the transfer still starts over Bluetooth and then moves to a direct Wi-Fi link: this computer joins the phone's own link when sending, or makes a short-lived hotspot the phone joins when receiving. Your Wi-Fi pauses during that transfer (the internet too) and comes back by itself straight after. Small shares (a link, a small photo) stay on Bluetooth and don't pause anything. Turn it off in Settings → Sharing → **Fast transfers without a shared Wi-Fi**; transfers then stay on Bluetooth, which is slow.
- Fixed: sending to a phone over its direct Wi-Fi link failed right after the file went: the phone turns off Quick Share's own encryption on that link (the link is encrypted already), and iRice kept using it.
- Your firewall needs to let phones get an address from the hotspot (DHCP). **Allow through firewall** in Settings → Sharing now opens that too (port 67/udp, as well as Quick Share's and LocalSend's ports), and Quick Share runs it for you, without a password, the first time a phone needs it. System health says when it's missing.
- Phones find this computer more reliably over Bluetooth: it advertises more often, marked as discoverable like phones do, and starts afresh after every phone that connected (some Bluetooth chips stopped advertising after the first one).
- A phone that retries a Bluetooth connection is no longer turned away while its first attempt hangs on.
- Setup now turns on **avahi-daemon**, which lets phones on the same Wi-Fi find this computer by name (the package was installed but never started on a fresh Arch install).

## Leaner by default · 2026-10-30

- The Anime tab in the sidebar is off, and the overlay (Super+G) no longer offers the floating image, crosshair and FPS limit. Both are still there: turn them on in Settings → Sidebar (Show Anime tab, Gaming overlays).
- Fixed: over a full-screen app (a video, a game) island alerts opened as an empty glass pill, and volume and brightness showed as a blank shape. Alerts now show in full over the app and slide away when they're done. Choose how in Settings → Menu Bar → Island → **In full screen**: **Show alerts** (the default), **Only important** (only calls, low battery, running hot, timers and alarms, plus critical notifications; everything else waits quietly in Notification Center) or **Compact pill** (a small pill with the alert's icon and one line; click it to see the whole alert). Incoming calls show the island's call card over full-screen apps too, instead of a banner.
- Over a full-screen app, volume and brightness show as a thin bar along the top edge instead of the island or the HUD.
- The glass plugin now fades its glass with the layer it belongs to, so a hidden menu bar never leaves a glass shape behind. Update it to get this: **Update plugin…** in Settings → Appearance → Glass, or run `tools/build-plugin.sh`. The island fix above works without it.

## Fewer password prompts · 2026-10-30

- Software Update asks for your password once, when you press **Update**, and never again in the middle: AUR updates no longer ask at each step.
- Your next update also installs a small iRice system helper (one password, as part of that update). From then on, these no longer ask for your password: the fixes in System health and Settings → Sharing, and using your wallpaper on the login screen. Quick Share's faster Wi-Fi links ask once as they start, never during a transfer. This only applies to an administrator's account, at the computer.
- Until that update, everything asks for your password as before.
- **Settings → Login Items** shows the apps that start when you log in (your own and the system's), with a switch each, and why iRice skips the ones it does (it does their job itself, they're for another desktop, or they aren't installed). **Add app…** picks from your installed apps, **Remove** takes out one you added, **Start now** runs one straight away, and every change can be undone from the toast. **Start them when you log in** turns them all off at once.
- The "Running hot" warning no longer goes off for a few seconds of high load: it waits until the CPU has averaged above your limit for about 30 seconds, and it ends once it has averaged 5 °C below it for a minute (it used to need 3 minutes 10 °C below, so it could stay forever).
- Plugging in or unplugging a display is smooth: the screen you're using no longer goes dark, and a new display starts its connect animation sooner.
- Each display has its own spaces, numbered from 1, like on a Mac. Super+1…9 (and Super+Shift / Super+Alt with a number to take a window along) go to that space on the display you're on, and the menu bar, Overview, Mission Control and the window switcher all show each display's own numbers. A monitor keeps its spaces by what it is, not which port it's in. Your existing spaces stay where they are, windows and all.
- Fixed: an external display's menu bar could show only its current space, even with windows on several of its spaces.
- Unplug a display and its spaces stay together: they move to the laptop as separate spaces, listed after the laptop's own under the display's name (they no longer pile onto the laptop's current space). Plug it back in and they go back to it, showing the space it showed before.
- Overview with several displays: **A row per display** (the default) shows every display's spaces in a labelled row, in the order they're arranged, with each display's current space marked; drag a window to any space on any display. Or pick **Each its own** in Settings → Appearance → Overview to open the Overview on every screen, each showing its own spaces.
- Moving to the other display, by keyboard or by moving the pointer across, now gives the display you moved to a brief soft glow at its edges, and the one you left dims a little for a moment (it used to dim every other display). With **Reduce motion** on, only the glow shows. Turn it off in Settings → Displays.
- **Show which display you moved to** (Settings → Displays) now has three choices: **Glow** (the default: a brief soft glow at its edges), **Dim** (it dims briefly) and **Off**. The display you left no longer dims. With **Reduce motion** on, Dim is a plain short fade. Your earlier on/off choice carries over.
- Overview: moving a window or a space to another display (dragging it to that display's edge zone, or right-click → Move to) no longer acts as if you'd switched displays: the display cue no longer plays, and focus and the pointer stay where you are. Dragging a window or a space against the screen edge that faces another display now moves it there too (the edge display lights up while you hold it there), and the dragged item stays at the edge instead of disappearing behind it. Moves land in one step instead of the miniatures jumping a few times, and windows in a space glide to their new places.
- Island alerts can be put away: swipe one up (drag it, or scroll up over it), use the small × that appears over it, or press Esc while the pointer is on it. Resting the pointer on alerts keeps them up; when it leaves they carry on, with a second to spare.
- Low battery and **Running hot** stay up until you plug in or the CPU cools down (or you dismiss them). A finished download and a newly connected drive stay up for 10 minutes. Everything else goes away as before.
- Island alerts are kept: Notification Center has an **iRice** group with every alert that wasn't already a notification (devices with their battery, drives, displays, sound output, charging, low battery, running hot, downloads, Wi-Fi and VPN…). Remove one with its ×, the group with its own ×, or everything with **Clear all**. The same alert again within a minute updates its line instead of adding another. They're kept when the shell reloads, like notifications.
- The open island has a **Recent** list of the last few alerts. Swipe a line sideways (or use its ×) to remove it, or **Clear** them all. While something is live (music, a timer), **Recent** opens the list in its place.
- The album art in the open island now follows the island's own rounded corners exactly, with no second, rounder corner or inner outline showing at the edges.
- Incoming calls (Discord, WhatsApp, any app) open the island as a call card, like on an iPhone: the caller's photo with a soft pulsing ring, their name, the app, and round red **Decline** and green **Answer** buttons. A ringtone plays while it rings (for about 30 seconds, following your notification sound setting). The card stays until you answer or decline (Esc declines too), or the caller hangs up; a stray swipe or scroll doesn't put it away.
- WhatsApp Desktop can't take calls on the computer, so its button reads **Open** and brings up WhatsApp (answer on your phone).
- A call you didn't answer goes into the island's Recent list and Notification Center as **Missed call**, with the caller's name and a button to open the app.
- After you answer, a small **Call** live activity with the app's icon and the time shows while the call's window is open (or the app is using the microphone).
- With the island switched off (or its Calls alert off in Settings → Menu bar), calls show as a banner that stays until you answer, decline or dismiss it, as before.
- Apps run natively on Wayland again, as they did before iRice: WhatsApp, Discord, Claude, Slack and other Electron apps take files dragged in from Dolphin and behave like other windows. Log out and back in once for it to apply.

- Dock and app icons in the Clear and Tinted styles are the same size: every logo is placed by what it really draws and drawn at the same height, a little smaller than before, so a row of them lines up top and bottom and wide logos (Webex) and small ones (Zen) match, and none overflow their tile. VS Code no longer shows a box inside its tile, and logos are brighter on dark glass.
- **Reduce motion** in Settings → Appearance → Accessibility: fades instead of movement for island alerts, connecting a display and the morning screen.
- App icons always get their tile after you plug in or unplug a display (one could stay unstyled until a restart).
- Claude Desktop: its Quick Entry shows just its prompt bar (no blurred panel around it) and stays open while you move the pointer, until you send or press Esc. It no longer asks for your wallet password every time it starts (sign in to Claude once more after updating).
- Report a Bug works: reports go to iRice's issue tracker.
- Buttons, text fields, list rows, Settings groups, menu highlights, search fields and cards now have the same smooth, continuous corners as the glass (round pills and circles stay round). Control Center's tiles and sliders also show their colours as intended: a little lighter on dark glass and whiter in Light mode (they came out darker than meant before).
- The display connect animation waits until both screens show a picture again (a monitor takes a moment to switch on), so you see it play instead of it finishing on dark screens.
- In a fullscreen app (a video, a game), an incoming call shows as a banner above it, since the island is hidden there.
- A monitor with speakers is listed in Control Center → Sound while it's plugged in (e.g. **BenQ GW2786TC speakers**): picking it moves sound to the monitor, and **Built-in speakers** brings it back. The laptop's microphone stays on.
- The installer checks which Hyprland your system is about to get. When it's newer than this iRice is tested with (Arch has 0.57 while iRice is tested with 0.56), it says so and lets you continue anyway (the default) or stop before anything changes. `irice doctor` and System health then point it out until an iRice update supports it.
- Your keyboard layout is kept: the installer uses the one your system is set to (`localectl`), instead of always US. The iRice test session uses it too.
- The idle sliders in Settings → Battery & Power work after installing: the installer no longer puts a hypridle.conf of its own in `~/.config/hypr` (which made Settings think you had your own, and replaced yours). If an earlier install left its copy there unchanged, it's taken out the next time you install or update; a hypridle.conf you wrote yourself is kept and still wins.
- When your repositories don't have Quickshell, or only an older one, the installer builds iRice its own instead of stopping. On Manjaro, a Hyprland that's too old comes with a note on why (its stable branch lags behind Arch) and what to do.
- System health's Quickshell fix now names the build script to run (Software Update doesn't rebuild Quickshell), and an installed iRice reads its tested versions again.
- Replacing PulseAudio no longer fails when only part of it is installed.
- With no login screen, Hyprland starts from tty1 whichever shell you log in with: bash, zsh, fish or sh (it used to be bash only).
- TLP, auto-cpufreq and tuned are left in charge: neither the installer nor Software Update adds power-profiles-daemon next to them (TLP's package would have been removed), and System health shows energy modes as managed by them instead of broken.
- Switching a network run by iwd over to NetworkManager keeps iwd underneath, with your saved Wi-Fi networks. When iwd or systemd-networkd runs your network, System health says so instead of offering a fix that would fight it.
- The Dolphin "Open with" list uses Plasma's menus when they're installed, else Arch's (it was Plasma's even when missing).
- The keyring only starts when nothing else (KWallet, KeePassXC, a keyring unlocked at login) already keeps your app passwords.
- No second snapshot before an update when snap-pac (or timeshift-autosnap) already takes one.
- The installer adds hyprpm (and what it builds plugins with) where it's a package of its own, for the optional title bars.
- Wi-Fi without NetworkManager: when iwd, ConnMan, wpa_supplicant or systemd-networkd runs your network, the Wi-Fi menu, Control Center and Settings → Network say so ("Wi-Fi is managed by iwd") and offer that manager's own app when one is installed (iwgtk, impala, CMST…), instead of an empty list. The menu bar still shows whether Wi-Fi is connected. The Wi-Fi control leaves Control Center then, and its gallery lists it greyed out with the reason, like other controls this computer can't use.
- Wi-Fi, VPN and Ethernet alerts in the island work without NetworkManager too (they watch the network interfaces, including WireGuard, Tailscale and other tunnels). With nothing to watch them with, the option in Settings → Menu bar is greyed out and says why.
- With TLP, auto-cpufreq, TuneD or system76-power in charge (or no power-profiles-daemon), Battery & Power, the battery menu and Control Center say "Managed by TLP" (or what to install) instead of showing energy modes that do nothing; the battery saver, Game Mode and cool-down no longer claim to switch to Low Power.
- Bluetooth says what's wrong: no adapter, BlueZ not installed, the Bluetooth service stopped (with **Start Bluetooth**), or blocked with rfkill as in flight mode (with **Unblock Bluetooth**, no password needed).
- About 10 seconds after iRice starts, one island alert names anything else doing its jobs: another bar (waybar…), another notification daemon (mako, dunst…), a second hypridle or clipboard recorder, or user services that start them. **Review** opens System health, which lists each with how to stop it; **Don't show again** remembers it per thing.
- Commands that open in a terminal (installing an emoji font, Spotlight's commands, terminal apps) use the terminal chosen in Settings → Default apps, else $TERMINAL, else the first one installed: kitty, foot, Alacritty, WezTerm, Ghostty, Konsole, GNOME Terminal, Console, Ptyxis, Xfce Terminal or xterm, each started the way it expects. They no longer assume kitty.
- On a desktop without adjustable display brightness, the brightness keys and menu bar scrolling no longer show a brightness slider that does nothing, and Spotlight answers "No adjustable display brightness". With several backlights, the one that drives the panel is used.
- Night Light only appears in Control Center when hyprsunset is installed. Emoji fonts install with yay or paru, whichever you have. Recording's microphone list works without pactl. Without NetworkManager, Settings → Network leaves out the Wi-Fi security it can't know instead of guessing. Installed tools are looked for again when System health refreshes and after an update.
- Overview with **Each its own**: every screen shows the other displays as small labelled targets on the edge that faces them (a display to the right sits at the right edge). Drop a window on one to move it to the space that display is showing, or hold it there a moment to pick one of its spaces. Drag a space onto one to move it there with all its windows, as that display's next space. Both screens' Overviews update right away.
- Right-click a window in the Overview for **Move to** another display (or one of its spaces), and a space for **Move space to** another display. It works with a row per display too.
- Screen recording on a computer without hardware video encoding (Intel without intel-media-driver, NVIDIA without libva-nvidia-driver) no longer just fails: iRice records again with the CPU and says which package brings hardware encoding back.
- External monitors' brightness: with ddcutil installed, the brightness keys, the Control Center slider and the OSD change the brightness of the external display you're working on (DDC/CI), and Settings → Displays has a slider for it. When ddcutil can't reach the displays (i2c-dev not loaded, no access to /dev/i2c-*), Settings → Displays says how to fix it.
- The GPU graph works on NVIDIA's driver (through nvidia-smi, only while a graph shows, without waking a sleeping GPU) and shows its temperature.
- Volume and brightness without media keys: Super+F11/F12 lower and raise the volume, Super+F1/F2 the brightness (Settings → Keyboard Shortcuts, Media).
- The lock screen says "Too many attempts: try again in about 10 minutes" (with the real time left) when too many wrong passwords have locked the account, instead of "Incorrect password" for a password that was right.
- Text Capture reads your system language as well as English when its tesseract-data package is installed (German with tesseract-data-deu, say), and tells you which package to install when it isn't.
- Sharing behind an nftables or iptables firewall: System health and Settings → Sharing notice it, and the fix lets LocalSend and Quick Share through (kept across restarts when nftables reads /etc/nftables.d, otherwise until the next restart, which System health says). Quick Share's direct Wi-Fi links get through nftables too.
- Your keyring unlocks with your login password at the iRice login screen and on the console (gnome-keyring's PAM lines, added once by the installer, taken out by irice uninstall). Not next to KWallet.
- Apps that start with your session (~/.config/autostart, /etc/xdg/autostart) now do, once per session, after the menu bar's tray is up. iRice leaves out the ones it does itself (keyring, password prompts, notifications, other bars).
- NVIDIA: the installer writes the environment Hyprland's wiki asks for (~/.config/hypr/irice/gpu.lua; only suggestions on hybrid graphics), and installs the video drivers for recording and playback: intel-media-driver for Intel, libva-nvidia-driver for NVIDIA.
- Your old hyprland.conf: after installing, iRice says what it had (displays, keyboard, env lines, programs) and puts them, turned off, in ~/.config/hypr/irice/from-your-old-config.lua: remove "-- " to use one.
- Bug reports also leave out Wi-Fi names iwd knows. Quick Share under iwd says Wi-Fi Direct needs wpa_supplicant instead of asking for a password for nothing.
- System health finds any Bluetooth adapter (not only the first), says to install bluez when it's missing, and counts PipeWire without pipewire-pulse as a warning. qt6ct is used when you've set it up and KDE's theme isn't installed or your environment.d asks for it. The installer uses paru when you have it, and on Manjaro builds yay only when you say so.
- LocalSend keeps finding nearby devices after a malformed message from the network; before, one bad message stopped discovery until the shell restarted.
- LocalSend: a request nobody answered now says "Request from <name> expired · Nothing was received", and declining one yourself shows nothing extra. "They declined" is only for files you sent.
- Leftover watchers from a crashed shell are only ended when they're provably iRice's: each watcher group now carries its own random mark, and after its leader has gone only processes with that mark are stopped. Nothing else in a reused group is ever touched.
- Calendar subscriptions show more repeating events on the right days: "the 4th Thursday of November" (Thanksgiving), "the last (or first) weekday of the month", every-other-week events with a week start other than Monday, and daily events on weekdays only. An event that repeats in a way iRice can't follow yet shows only its first date, and its details say so, instead of landing on wrong days.
- The app icon list notices icons added inside icon theme folders that are links to another theme (MacTahoe-dark and others), instead of staying out of date for a week.

## Sharing and spaces fixes · 2026-10-30

- Phones show up when you send: while iRice looks for devices it tells phones nearby that someone is sharing, so they make themselves visible (as one phone does for another).
- "Send to a nearby device…" appears in Dolphin and other file managers again (it was added where they didn't look).
- **Send files…** in Control Center: on the Sharing page and in the Sharing tile's right-click menu.
- An empty space between two in use (say 6, between 5 and 7) keeps its chip in the menu bar and its place in Overview, so you can still go to it.
- Fewer warnings in iRice's log (notification banners, the window snap outline, saved settings).
- Updates run in the background at low priority, so the desktop stays smooth while packages install or AUR software compiles.
- One island alert per thing: headphones whose battery shows up a moment later, or a device that reconnects, update the alert already showing instead of adding a second.

## Quick Share: firewall, Wi-Fi Direct and pairing fixes · 2026-10-30

- If your firewall (ufw or firewalld) blocks sharing, Settings → Sharing and System health say so, with an **Allow through firewall** button.
- Quick Share now always listens on port 53318, so one firewall rule covers it.
- The firewall check recognises port ranges, so a firewall that already lets sharing through isn't reported as blocking it.
- Wi-Fi Direct starts again (it was refused by the Wi-Fi service).
- Wi-Fi Direct no longer ends the moment it starts: NetworkManager leaves the link alone while a transfer uses it.
- Quick Share no longer asks for your password for a Wi-Fi Direct link that can't work: NetworkManager ends such links the moment they start, and Intel Wi-Fi can't run a hotspot on 5 GHz. For full speed, have the phone on the same Wi-Fi.
- No more hotspot attempts on 5 GHz Intel Wi-Fi, where they can't work; transfers move on to the next option straight away.
- Phones no longer need to ask to pair: **Stop pairing prompts** in Settings → Sharing turns off the experimental Bluetooth features that caused it (Ranging, and LE Audio controls that made phones take the computer for earbuds).
- A phone shows once in the Bluetooth lists, not once per connection.

## Battery & Power, System health, replies and more · 2026-10-30

**Battery & Power** (new in Settings)
- Choose when the screen dims, turns off and locks, and when the computer sleeps, separately on battery and on the charger.
- Closing the lid: sleep, lock or do nothing, on battery, on the charger, or with another display connected. Choose when the password is needed after sleep.
- One energy mode, with the reason shown: Cool-down, the battery saver, your choice and Game Mode no longer fight each other.
- At 20% the island offers Low Power (or turns it on by itself if you choose). Warnings at 10% and 5%, and the computer sleeps at 3%, even with Caffeine on or if iRice isn't running.
- See which apps used the most energy in the last hour, and quit them.

**System health** (Settings → About)
- Checks every service iRice relies on (sound, network, Bluetooth, sharing, power, portals, the glass plugin…) and says how to fix what's broken. Fix restarts things for you, and asks for your password when it needs to change system services.

**Notifications**
- Reply to messages (Discord, Telegram, Signal, KDE Connect…) right from the banner, Notification Center or the island. Super+Ctrl+N replies to the newest message.
- Notification cards no longer show through the Notification Center header.
- Calendar, To-do and Focus tabs now sit inside their card.

**Google**
- Connect several Google accounts; their calendars show together.
- Create, edit, move and delete Google Calendar events. Repeating events ask "This event" or "All events".
- Changes made offline are saved and sent later. If something changed on Google meanwhile, you're told and can copy what you had.

**Keyboard**
- Every button, switch, slider, list and menu works with Tab, the arrows, Space and Enter, with a focus ring only when you use the keyboard. Screen readers hear each control's name.

**Island and sharing**
- One scroll or swipe over the island moves to the next activity.
- Downloads: no more stuck "Unconfirmed … 0 B" card; a finished download offers Open and Show in Folder.
- The Sharing control in Control Center is a pill like Wi-Fi: tap to turn on for 10 minutes, or open its menu for No one / 10 minutes / Everyone.

## Quick Share: fast transfers, fewer alerts · 2026-10-30

- Quick Share now moves files over a direct Wi-Fi link instead of slowly over Bluetooth: NetworkManager no longer takes the link down as it starts, and the phone is told the link's real channel. If the direct link can't start, a hotspot on your current channel is tried next.
- While Quick Share is on, phones connecting and disconnecting over Bluetooth aren't announced in the island; Quick Share's own prompt and progress say what's happening.

## Wi-Fi and Bluetooth everywhere · 2026-10-29

- Right-click a Wi-Fi network or a Bluetooth device, in the menu bar, Control Center or Settings, for Connect, Disconnect, Forget, Copy name and more. Press and hold works too.
- Long lists of networks and devices scroll instead of running off the screen.
- Settings → Network lists your Wi-Fi networks and lets you join them (including office and hidden networks), with Known networks to forget.
- New Settings → Bluetooth: your devices, nearby devices to pair (found while the page is open), and how this computer appears to others.

## Quick Share over Bluetooth fixed, apps come forward · 2026-10-29

**Quick Share**
- Phones sending over Bluetooth disconnected after two seconds and reported "failed". iRice now answers the phone's connection in time, and its Bluetooth announcement fits your Bluetooth chip.
- If a phone paired with this computer during an earlier try, remove that pairing on both devices first; Quick Share doesn't need it.

**Apps come forward**
- Clicking a link, or "Open in the app" in the browser, brings the app forward even with focus following the pointer. The pointer moves onto it, so focus stays there.
- An app that asks for attention while you're busy elsewhere doesn't steal focus: its dock icon bounces and the island offers Show.
- File pickers and sign-in windows open centred, on top of Settings. While you sign in to Google, Settings waits in the dock and comes back by itself.

## iRice's own Quick Share, enterprise Wi-Fi · 2026-10-29

**Quick Share with Android, built in**
- iRice has its own Quick Share; no extra app is needed. Turn it on in Settings → Sharing or in Control Center: No one, Everyone for 10 minutes, or Everyone.
- Works like it does between phones: nearby phones find this computer over Bluetooth, even without the same Wi-Fi, and files then move over a fast direct Wi-Fi link while your normal Wi-Fi stays connected. On the same Wi-Fi, they simply use that.
- You're asked in the island before anything is saved, with the same PIN the phone shows. Files, links, text and Wi-Fi networks all work.
- A direct Wi-Fi link needs your password once; if you skip it, the transfer still finishes over Bluetooth, slowly.
- Needs Bluetooth on. Settings has a button to turn it on.

**Sharing everywhere**
- The share sheet lists every nearby device in one place: Android phones over Quick Share, everything else over LocalSend.
- A new Sharing control in Control Center, in four sizes: a toggle, Off · 10 min · On, and larger ones with nearby devices.
- Share… in the right-click menu of the desktop, the Downloads stack, and Nautilus, Dolphin, Thunar and Nemo (add or remove them in Settings → Sharing).
- Share selected text with Super+Ctrl+S.

**Report a bug**
- Settings → Report a bug, or from Spotlight, a shortcut or a gesture. Add a screenshot (with Mark up… to hide anything), the iRice log and system info, and see exactly what will be sent first. Personal details like your name, paths, network names and addresses are removed from the log.

**Google**
- Connect Google account opens your browser and comes back connected; no detour through GNOME Settings.
- Sync only fetches what changed: at login, when you unlock, when you open Notification Center, and every 15 minutes while you're using the computer. Nothing while it's locked or idle. Choose which calendars to show.

**Wi-Fi**
- Join office, college and eduroam networks: pick the sign-in method (PEAP, TTLS, TLS or PWD), and use the system's certificates or choose a certificate file. For TLS, add your own certificate and key.
- Join other network… at the end of the list joins a hidden network.
- Joining a phone's hotspot works: Join no longer misses your click while the hotspot comes and goes in the list. If the password is too short, Join says so.

**Fixes**
- Tapping Super twice quickly no longer leaves a half-open Overview that ignores clicks.

After updating, run Update again so it installs the new packages (`iw`, `wpa_supplicant`, `dnsmasq`, `avahi`, Python libraries).

## Fixes after review · 2026-10-29

**Calendar**
- A monthly repeating event in any calendar stopped every Calendar widget from showing, and emptied Notification Center's calendar. Fixed.
- In Notification Center, a busy week shows a few items per day and "+N more", so it no longer stretches the panel.

**Google**
- Edits and deletions always reach the task you meant, even while a sync is running. Changes show at once and are sent when Google is free.
- Deleting a Google task can be undone for 8 seconds.
- If the Google helper can't run, Settings says what to install.

**AI**
- An API key is only sent over plain http:// to this computer. Remote servers need https://.
- Removing a model also removes its key from the keyring, with 8 seconds to undo.

**Desktop colours**
- The Dolphin colour fix no longer replaces a Qt or KDE colour scheme you picked yourself.

**Battery and stability**
- Background helpers left behind by a crashed shell are cleaned up again.
- Less background work: state saving, the icon index and download progress all cost less.
- Glass shapes no longer stay on screen after the shell quits.
- A damaged state file is set aside instead of stopping settings from saving.

**Lock, sleep and login**
- The Locking animation setting works again.
- If the screen can't be locked, the computer stays awake and tells you, instead of not sleeping without a word.
- The login screen has clickable Sign in and power buttons, and plain error messages.

**Other**
- Spotlight's Lock Screen locks straight away again.
- A new window no longer pulls focus or the pointer when your window rules say it shouldn't.
- The Today screen shows your events and to-dos when it opens at login.
- Sharing settings show Quick Share's status in one row.

## Glass calendar and full weekly lists · 2026-10-29

- The large Calendar widget keeps its glass background. The list of the day's events now slides up over the calendar, which fades and blurs behind it.
- The week view lists everything on the desktop widget. Add events and to-dos with the + button.

## Calendar containment and scrolling agenda · 2026-10-29

- The large Calendar widget stays inside its frame in every view. Scroll to see all of a day's events and to-dos; the arrow at the top brings the dates back.
- Scrolling over the widget no longer changes the month. Use the arrows for that.

## Super-tap correction verified in the running session · 2026-10-29

- Tapping Super on its own opens Overview again. Super with another key works as before.
- Dolphin and other Qt apps keep readable colours after an update restarts the shell. Quit and reopen Dolphin once if it still looks wrong.

## Audit fixes and reliable startup · 2026-10-29

**Lock and sleep**
- The screen locks immediately, before any animation, and Sleep waits until the lock is really in place.
- Locking and sleeping when idle now works in every iRice session.

**Your data**
- Settings and UI state survive crashes and two shells writing at once. Notes are saved safely and you're told if a save fails.
- Updates back up your configuration first and put things back if something goes wrong.

**Sharing**
- LocalSend uses HTTPS and checks the other device's certificate. Devices with HTTPS off must turn it on to receive.

**Login screen**
- A new, simpler login screen: pick the user and session, type your password.

**Fixes**
- Repeating calendar events on month ends, leap days and intervals.
- Tiled or floating at startup, timers that drifted, and results from an older search replacing newer ones.
- Notifications no longer offer a Reply that didn't work.

After updating, run Update once more so it installs `openssl` and `plasma-integration`, then log out and in again.

## Pointer focus and multiple displays · 2026-10-29

- With focus following the pointer, the window under it gets focus again after you switch spaces or a window closes.
- Each display's current space stays visible in the menu bar's spaces, even when there are more spaces than fit.
- Settings → Mouse: stop the pointer slipping to another display at shared corners or edges, with an adjustable corner size.
- A screenshot selection stays on the display where you started it.

## Google integration and clipboard image reuse · 2026-10-29

- **Settings → Google:** sign in with your Google account (through GNOME Online Accounts; no Google Cloud project needed). Your Google Calendar events and Google Tasks show in the Calendar, Reminders and Today widgets, and you can edit tasks.
- Google Keep opens in the browser; it can't be synced.
- The clipboard panel loads image previews once and keeps them, and previews every image in your history.

## Your own AI models · 2026-10-28

- Settings → AI now lets you add, edit, remove and select models with a display name, model ID, provider and credentials. New installations have no preset models; existing selected models, extra models and provider keys are preserved.
- Supports Google AI Studio, Vertex AI (Gemini, with an Express API key or OAuth access token), OpenRouter, Anthropic, OpenAI, Ollama and custom OpenAI-compatible servers. Connection-specific fields appear as needed.
- Model selection is shared by sidebar chat and Spotlight. The same model ID can be used across different providers and servers, with credentials stored separately from model definitions.

## Dolphin colors and easier file sharing · 2026-10-28

- Dolphin's text and background now follow iRice's light/dark mode even with Match apps off. KDE and Qt palettes stay consistent, including after turning accent matching off.
- Settings → Sharing now has a Send files button with multiple-file selection. Spotlight also has a Send Files action. LocalSend sending works independently of receiving.
- Added optional Android Quick Share / Nearby Share integration through Packet, available from the share sheet, Sharing settings and Spotlight. Packet handles device selection, confirmation and progress; it requires Bluetooth and the same Wi-Fi network.

## Keyboard power confirmation and window focus · 2026-10-28

- Spotlight's power commands open the session menu with the requested action selected. Use the arrow keys to change it, Enter to confirm, or Esc to cancel. Mouse clicks only select an action, and holding Enter cannot accidentally confirm it.
- App windows can take keyboard focus when they open or request activation, without waiting for the mouse to move.

## Island closes without glitches · 2026-10-28

**Fixes**
- When the island closes, its contents and pin button go at once instead of being cut off by the shrinking edges, and its background fades along with the shape instead of stalling and then popping.

## Island closes cleanly · 2026-10-28

**Fixes**
- Closing the island no longer turns it into a round blob or cuts off its contents. It shrinks smoothly back into the small pill.
- The system stats row in the open island is centred.

## Smoother island · 2026-10-28

**Fixes**
- The album art no longer reloads and flickers each time you open the music player in the island.
- The volume and brightness island is a pill from the start, instead of a rectangle that turned into one.

## Hover tips across the menu bar · 2026-10-28

- Point at a menu bar icon for a moment to see what it's doing:
  - **Wi-Fi:** the network and signal.
  - **Bluetooth:** what's connected and each device's battery.
  - **Sound:** volume and output.
  - **Others:** weather, Focus, keep awake, recording time, the song playing, keyboard layout, app tray icons, and the full date with unread notifications.
- Icon-only buttons (Search, AI, Control Center and the extra buttons) show their name.

## StandBy stays centred, and update conflicts you decide · 2026-10-28

**StandBy**
- The clock now always stays centred across the screen. To protect against burn-in it only moves up and down, a good distance each minute.

**Software Update**
- A file that both you and the update changed is now listed for you to decide: **Merge both** (when the two changes fit together cleanly), **Keep mine** or **Use the update's**. Before, your version was kept without a word and the update's change was lost.
- Also listed: files deleted on one side and changed on the other, binary files, and a file of yours with the same name as one the update adds.
- Whenever you choose the update's version, yours is saved in the update's backup folder first.

## Cooler, safer updates, and fixes · 2026-10-28

**Battery and heat**
- Fixed: the screen-sharing detector kept a CPU core busy, and every shell restart left another copy running, which made the laptop hot. It now costs almost nothing, and nothing iRice starts in the background outlives the shell any more. A new shell also ends anything an earlier one left behind.
- Lighter background checks for the camera, location and file copies.
- Stage Manager previews pause when hidden and are capped at 5 frames a second. A live wallpaper pauses when the screen is locked or idle.
- The island's music bars and heat ripples redraw less often, and pause when hidden or locked. The ripples stop entirely during Cool-down.
- The whole shell now idles below 1% of one core.

**Software Update**
- Only one update runs at a time, across every iRice session.
- A failed iRice update puts everything back exactly as it was, your own changes included.
- Package lists from an update are checked as plain text and never run. Only valid package names reach pacman.
- AUR updates are now a choice in the plan (off by default), because AUR packages are built from user-submitted scripts.
- Once your password is accepted, a system update can't be cut off halfway by the shell restarting or closing.

**Fixes**
- The microphone stays muted after a recording if you muted it during one: fixed.
- Discarding two recordings in a row left the first file behind: fixed.
- The Today screen could open at 4 AM if you were still at your desk, and then not in the morning: it now waits for your next login or unlock.
- A removed Today widget no longer comes back after a restart.
- A display-connect animation could replay when another screen's finished: fixed.
- Clipboard times were missing after a restart until the next copy: fixed.
- Green and orange accents get dark text on their buttons, for contrast.
- Quit leaves apps alone when it can't tell whether a "Save changes?" window is open.

## Today, a smarter accent, and apps that behave · 2026-10-28

**Today**
- On your first unlock after 4 AM, a glass panel greets you with today's events, your to-dos and the weather (now, high and low, rain, and the next hours), with a rising chime timed to the animation.
- Choose how it appears (Bloom, Shrink or Drop) and whether it shows every morning, as a desktop widget, both, or not at all: Settings → Desktop & Widgets → Today.
- **Pin for today** keeps it on the desktop until midnight. Set the name it greets you by in Settings → About.

**Accent colour**
- Your accent is used where it reads, a lighter shade of it on dark backgrounds when needed, and neutral otherwise. It's never darkened, so yellow no longer turns brown.
- Control Center looks like macOS: an "on" button is a white circle with the icon in your accent, or in near-black when the accent can't be read on white.
- Desktop widgets (clock, calendar, reminders, Screen Time, stocks) use your accent.

**Desktop calendar**
- The Calendar widget now works like Notification Center's: the month with busy days, Month and Week views, quick add, and a To-do tab where you can add, rename, tick off and delete (with Undo).

**Clipboard (Super+V)**
- Grouped by when you copied things: Just now, hours ago, Yesterday, weekdays, then dates.
- Bigger image previews, and a two-column grid on the Images tab.
- Fixed: the panel could be empty again after an update restarted the shell.

**Battery**
- The battery menu shows battery health (maximum capacity, condition, and cycle count when reported) and how much power is going in or out.
- Point at the battery for its charge, time left and energy mode.

**Glass and icons**
- New glass optics in Settings → Appearance → Glass → Advanced…: index of refraction, refraction width and dispersion, with a live preview.
- Clear and Tinted app icons are see-through glass tiles with one-tone logos that read evenly across apps; Tinted colours them with your accent.

**Apps and windows**
- Apps no longer open maximized over everything; allow it per app in Settings → Windows.
- Menus and dropdowns in apps like Claude and Webex stay open when you move the pointer.
- Webex shows its own name and icon. Quit now ends apps that keep running in the tray.

**Fixes**
- Sliders no longer scroll the page when your pointer drifts while dragging.
- Brightness and volume icons match the rest of Control Center.
- The island's status row shows system stats only; your events moved to Today.

## The update screen waits for your desktop · 2026-10-28

**Fixes**
- After an update, the screen could go black for a few seconds before your wallpaper and menu bar came back. The update screen now stays until your desktop has drawn, then fades out over it.
- Clicking the update screen while it works does nothing.

## One update for everything, a screen recorder, iOS-style Control Center · 2026-10-28

**Software Update**
- One **Update** button now updates your system and iRice together, in a safe order:
  1. a snapshot first, if snapper or Timeshift is set up;
  2. your system (`pacman -Syu`), with the packages iRice needs;
  3. AUR packages, if you use yay or paru;
  4. iRice itself, rebuilt for the Hyprland you now have.
- Your password is asked in iRice's own dialog. No terminal opens, and the update keeps going if you close Settings.
- **Show details** shows the full output while it runs. The island shows progress.
- Nothing is checked on its own any more. Press **Check for updates** when you want to.
- If Arch has a newer Hyprland than iRice is tested with, you choose each time: keep the current one for now, or update anyway.
- These notes, and a list of the packages each update touches.

**Screen recording**
- Press Super+Shift+5, or Record in Control Center, to open a recording bar: Entire screen, Window or Selected region.
- Options: a microphone (pick which one), system audio, the mouse pointer, and a 3 or 5 second countdown.
- While recording, the island shows the time with Stop. Open it for Pause, Mic mute with a level meter, and Discard (8 s to undo).
- Recordings are saved to `~/Videos/Screen Recordings` as MP4 at 60 fps. Change this in Settings → Desktop & Widgets → Screen recording.
- Uses [gpu-screen-recorder](https://git.dec05eba.com/gpu-screen-recorder/about/), which encodes on the graphics card. Without it iRice falls back to wf-recorder: no pause, one audio source.

**Control Center**
- Wi-Fi, Bluetooth and the other wide controls are pills. When on, the button turns white with a coloured icon.
- Square tiles like Now Playing use iOS rounding, with continuous corners.
- Brightness and volume are capsules without labels. In Edit Controls, choose wide or tall for each.
- CPU & Memory opens a page with graphs of the last minute, in four rows: processor and temperature, graphics, memory and swap, and network down and up.
- A speed test, in the Wi-Fi menu and in Settings → Network.

**Menu bar and island**
- The battery is drawn like iOS 27: a solid capsule, red when low, yellow in power saver, with a bolt or plug when charging. Show the percentage inside it, beside it, or not at all in Settings → Menu Bar.
- Wi-Fi, Bluetooth and the Control Center icon match it.
- Privacy dots are only for the microphone, camera and location. Screen sharing is detected from the actual stream and shown in the island or as a dot, your choice, with **Stop sharing**.
- Spaces with windows show as short pills, empty ones as dots. With more spaces than shown, choose arrows or dots that shrink and fade at the ends.
- 18 more menu icons to choose from, in Settings → Appearance.

**Displays**
- Plugging in a display plays a short animation from the side it's on: Extend (a soft, blurred wipe), Liquid (a drop that grows to fill the screen) or Warp.
- An optional sound comes from that side. Try them in Settings → Displays.

**Windows and spaces**
- New windows open tiled or floating, your choice, with exceptions per app in Settings → Windows. Dialogs always float, and switching rearranges what's open.
- A macOS-style Overview: a strip of your spaces on top and search in the middle. The space wallpaper is now a setting, off by default.
- Live window previews are capped at 30 frames a second, and only when Live is chosen. Mission Control and Stage Manager follow the same setting.

**Settings**
- Grouped like System Settings, with a toolbar (back, forward, and where you are).
- Long pages open subpages, like Appearance → Glass.
- Search stays at the top, folds to a magnifier when you scroll, and ends when you click anywhere else.

**Look**
- Glass surfaces have continuous (squircle) corners. The dock is a rounded rectangle, with a slider for the space between icons.
- Clear and tinted icons lift each logo off its tile, with a contrast slider.

**Fixes**
- The clipboard panel was empty in the iRice login session. It now shows your history.
- The screen-sharing dot no longer flickers when iRice draws its own window previews.

<!-- packages: +gpu-screen-recorder -->

## Spaces you can reorder, and a tap of Super opens Overview · 2026-10-28

**Spaces**
- With more spaces than **Spaces always shown**, the menu bar shows a window of them with arrows at the ends. A dot under an arrow means a space past it has windows.
- Drag a space in Overview or Mission Control to put it somewhere else. Its windows move with it.

**Keyboard**
- A tap of Super on its own opens Overview. Super held with another key works as before.

**Dock**
- The dock is a wide glass slab with softly rounded corners and more room around the icons, like macOS Tahoe.
