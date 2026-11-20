# shell/modules/settings/pages/phone.qml ↔ surfaces/settings (src/pages/devices/phone.rs `phone`, ui/settings-devices.slint `qr`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Phone & Sharing (page `phone`), with the PagePart "Sharing" (sharing.qml, its
own parity file). Page spacing 24. No popups. One Timer (camera restart
800 ms, in the bridge for Rust).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Phone.checkCam() as the page is made (18) | RustBridgeDevices.qml 35–38 (onPageChanged "phone": checkCam, fm status, checkSystem) | same |
| 2 | Page spacing 24 (14) | phone.rs `.above(24.0)` on each group, More options and the Sharing heading | same |
| 3 | "What it needs" (needsTools: no adb, no scrcpy or no qrencode) + footer about scrcpy: "Android tools and scrcpy", detail installError or "To see and use your phone here…", label red on an error; primary "Install"/"Installing…" (download-simple), busy, disabled while updating → Phone.install(false) (21–38) | 69–78 | same |
| 4 | Group "Phone", footer by phone/no phone (44–45) | 87–93 | same |
| 5 | Phone row: label model; detail "Android N · B% charging · Wireless/USB" (empties dropped); "Show Screen"/"Showing" (device-mobile), primary when not mirroring and tools present, enabled with scrcpy → Phone.mirror(serial) (46–57) | 94–97 (`phone_detail` tested; feed `mirroring`) | same |
| 6 | "Allow this computer on your phone" (no phone, waiting): **label in the accent colour** + detail (58–63) | 98–100 | **same (fixed before 17 Nov: tone 4: accent label)** [12 Nov: **different**: label in the normal label colour (Row has no accent tone)] |
| 7 | "No phone connected": detail "Connect it with a USB cable, or pair it without one below." / "Looking for phones is off." (64–68) | 101–103 | same |
| 8 | Group "Without a cable" + footer; "Network discovery is off" (Needs checked, no avahi) + "Turn On"/"Turning on…" busy → Needs.turnOnAvahi() (72–85) | 106–124 | same |
| 9 | Pair row: label/detail by pairStage (7 cases), red label on error; "Pair"/"Cancel" (while starting/waiting/pairing/connecting), enabled with adb (86–103) | 109–125 (`pair_text` tested) | same |
| 10 | QR code while waiting: 196 tall item, white squircle 176 (radius module) centred, the SVG at 10 px margins, sourceSize 312, **not smoothed**, cache off; accessible "QR code to pair your phone" (104–126) | 111–116 `qr` row: `cx.picture(&qr, 312, 312)`; slint 889–908 (pixelated) | **same (fixed before 17 Nov: the SVG goes through the icon loader (reads SVG), drawn crisp at 312)** [12 Nov: **missing**: phone.py writes the code as an **SVG** (qrencode -t SVG); `cx.picture` → core/picture.rs `decode` uses the `image` crate, which can't read SVG → None, the white square stays empty. (It also resizes with a Triangle filter: blurred modules.)] |
| 11 | Group "Webcam", footer camError or the "Your phone's camera as a webcam…" text (130–134) | 165–169 | same |
| 12 | Not ready: "Phone camera", detail installError (tools fine) / "Needs a virtual camera (v4l2loopback[, built for your K kernel])…" / "Set up a virtual camera…"; "Install"/"Installing…" (no module; busy; disabled while updating) → install(true) or "Set Up"/"Setting up…" (busy) → setUpCam (135–154) | 136–152 | same |
| 13 | Ready: "Use as webcam" switch (enabled with a phone and scrcpy), detail "Connect your phone first" / "On: the phone's front/back camera" / "Off" → setWebcam(!checked) (155–164) | 153–158 | same |
| 14 | "Camera" Segmented 200 Back/Front ↔ phone.camera; when the webcam is on: off, then on again 800 ms later (165–182) | 159–163 + bridge `phoneCameraChanged` (RustBridgeDevices 500–504, camAgain) | same |
| 15 | More › "Options": "Look for phones", "Show the phone in the Dock", "Keep the phone's screen off" switches + details (186–206) | 171–186 | "Show the phone in the Dock" hidden on purpose: the Rust Dock has no phone item yet (phone.dock does nothing) |
| 16 | PagePart "Sharing" (209–212) | 188–193 | same |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open phone`, `shot ph`.
Material window 1040×700 at 440,205; page column from x≈772; read y from
shots. Never pair a real phone: start pairing to get the code, shot it, then
Cancel.

P1 Page: PASS: same groups, rows, texts, footers (the arena has no phone: "No
phone connected"), gaps 24 between groups, both shells.

P2 QR: (needs adb and qrencode in the arena) click "Pair". PASS (QML): label
"Scan this code on your phone", a white 176 px square with a sharp black QR
code. Rust must show the code (today: an empty white square). Zoom the shot:
modules crisp, not blurred. Click "Cancel": label "Pair with a QR code", the
square gone.

P3 Waiting colour: (only with a phone plugged in and unauthorised: skip in the
arena) "Allow this computer on your phone" in the accent colour.

P4 Webcam rows: whatever the arena has (not ready → Phone camera with Install /
Set Up). PASS: same text and button in both shells. Do not click Install.

P5 More options: open; toggle "Show the phone in the Dock": config phone.dock
follows.

## Fixed in batch 3 (fixer)

- The pairing QR shows (an SVG, drawn by the icon loader) and crisp; "Allow this computer on your phone" in the accent.

## Test results

(tester)
