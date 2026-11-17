# shell/modules/settings/RustBridgeDevices.qml ↔ host/system/src/input/devices_bridge.rs (DevicesBridge), net/network_feed.rs, wifi_join.rs, wifi_share.rs, speed_test.rs, brightness.rs, input/displays.rs; host/shell probes.rs (backends, needs); host/sharing lib.rs (airplay, mirroring, phone, quickshare, localsend)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "devices" page group (Displays, Sound, Network: Wi-Fi & Bluetooth, Phone & Sharing).
DevicesBridge follows `devices.live` / `devices.page` (panels.rs 438–453) and builds the feed
from host-system's private bus (`crate::bus`). Most of the keys it reads are never put by
anyone: `feed.wifi`, `feed.backends`, `feed.speed`, `feed.speedTest`, `feed.ddc`,
`feed.airplay`, `feed.mirroring`, `feed.phone`, `feed.sharing`, `wifijoin`, `needs`, `health`,
`filepicker`, `bluetooth` (the real owners publish under other names — "WifiJoin",
"SpeedTest", "Brightness" — or as host state in other crates: `h.state("backends")`,
`"needs"`, `"health"`, `"airplay"`, `"mirroring"`, `"phone"`, `"quickshare"`,
`"localsend"`, `"sharing"`, `"picker.busy"`). So the Network, Sound, Displays (DDC,
mirroring) and Phone pages get empty segments.

## Behaviours

Page on show, scanners, timers (QML 26–89):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | network: SpeedTest.probe, readLink, WifiJoin.loadKnown (27–30) | devices_bridge.rs 126–129 | same |
| 2 | sound: AirPlay.check (31–32) | 141 → airplay.check (sharing alias) | same |
| 3 | displays: Displays.refresh (33–34) | 142 → displays.refresh | same |
| 4 | phone: Phone.checkCam, fm status, sharing-check status (35–38) | 143–147 | same |
| 5 | Mirroring.look while on displays and available, also on availability change (40–61) | 150–154 (only on page change; reads private bus "feed.mirroring" which is never put) | **same (fixed before 17 Nov: sync_look() on page and availability)** [12 Nov: **different**: never looks for TVs (availability always false); no re-check when availability changes] |
| 6 | Leaving network: asking cleared, WifiShare.close (45–49) | 155–161 (reads bus "feed.wifi" → never put, so share isn't closed) | **same (fixed before 17 Nov: reads bus WifiShare.stage, closes it)** [12 Nov: **different**: share card isn't closed on leaving (read `bus::get("WifiShare")["stage"]`)] |
| 7 | readLink every 5 s on network (62–67) | 136–137, 469–474 | same |
| 8 | Wi-Fi scanner on while on network (68–75) | 131 `network.rescan` once | **same (fixed before 17 Nov: network.rescan + network.list every 4 s while on network)** [12 Nov: **different**: one rescan, and NetworkFeed (network_feed.rs 311–330) lists networks only while the Wi-Fi menu is open] |
| 9 | Bluetooth discovering while on network and powered (76–83) | 132–134 `bluetoothctl scan on` child; stopped off-page | same (method differs) |
| 10 | Backends.recheck every 3 s while BT state not ok (84–89) | 139, 475–483 (reads bus "feed.backends" → never put) | **same (fixed before 17 Nov: M::Recheck every 3 s on network while backends.btState != ok)** [12 Nov: **missing**: never rechecks; read `h.state("backends")["btState"]`] |

