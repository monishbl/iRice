# shell/modules/settings/pages/network.qml ↔ surfaces/settings (pages/devices/network.rs `network`, ui/settings-devices.slint `list` / `join` / `share` / `speed` / `signal` / `btn-switch`)

The page embeds controlcenter/WifiList.qml (with ListRow.qml, PointerMenu.qml,
WifiJoinForm.qml, WifiShareCard.qml, ServiceNotice.qml) and network/SpeedTestCard.qml.
Those files are tracked by the CC / panels groups for their own surfaces; the
Settings Rust redraws them in settings-devices.slint, so what they do *on this
page* is listed here. Bluetooth (PagePart) is in shell_modules_settings_pages_bluetooth.qml.md.

Data: RustBridgeDevices.qml `feed.wifi` (networks sorted connected/known/signal,
max 30; known; join; saved; share), `feed.backends`, `feed.speed` (SpeedTestCard's
texts), `feed.speedTest` (link, history, ip…); Wi-Fi on/present also from
tessera-ui's status service. QQC.Popup: WifiJoinForm's "Inner authentication"
Dropdown (common/Dropdown.qml → QQC.Popup); Rust `.choice()` → ActionMenu xdg_popup.
The right-click menu is PointerMenu → glass/ContextMenu.qml (a GlassWindow at the
pointer, not a QQC.Popup); Rust uses the ActionMenu popup.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On show: SpeedTest.probe(), readLink(), WifiJoin.loadKnown() (22–26) | bridge onPageChanged "network" | same |
| 2 | Read the link again every 5 s while shown (28–33) | bridge Timer (page === "network") | same |
| 3 | Scan only while the list shows (WifiList 132–138) | bridge Binding scannerEnabled | same |
| 4 | Wi-Fi row (not unmanaged): detail "No Wi-Fi adapter found" / "Connected to X" / "On, not connected" / "Off" (45–48) | network.rs 441–448 | same |
| 5 | "Scan again" (arrow-clockwise) while on → rescan (scanner off/on + refresh) (51–57) | `btn-switch` "button" → wifiRescan | same |
| 6 | Wi-Fi Switch (59–63) | `btn-switch` toggle → wifiEnable | same |
| 7 | Footer "Right-click a network…" unless unmanaged (44) | network.rs 573 | same |
| 8 | List shows when on or unmanaged; placeholder "No Wi-Fi adapter" / "Looking for networks…" (66–78; WifiList 224–233) | network.rs 514–517, DevList placeholder | same |
| 9 | Unmanaged: ServiceNotice — centred 24 px wifi-slash icon (tertiary), title, reason, centred button "Start NetworkManager" / "Open <app>", busy while fixing (WifiList 195–204; ServiceNotice.qml) | network.rs 463–472: an ordinary Row with a leading icon and a right-hand button | **different** (layout) |
| 10 | Rows are a snapshot: re-sorted every 4 s only when the pointer isn't over the list, no password/form open; a vanished network is dropped at once; rescan forces it (WifiList 57–101) | rows follow `feed.wifi.networks` live (the bridge re-sorts on every signal change) | **missing**: rows reorder under the pointer |
| 11 | Row (ListRow): strength icon high/medium/low; connected highlighted; status Connected / Connecting… / Couldn't connect / Known network; lock if secured; busy spinner (Material Spinner 22 / Glass circle-notch 800 ms turn) (252–261) | `wifi_list_rows`, slint ListItem 63–185 | same |
| 12 | Material segmented rows: 2 px apart, ends r20, inner 4 → 12 hover → 16 pressed/highlighted (springFast), tint 8 %/10 %, colour anim; ElasticControl press swell (ListRow 107–126) | slint ListItem 84–93 (spring(0.6) 250 ms) | different: no press swell (ElasticControl); spring is an approximation |
| 13 | QR action button on the connected (non-enterprise) network, "Share this network" (257–259) | ListItem `has-action` GhostIcon | same (GhostIcon a11y/focus gaps: see sound row 5) |
| 14 | Click: connected → disconnect; known/open → connect; enterprise → join form; else toggle the password row (262–272) | `activate` 174–191 | same |
| 15 | Right-click or press-and-hold 550 ms (left) opens the menu; a held press doesn't click (ListRow 205–225) | ListItem: right-button up only | **missing**: press-and-hold |
| 16 | Keyboard: Tab to a row (FocusRing), Space/Enter click, Menu key / Shift+F10 menu, ↑/↓ next/previous row (ListRow 74–100) | ListItem has no FocusScope | **missing** |
| 17 | Menu at the pointer, its left edge at the pointer (PointerMenu 294) | ListItem 150: anchor (x−110, y, 220, 0); lib.rs open_menu offset (aw − w).min(0) right-aligns | **different**: menu opens left of the pointer |
| 18 | Menu items: Disconnect / Connect, Connect with password…/sign-in…, **separator**, Share network…, Copy name, Forget network (danger) (158–182) | `wifi_menu` 201–220 | **different**: no separator (MenuItem has none) |
| 19 | Menu look: ContextMenu.qml rows 28 px, separators 11 px | settings-menu.slint (44 / 32 px rows) | different (taller rows) |
| 20 | Password row under the network: field 180 × 40/30, secret, "Password" placeholder, focused on show; Enter = Join; Esc closes; Join primary only when 8–63 (or 5/10/13/26) chars; a click when not ready says "Type the password first." / "Wi-Fi passwords are at least 8 characters." (277–324) | network.rs 518–555, slint `join` 691–775 | same |
| 21 | "Other options…" link: hover accent ink + **underline**, pointing hand → enterprise form (327–345) | slint 759–773 | different: no underline on hover |
| 22 | The password row scrolls into view when it opens (revealAsking 120–129) | — | **missing** |
| 23 | Why a join failed: danger caption under *that network's* row (347–356) | network.rs 559–564: all failures appended after the whole list | **different** (position) |
| 24 | "Join other network…" (plus) → hidden-network form (368–376) | network.rs 566–569 | same |
| 25 | Share card in place of the list: back, "Share “name”", QR on white (188), loading, error + Install qrencode, password with show/hide and copy ("Copied" 1.5 s) (WifiShareCard) | slint ShareCard 342–449, network.rs 476–510 | same at a glance; "Copied" clears only on the next rebuild (no 1.5 s timer): check |
| 26 | Join form in place of the list (WifiJoinForm): fields, Segmented security/method/CA, inner-auth Dropdown, file choosers, problem text, Cancel / Join network, saved enterprise settings filled in, back to the list once connected | `join_form` 292–386, 402–429 | same at a glance (QQC.Popup → ActionMenu popup) |
| 27 | Leaving the page (list hidden): password row, form and share card are dropped (WifiList 140–150) | bridge clears `asking`/WifiShare; Rust locals `wifi.form`, `wifi.asking`, `wifi.typed`, `wifi.sharing` survive (lib.rs 531 only drops `more:`) | **different**: coming back shows the old form / password row |
| 28 | Known networks group (not unmanaged): "No saved networks" (secondary); rows ssid + "Saved as …"; Forget (danger) → Quickshell forget + reload after 800 ms, else WifiJoin.forget; footer = forget error or help (81–118) | network.rs 579–597 + bridge forgetWifi | same |
| 29 | Speed test card (SpeedTestCard, settingsLink false): title, sub line, button Test speed / Cancel / Test again; live value + progress; 3 stats (active one accent); jitter/load; verdict; server; error (120–134) | network.rs 600–627, slint SpeedCard 214–336 | same |
| 30 | Speed button: Tab focus + ring (Material 3 px secondary), Space/Return; Glass press scale 0.97 (100 ms); Material ElasticControl + radius 8 when pressed (MaterialSpring) (SpeedTestCard 62–118) | SpeedCard `act` 244–269 | **different**: no keyboard focus/ring, no Glass 0.97 press, no Material press swell |
| 31 | Material progress: WavyProgress (SpeedTestCard 145–151) | SpeedCard 280–291: flat 6 px bar | **different** |
| 32 | Footer: "Runs only when you click… a few hundred MB / about 370 MB…" (122) | network.rs 622–627 | same |
| 33 | Connection: Not connected / Network / Signal (4 bars 4 px, 5+3i tall, filled by quality, "N dBm · words") / Ethernet speed; footer about −67/−75 dBm on Wi-Fi (136–179) | network.rs 630–644, slint `signal` 855–880 | same |
| 34 | More options → Connection: Band, Link rate, Security, Interface, Public IP (+ ISP) (183–213) | network.rs 649–663 | same |
| 35 | Server: Test with Segmented 220 (cloudflare/ookla), disabled while running; detail + footer by state (214–232) | network.rs 665–679 | same |
| 36 | History: "No tests yet" / rows (date · network, backend · server · ping · load, ↓ ↑ values) / Clear history (danger); footer "The last 10 tests." (233–260) | network.rs 681–689 | same |
| 37 | Bluetooth part (PagePart) (264–267) | `p.part(cx, "Bluetooth", bluetooth)` | same (see bluetooth file) |

