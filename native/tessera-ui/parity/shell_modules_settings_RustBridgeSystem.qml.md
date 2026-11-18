# shell/modules/settings/RustBridgeSystem.qml ↔ host/system/src/sysinfo/bridge_system.rs (SystemBridge, inside the System service: system.rs 603, 650, 674), accounts/fingerprint.rs, accounts/face.rs, net/power.rs; host/shell probes.rs (backends, priority), appearance.rs (cooldown); host/sharing (quickshare, sharing.ports); host/desktop screen/captions.rs, screen/dictation.rs

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "system" page group (Privacy & Security: fingerprint, face, firewall; Battery & Power;
Cool-down; Accessibility: live captions). SystemBridge reads host-system's private bus. Only
"fingerprint", "face", "system" and "updates" are put there under those names. "power" is
put as "Power"; "backends", "shellpriority" (host state "priority"), "cooldown",
"quickshare", "sharingports" (host state "sharing".ports), "captions" and "dictation" live
in other crates and are never on this bus. So Battery & Power, Cool-down, Quick Share trust and
captions show defaults.

## Behaviours

Page on show, energy, scans (QML 25–86):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | security: Fingerprint.refresh, Face.refresh, sharing-check status (26–29) | bridge_system.rs 104–107 | same |
| 2 | accessibility: Dictation.refresh (30–31) | 108–109 | same |
| 3 | Power.energyUsers ±1 while on power (35–36) | 111–116 → power.energyUsers (power.rs 536) | same |
| 4 | Re-check the firewall when SharingPorts stops being busy, on security (56–62) | 257–263 `system.sharingBusy` | **same (fixed before 17 Nov: bridge_system.rs watch(): sharing.ports.busy → sharingBusy in the 400 ms tick)** [12 Nov: **missing**: nothing ever calls `system.sharingBusy`; watch `h.state("sharing")["ports"]["busy"]` in the 400 ms tick] |
| 5 | Fingerprint.scanned → takes/misses/retry (2.2 s)/flash (510 ms) (66–86) | 266–281 `system.scanned` | **same (fixed before 17 Nov: bridge_system.rs watch(): fingerprint scanned.seq → scanned)** [12 Nov: **missing**: nothing calls `system.scanned`; fingerprint.rs 133 publishes `scanned {seq, result}` on the bus. Compare `seq` in `tick("sysFeed")` (133–141) and run the same code] |