Feed (QML 207–293):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 11 | `page`, `healthFixing`, `updatesRunning`, `needs{…}` (211) | 205–214 (bus "health"/"needs" never put; "updates" ok) | **same (fixed before 17 Nov: h.state health/needs)** [12 Nov: **different**: healthFixing "" and needs all false always; read `h.state("health")["fixing"]`, `h.state("needs")`] |
| 12 | `wifi` {device, enabled, networks[30 sorted: name, signal, connected, known, secured, open, enterprise, connecting, busy, failed, failedText], known, forgetError, join, saved, share{…, hasPassword, password if revealed}} (213–235) | 217–225 (bus "feed.wifi" never put → stub with no networks; "wifijoin" saved never put) | **same (fixed before 17 Nov: wifi_feed from buses WifiList/WifiJoin/WifiShare)** [12 Nov: **missing**: Network page shows no Wi-Fi networks, known list, join state or share card. Needs a builder from NetworkFeed's nmcli list + bus "WifiJoin" (knownNetworks, forgetError, ssid/stage/error/busy, saved) + bus "WifiShare"] |
| 13 | `backends` {wifiUnmanaged … btFixError} (236–239) | 226 bus "feed.backends" never put | **same (fixed before 17 Nov: h.state("backends"))** [12 Nov: **missing**: use `h.state("backends")` (probes.rs 352)] |
| 14 | `bt` {adapter, enabled, name, devices{addr: trusted, pairing, busy, picture, name}} (240–243) | 227–238 (core BlueZ for adapter/enabled/picture/name; bus "bluetooth" never put) | **same (fixed before 17 Nov: bt_trusted/bt_pairing/bt_busy kept by the bridge; adapter name read)** [12 Nov: **different**: trusted/pairing/busy always false, adapter name "" (core bluez Device has no trusted/pairing)] |
| 15 | `speed` SpeedTestCard texts {running, sub, button, live, progress, stats, download, upload, ping, stage, extra, verdict, server, error} (179–206, 244) | 239 bus "feed.speed" never put | **same (fixed before 17 Nov: speed_card(bus SpeedTest))** [12 Nov: **missing**: build from bus "SpeedTest" (speed_test.rs 307) in the QML shape (network_feed.rs `build_speed` is the menu's shape: `action`, `resp`, `stats[]`, not `button`, `extra`, `download`…)] |
| 16 | `speedTest` {backend, running, ooklaReady, hasOokla, ooklaInstall, link, signalWords, ip, isp, history[label, detail, value]} (245–254) | 240 bus "feed.speedTest" never put | **same (fixed before 17 Nov: speed_test(bus SpeedTest))** [12 Nov: **missing**: build from bus "SpeedTest"] |
| 17 | `airplay` {checked, installed, installing, installError, error, wanted, speakers} (256) | 242 bus "feed.airplay" never put | **same (fixed before 17 Nov: h.state("airplay") + speakers counted from the raop sinks)** [12 Nov: **missing**: `h.state("airplay")` (sharing airplay.rs 128) — has no `speakers` count] |
| 18 | `displays` {loaded, confirming, confirmLeft, monitors+lw/lh/number/scales, saved, lowered} (258–271) | 244–248 bus "displays" (displays.rs 436–462) | same (but `lowered` reads bus "cooldown" savedRates, never put → always []) |
| 19 | `ddc` {displays, values, installed, problem, scanned, scanning} (272) | 249 bus "feed.ddc" never put | **same (fixed before 17 Nov: ddc from bus Brightness)** [12 Nov: **missing**: map bus "Brightness" ddcDisplays[].output, ddcValues, ddcInstalled, ddcProblem, ddcScanned, ddcScanning (brightness.rs 146–149)] |
| 20 | `mirroring` {available, error, p2p, installing, installError, target, state, displays[uuid,name,protocol name]} (273) | 250 bus "feed.mirroring" never put | **same (fixed before 17 Nov: h.state("mirroring"), protocol named)** [12 Nov: **missing**: `h.state("mirroring")` (mirroring.rs 149; protocol not named)] |
| 21 | `phone` {phone{model, android, battery, charging, wireless, serial}, mirroring, hasAdb, hasScrcpy, qrencode, installing, installError, waiting, pairStage, pairQr, pairError, camReady, camModule, kernelPackage, camSetting, camError, webcam} (275–283) | 253 bus "feed.phone" never put | **same (fixed before 17 Nov: h.state("phone"): mirroring from mirrors[], qrencode on PATH)** [12 Nov: **missing**: `h.state("phone")` (phone.rs 312: has `mirrors[]` not `mirroring` bool, no `qrencode`)] |
| 22 | `sharing` {localSend, localSendError, quickShare, quickShareName, quickShareError, btAvailable, btOn, portsError, pairingRisk, passwordless, fixing, fixError, managers, fmError, fmBusy, pickerBusy, user} (284–289) | 254–267 (own fields same; bus "feed.sharing" and "filepicker" never put) | **same (fixed before 17 Nov: sharing from h.state localsend/quickshare/sharing + picker.busy)** [12 Nov: **different**: LocalSend/Quick Share/ports fields missing, pickerBusy always false; use `h.state("localsend")`, `"quickshare"`, `"sharing"`["ports"], `"picker.busy"`] |
| 23 | Feed told on change (293) | 274–284, polled every 250 ms (463–468) | same (polled) |

Acts (QML 316–528):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 24 | `wifiEnable` [on] (320–322) | 293 → network.wifi (nmcli radio) | same |
| 25 | `wifiRescan` (323–328) | 294 → network.rescan | same |
| 26 | `wifiConnect` [name] (329–336) | 295 → network.connect | same |
| 27 | `wifiDisconnect` [name] (337–339) | 296 → network.disconnect | same |
| 28 | `wifiJoin` [name, password] (340–348) | 297–302 → network.join → wifijoin.join | same |
| 29 | `wifiAsking` [name] (349–351) | 303–306 → network.hold | same |
| 30 | `wifiForget` [name] (352–354, 306–315) | 307–310 → network.forget → wifijoin.forget + loadKnown | same |
| 31 | `wifiShare` / `wifiShareClose` / `wifiShareReveal` (355–365) | 311–319 | same |
| 32 | `wifiShareCopy` (366–368) | 320–334 (reads bus "feed.wifi" password → never put) | **same (fixed before 17 Nov: wifiShareCopy reads bus WifiShare.password → wl-copy)** [12 Nov: **missing**: copies nothing; read `bus::get("WifiShare")["password"]`] |
| 33 | `wifiForm` [ssid] (369–374) | 336–341 → wifijoin.dismiss, wifijoin.saved | same (result never fed: row 12) |
| 34 | `wifiJoinForm` [request], `wifiDismiss` (375–380) | 342–343 | same |
| 35 | `installTools` [[pkgs]] (381–383) | 344 → needs.install | same |
| 36 | `startNm`, `openWifiApp` (384–389) | 345–346 → backends.* (probes.rs 650, 668) | same |
| 37 | `speedTest` toggles start/cancel (390–395) | 347–353 (reads bus "feed.speed".running → never put) | **same (fixed before 17 Nov: reads bus SpeedTest.running)** [12 Nov: **different**: always starts, never cancels; read `bus::get("SpeedTest")["running"]`] |
| 38 | `speedClear` (396–398) | 354 | same |
| 39 | `btEnable`, `btRescan`, `btFix`, `btConnect`, `btDisconnect`, `btPair`, `btCancelPair`, `btTrust`, `btForget` (400–433) | 356–368 (bluetoothctl) | same (bluetoothctl instead of BlueZ properties) |
| 40 | `airplayInstall`, `avahiOn` (435–440) | 370–371 | same |
| 41 | `displaysSet`, `displaysSetMany`, `displaysIdentify`, `displaysKeep`, `displaysRevert`, `displaysReset` (442–459) | 373–378 → displays.* | same |
| 42 | `ddcSet` [output, v], `ddcRescan` (460–465) | 379–380 → brightness.* | same |
| 43 | `mirrorInstall`, `mirrorStart` [uuid], `mirrorStop` (466–477) | 381–383 (sharing alias takes the uuid) | same |
| 44 | `fxPreview` [animation, side] (478–480) | 385 → displays.preview | same |
| 45 | `phoneInstall`, `phoneMirror`, `phonePair`, `phoneCancelPair`, `phoneSetUpCam`, `phoneWebcam` (482–499) | 387–392 | same |
| 46 | `phoneCameraChanged`: webcam off, on again after 800 ms (500–505) | 394–399 (reads bus "feed.phone".webcam → never put) | **same (fixed before 17 Nov: phoneCameraChanged reads h.state("phone").webcam)** [12 Nov: **missing**: never restarts; read `h.state("phone")["webcam"]`] |
| 47 | `sendFiles`, `quickShareBluetooth`, `sharingFix`, `fileManager`, `receiveLink`, `shareSelection` (507–524) | 401–410 | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` first). Compare every `shot` with `setup qml material`.

D1 Wi-Fi list: `open network` → wait 5 s → `shot d1`. PASS: the same networks (top 30, connected first), known marks, lock icons as QML.
D2 Rescan: click Rescan (`clicks d2 "x y"`), `shot d2`. PASS: list refreshes, no "no act" in `log`.
D3 Backends: `shot d1` shows the same Wi-Fi/Bluetooth backend notices as QML (none when NetworkManager/BlueZ are fine).
D4 Bluetooth: same page, Bluetooth group → `shot d4`. PASS: adapter name, paired devices with pictures; trusted toggles as QML.
D5 Speed test: click Test speed (`clicks d5 "x y"`), `shot d5a` while running (PASS: live figure, button "Cancel"), click Cancel, `shot d5b` (PASS: stopped).
D6 Share card: on the connected network's menu choose Share (`clicks d6 …`), `shot d6` (PASS: QR shows), click Copy password (PASS: `run 'wl-paste'` prints it), close the card, open Sound, come back: PASS card closed.
D7 Sound: `open sound` → `shot d7`. PASS: AirPlay row as QML.
D8 Displays: `open displays` → `shot d8`. PASS: monitors, scales, DDC brightness rows (if any external screen), mirroring/TV row as QML. Click Identify (`clicks d8b "x y"`): numbers show.
D9 Phone: `open phone` → `shot d9`. PASS: phone row (or "No phone"), adb/scrcpy install state, LocalSend/Quick Share availability, file manager integration list, firewall/pairing notice as QML.
Never: wifiForget, btForget, btPair with real devices, installs (airplay/phone/mirror/tools), displaysSet/Reset, sharingFix, fileManager install/remove.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- Wi-Fi: networks from the bus's new "WifiList" (network_feed `publish_list`, listed every 4 s while the page is up and no password is being typed), join/share from "WifiJoin"/"WifiShare"; backends, needs, healthFixing from host state (3 s Bluetooth recheck); Bluetooth adapter name and trusted marks (busctl), pairing/busy while its own commands run; speed test card and page from "SpeedTest"; DDC from "Brightness"; AirPlay, mirroring, phone, LocalSend, Quick Share, sharing ports and pickerBusy from host state; wifiShareCopy, speedTest cancel, phoneCameraChanged, the TV look and closing the share on leaving the page work.
- Not done: displays.lowered (nothing publishes cool-down's savedRates); per-network "connecting" for connections started elsewhere.

## Test results

(tester)