## Test plan (Material window at 440,205; page column from x≈772)

The arena has no real Wi-Fi: never click the Wi-Fi switch, Scan again,
networks, Forget, or "Start NetworkManager". Look, hover, open menus, Esc.
The speed test uses the host's internet: do not run it.

N1 Look: `setup qml material`, `open network`, `shot net-qml-m`; scroll the page
(`run 'P move 1100 600; P scroll 5'`) and shot again; rust the same; both looks.
PASS: Wi-Fi group (row text by state; if unmanaged, compare the ServiceNotice
— centred icon/title/button — with Rust's row: row 9), Known networks, speed
test card (title "Speed test", sub line, button), Connection group, More
options, then Bluetooth. Texts identical.

N2 Speed button hover: glide onto "Test speed" (read x,y). PASS: hover tint in
both. Press-and-hold without releasing far away: `run 'P down; sleep 0.4'`, shot,
then move the pointer off the button and `P up` (no click). PASS (QML): Glass
shrinks to 0.97 / Material corners go to 8; Rust: report (row 30).

N3 Keyboard: `run 'key -k Tab'` ×N from the page top, shot each. PASS (QML):
a focus ring reaches the speed button (and list rows if any). Rust: report.

N4 More options: open (read y), shot. PASS: Connection rows the arena has
(Interface at least, if connected), Server Segmented (Cloudflare chosen),
History ("No tests yet" or rows). Click Ookla then Cloudflare: Segmented and
footer change and come back.

N5 If (and only if) the arena lists Wi-Fi networks: hover a row (shot: Material
inner corners 12, tint), right-click it (PASS: menu's left edge at the pointer
in QML, with a separator before "Share network…"/"Copy name"; Rust: report
where it opens and whether there's a separator), Esc. Do NOT press-and-hold
a network in Rust (without row 15 the release is a click, which connects or
disconnects). Otherwise skip N5 and note "no networks in the arena".

N6 Leave and return: `open displays`, `open network`. PASS: the list (not a
form / password row) shows in both. (Only reachable with networks present.)

## Fixed in batch 2 (fixer)

- The right-click menus open at the pointer (left edge there) and have the QML's separators (before Share/Copy name; before Copy address).
- The password row, join form and share card are dropped when you go to another page (WifiList.qml when hidden).
- Not fixed yet: the list's snapshot order (re-sorted every 4 s, not under the pointer), press-and-hold, row keys, join-failure placement, password row into view, the "Other options…" underline, the no-NetworkManager notice, press swell, speed test keys/swell, wavy progress, "Copied" timer.

## Fixed in batch 3 (fixer)

- The Wi-Fi list keeps its order while the pointer is over it, a menu is open, a password row or the join form is open; re-sorted at most every 4 s otherwise; a network that's gone goes at once; Scan again sorts afresh (network.rs `frozen`; the list reports hover: `hover:wifi`).
- Press and hold a row 550 ms: its menu at the pointer, and letting go isn't a click (ListItem `hold`).
- Keys: the list is a Tab stop; ↑/↓ move a ring over the rows, Space/Return click, Menu or Shift+F10 its menu.
- Each join failure shows under its own network (the list split there).
- No NetworkManager: a centred notice (icon, title, why, a button: `devices` `notice`).
- Speed test button: a Tab stop with its ring, Space/Return, Glass gives 0.97 when pressed.
- Not done: the password row scrolling into view; the "Other options…" underline; Material press swell; the wavy progress bar; the "Copied" 1.5 s timer.

## Test results

(tester)