Feed (QML 88–164):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 6 | `fingerprint` {hasReader … pamError} (89–113) | 157–159, 175–188 (bus "fingerprint") | same |
| 7 | `sheet` {takes, misses, retry, flash} (114) | 189 | same (never advances: row 5) |
| 8 | `face` {hasCamera … camera name} (115–125) | 160, 190–194 (bus "face") | same |
| 9 | `check` sharing-check.py status (126) | 121–131, 195 | same |
| 10 | `quickShare` {trusted, trustBusy, trustError} (127) | 161, 196 (bus "quickshare" never put) | **same (fixed before 17 Nov: h.state("quickshare"))** [12 Nov: **missing**: `h.state("quickshare")` (sharing lib.rs 575)] |
| 11 | `ports` {error, failedWith, busy} (128) | 162, 197 (bus "sharingports" never put) | **same (fixed before 17 Nov: h.state("sharing").ports)** [12 Nov: **missing**: `h.state("sharing")["ports"]` (sharing lib.rs 573)] |
| 12 | `updatesRunning` (129) | 198 | same |
| 13 | `power` {ppd, powerShort, powerReason, hasBattery, why, mode, modeName, hasPerformance, guardError, ownHypridle, userHypridle, energyMinutes, energyApps, keepAwake, keepAwakeStatus, priority{…}} (130–147) | 163–165, 199–207: bus "power" (published as "Power", power.rs 470), "backends" and "shellpriority" never put | **same (fixed before 17 Nov: bus "Power" + h.state backends/priority, keepAwake from bus system)** [12 Nov: **missing**: Battery & Power shows no battery, mode always Balanced, no energy report, no Low Power/Performance availability, priority row off. Read `bus::get("Power")`, `h.state("backends")`, `h.state("priority")`; `hasPerformance` is `Power.hasPerformance`. (Power's `userHypridle` is a path string, QML's a bool: check the page)] |
| 14 | `cooldown` {active, why} (148) | 166, 208 (bus "cooldown" never put) | **same (fixed before 17 Nov: h.state("cooldown"))** [12 Nov: **missing**: `h.state("cooldown")` (appearance.rs 109–110)] |
| 15 | `captions` {hasBinary, hasModel, ready, error, downloading, installing, installError, downloadProgress, downloadError, lastDownload, dictDownloading, languages} (149–162) | 167–168, 209–214 (buses "captions", "dictation" never put) | **same (fixed before 17 Nov: h.state captions + dictation)** [12 Nov: **missing**: Accessibility's live captions always "not installed"; `h.state("dictation")` has installing/downloading/downloadProgress/downloadError (no languages, installError, lastDownload); host-desktop CaptionsSvc publishes no hasBinary/hasModel/ready] |
| 16 | Feed told on change (164) | 219–228, polled 400 ms (241) | same (polled) |

Acts (QML 166–249):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 17 | `fpEnroll` [finger] (known names only), reset sheet (169–176) | 283–291 → fingerprint.enroll | same |
| 18 | `fpCancel` while enrolling (177–180) | 292–296 | same |
| 19 | `fpRemove` [finger] if enrolled (181–184) | 297–301 | same |
| 20 | `fpFix`, `fpInstall` (185–190) | 302–303 | same |
| 21 | `fpSetPam` [polkit\|sudo\|login, on] (191–194) | 304–308 | same |
| 22 | `faceInstall`, `faceSetUp`, `faceAdd`, `faceClear` (196–207) | 310–313 → face.* | same |
| 23 | `quickShareTrust` [on] (209–211) | 315 → quickshare.setTrusted (sharing alias) | same |
| 24 | `portsRetry` (212–214) | 316 → sharingports.retry | same |
| 25 | `powerMode` [power-saver\|balanced\|performance] (216–219) | 318–322 → power.setMode | same |
| 26 | `caffeineOff` (220–222) | 323 → system.setKeepAwake [false, 0] | same |
| 27 | `shellPriority` [on] (223–225) | 324 → shellpriority.set (probes.rs 697) | same |
| 28 | `clearEnergy` (226–228) | 325 | same |
| 29 | `quitApp` [app] (229–232) | 326–330 → apps.quit (wm/mod.rs 252) | same |
| 30 | `coolDownToggle` (233–235) | 331 → cooldown.toggle (appearance.rs 324) | same |
| 31 | `captionsInstall`, `captionsDownload` (237–242) | 333–334 → captions.install / captions.download | **same (fixed before 17 Nov: captions.rs call: install → dictation.install, download → dictation.download tiny)** [12 Nov: **missing**: CaptionsSvc.call (captions.rs 185–200) matches only `args[0]` words off/toggle/place/state; there is no install or download] |
| 32 | `captionsCancel` → Dictation.cancelDownload (243–245) | 335 | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` first). Compare with the QML pass.

S1 Security: `open security` → wait 3 s → `shot s1`. PASS: fingerprint reader state, enrolled fingers, PAM switches, face rows, firewall/root helper notice, Quick Share trust switch as QML.
S2 Enroll sheet (only if a reader is present): click Add a fingerprint (`clicks s2 …`), touch the reader once well and once badly, `shot s2`. PASS: one section filled, a red flash and the reader's hint on the miss. Then Cancel (do not finish).
S3 Battery & Power: `open power` → `shot s3`. PASS: battery, energy mode choices (Low Power/Balanced/Performance availability), the apps' energy list after ~1 min, priority row as QML.
S4 Mode switch: pick Low Power then back to the original (`clicks s4 "x y" "x y"`). PASS: the menu bar battery shows Low Power then not; choice follows.
S5 Cool-down: `open power/cooldown` → `shot s5`. PASS: active/why as QML. Toggle Cool-down on then off (`clicks s5b …`): status follows.
S6 Accessibility: `open accessibility` → `shot s6`. PASS: live captions row (installed, model, languages) as QML.
S7 Ports retry: on Security, if a firewall error shows, click Retry; PASS: the error clears or updates and `check` refreshes.
Never: fpRemove, fpSetPam, fpInstall/fpFix, faceInstall/faceClear, captionsInstall/Download, quitApp on apps you use, shellPriority.

## Fixed in batch 3 (fixer)

- Battery & Power: the feed reads the bus's "Power" (it was "power", empty) and the shell's `backends` / `priority` states.
- Not done: cooldown, quickShare, ports, captions, fingerprint scanning, sharingBusy (see the list above).

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- quickShare, ports, cooldown from host state; fingerprint enrolment advances (the feed tick follows the service's scan number); the firewall recheck on sharing.ports.busy; captions from its new host state (screen/captions.rs) with install/download/refresh.
- Not done: captions noticing a finished install before its next refresh.

## Test results

(tester)
