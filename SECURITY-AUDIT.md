# iRice security and legal audit

Date: 2026-11-03. Branch: `worktree-agent-aca1efca23f50246d`, starting from `60cebd4`.

**Scope.** I read the following and tested them in place:

- the root helper and its polkit policy and rules;
- every network listener: Quick Share (TCP, Bluetooth, Wi-Fi Direct), LocalSend, link sharing, the Cast file server;
- the firewall changes;
- how secrets and private files are handled;
- the installer and update path;
- the QML: shell commands, links, rich text;
- licences, trademarks, and the reverse-engineered protocols.

**How it was tested.**

- Nothing ran as root, and no system setting or firewall was touched.
- No screens were captured, and no other hosts were contacted (loopback only).
- One Qt check ran offscreen against a local server, to confirm that `Text` loads `<img>` from the network. It does.

**Result.** All the clear, low-risk issues are fixed, with tests (commit `8625535`). Anything that changes a feature or a default is listed under *Your decisions* (section 5).

**Tests.**

- `python3 -m pytest -q tests`: **616 passed, 4 skipped**, up from 568 passed and 4 skipped. That includes 46 new tests in `tests/test_security_defaults.py`.
- `node --test tests/*.test.cjs`: 110 passed.
- All 69 changed QML files parse (`qmlformat`).

---

## 1. Findings by severity

Status: **Fixed** means fixed in this branch. **Decide** means it needs your decision; see section 5.

### High

| # | Finding | Where | Status |
|---|---|---|---|
| H1 | **The Wi-Fi helper (root) trusted the phone's DHCP answer.** When joining a phone's Wi-Fi Direct group or hotspot, it ran `ip addr add <ip>/<prefix>`, with the address and mask taken from the phone. A phone could answer mask `0.0.0.0` (`/0`) or `128.0.0.0` (`/1`), or give an address inside your home or VPN network. The kernel's link route (metric 0) would then send this computer's internet, LAN or VPN traffic to the phone. Who can do this: the device you send to, and it can run without a password if "Fast transfers without asking" is on. | `qs-wifi-helper.py` `join` and `_join_as_client` (before the fix: around lines 821 and 867) | **Fixed.** New `lease_network()` accepts only a private IPv4 address, at most a /24, and never one that overlaps a network another interface uses. Otherwise it refuses, and the transfer stays on Bluetooth. |
| H2 | **A received share changes the firewall permanently, without asking.** When a Bluetooth transfer is accepted and a hotspot would help, the Quick Share daemon runs the Settings → Sharing firewall fix by itself (`allow_hotspot`). That fix runs without a password (`49-irice.rules`). It opens 53317–53320/tcp, 53317/udp, 5353/udp and 67/udp **from every source on every interface, permanently**: ufw `allow` with no `from`, firewalld's default zone with `--permanent`, nftables rules first in every input chain. Any program in your session can also trigger it silently (`pkexec /usr/lib/irice/irice-root fix firewall`). | `nearby/service.py:414-418, 887-896`; `irice-root.py:126-198`; `49-irice.rules:9-24` | **Decide** (R1, R2) |
| H3 | **The installer downloads Apple's copyrighted wallpapers by default and makes them the first wallpaper.** The source is an unofficial GitHub mirror, and the prompt defaults to "y". | `install.sh:605-618`; `tools/get-wallpapers.sh:2-23` | **Decide** (L1) |
| H4 | **The repository has no licence.** Without one it is "all rights reserved", so nobody may legally fork or redistribute it. The third-party notices are also incomplete (section 6). | repo root | **Decide** (L2) |

### Medium

| # | Finding | Where | Status |
|---|---|---|---|
| M1 | **Outside text was rendered as markup.** Qt's default `textFormat` (AutoText) treats a string as rich text when it looks like HTML. It then fetches `<img src="http://…">` and honours `<font>`. Strangers control many of these strings, and some render with no click at all: a LocalSend sender's name on the incoming card, calendar invitation titles, web page titles shown in the menu bar, media titles, Wi-Fi network and Bluetooth device names, clipboard entries, island alerts, AUR descriptions. The effect is a network beacon (your IP, and that you saw it) plus UI spoofing, for example hiding a file extension. | 86 `Text` items, listed by the test | **Fixed.** Every one now sets `textFormat: Text.PlainText`. So do the components that show such text (`Marquee`, `HoverTip`, `ListRow`, `Heading`). A static test keeps it that way. |
| M2 | **Notification bodies and links.** The body was StyledText with no sanitiser, so `<img>` from any app or web notification loaded from the network. Body links opened **any scheme** through `xdg-open`: `file:///…/x.desktop`, `smb://`, custom handlers. The same applied to AI chat links, AUR homepages and booru pages. The island also un-escaped `&lt;`/`&gt;`, which turned an app's escaped text back into live tags. | `NotificationCard.qml:126-133`; `MaterialNotificationCard.qml:187-198`; `Notifications.qml:126`; `ChatMessage.qml`; `AppStoreWindow.qml`; `Booru.qml`; `UpdateNotes.qml` | **Fixed.** `Notifications.safeMarkup()` keeps only b/i/u/br and http(s)/mailto links. `Launch.openLink()` opens only http(s) and mailto, and every `onLinkActivated` uses it. The island text is now plain. |
| M3 | **Screen text reached `trans` as options.** OCR'd text (Translate a region) and typed text were passed with no `--`. On-screen text such as `-i=/home/you/.ssh/id_ed25519` would make translate-shell **read that file and send it to the translation service**; `-o=FILE` would write a file. The target language was also quoted without escaping. | `ScreenTools.qml:202`; `Translator.qml:50` (both looks) | **Fixed.** Both values are quoted, and `--` comes first. |
| M4 | **AUR packages install and update with no review.** The flags are `--noconfirm --answerdiff None --answerclean None --answeredit None` (paru: `--skipreview`). AUR scripts are user-uploaded and unchecked. A hijacked AUR package (several are themes from small maintainers) becomes code in the next update, built as you and installed by pacman as root. | `install.sh:440,456`; `tools/setup-tty.sh:51`; `sysupdate.py:851-855` | **Decide** (R4) |
| M5 | **"Fast transfers without asking" (off by default).** It installs a rule letting any process in your session start the root Wi-Fi helper without a password. The helper's interface is narrow and strictly checked (host, join, stop; SSID, passphrase and channel validated; H1 now fixed). While a link exists, though, it accepts **all** traffic on the link interface (firewalld `trusted` zone, nft `iifname accept`, `iptables -j ACCEPT`), not just DHCP and the transfer port. | `irice-root.py:506-546`; `qs-wifi-helper.py:649-667` | **Decide** (R3) |
| M6 | **Code fetched without a pinned signature.** `bash <(curl …/get)` clones GitHub over HTTPS. `--with-settings` downloads a tarball and **runs its `import.sh`** with no checksum. Self-update is `git pull`, and the update then installs the root helper and polkit rules from that checkout (after your password). Anyone who controls the GitHub account or the site's host controls root on the next update. | `site/public/get:47-56`; `setup.sh:4,35`; `irice-root.py:769-795` | **Decide** (R5) |
| M7 | **Google Lens uploads the selected screen area to a public file host** (uguu.se) before opening Lens. A comment in the code says so; the UI label doesn't. | `lens-search.sh:3,14`; label at `shell.qml:167` | **Decide** (R6) |
| M8 | **Other companies' trademarks are used as iRice feature names**, most of all in the site's marketing (section 7). | UI and `site/public/index.html` | **Decide** (L3) |

### Low

| # | Finding | Where | Status |
|---|---|---|---|
| L1 | **LocalSend saved and showed the sender's file names almost raw** (`basename` only). Control characters and bidi overrides got through (`invoice‮gpj.exe` reads as "invoiceexe.jpg"), as did hidden dot-files and names too long to save. | `localsend.py:370` | **Fixed.** `clean_name()` follows the same rules as Quick Share. You're asked about the cleaned names, and those are what land. |
| L2 | **Link sharing had three gaps.** It had no free-space check, so a phone holding the link could upload up to 200 × 4 GB. `os.rename` could replace a file that appeared between the name check and the rename. `private()` accepted `0.0.0.0` (listening there means every network) and documentation ranges. | `link-share.py:71-77, 309-352` | **Fixed.** It keeps 512 MB free, publishes with a hard link (never replacing a file), and accepts only RFC 1918 addresses. |
| L3 | **Cast passed a link to `yt-dlp` with no `--`**, so a link like `--exec=…` became an option. The file server also listened on every interface. | `cast.py:211-224, 353` | **Fixed.** Only http(s) links are taken, they come after `--`, and the server binds to the address the device reaches. |
| L4 | **The Quick Share debug log could fall back to `/tmp`.** With no `$XDG_RUNTIME_DIR`, another user's `/tmp/irice-quickshare.debug` switched the log on, and a symlink in place of the log was followed. | `nearby/debuglog.py:13-30` | **Fixed.** Only a private runtime folder is used, opened with `O_NOFOLLOW`. |
| L5 | **Received files opened as `"file://" + path` without URL-encoding.** A name containing `%`, `#` or `?` opened a different path from the one shown. | `QuickShare.qml:312`; `LinkShare.qml:83`; `IncomingCard.qml:240` (both looks) | **Fixed.** They now use `Files.open(path)`, which passes the path as `$1`. |
| L6 | **Calendar invitation titles and synced reminder titles reached `notify-send` as options**, so a title starting with `-` was read as a flag. | `Calendar.qml:261`; `Todos.qml:197` | **Fixed.** `--` comes first. |
| L7 | **LocalSend's HTTPS server always listens on 53317.** `/info` and `/register` answer with your name and fingerprint even when receiving is off. Anyone on the network can register a peer under any name; this is inherent to LocalSend's trust-on-first-use. | `localsend.py:304-330, 486-494` | **Decide** (R7) |
| L8 | **Quick Share accepts 3 connections at once** (`MAX_INCOMING`). While you're visible, a host on the network can hold all three before the handshake (each up to its timeout). | `service.py:34, 340` | Noted. The limit is the right trade-off. |
| L9 | **`calendars.json` can hold secret calendar URLs.** It is written by `FileView`, then `chmod 600` runs, so each save has a brief 0644 window. | `Calendar.qml:543-552` | **Decide** (R8). Low risk. |
| L10 | **`/tmp` fallbacks when `$XDG_RUNTIME_DIR` is unset** (never the case under logind). | `Clipboard.qml:21`; `ScreenTools.qml:17,109`; `Sharing.qml:70`; `Dictation.qml:96`; `wifi.py:380`; `link-share.py:360` (O_EXCL, random name); `xdg-autostart.py:260` and others | Noted |
| L11 | **Without the installed helper, Wi-Fi Direct falls back to `pkexec python3 -I <script in your home>`.** It needs the password, but the prompt names python. | `nearby/wifi.py:301-304` | **Decide** (R3) |
| L12 | **The D-Bus "shield" can be left behind.** It is a mandatory deny of wpa_supplicant `RemoveInterface`, for everyone, while a group is hosted. It stays if the helper is SIGKILLed, until the helper next starts. wpa_supplicant's log level is raised to `debug` while a link exists; keys are not logged at that level. | `qs-wifi-helper.py:162-176, 768-777, 908` | Noted |
| L13 | **`update --downgrade` accepts package files from `~/.cache/yay` and `~/.cache/paru`**, which you can write to. The path is checked through `realpath`, then given to pacman as root. You could swap the file in between, but you already gave the admin password for this step. | `irice-root.py:831-853` | Noted |
| L14 | **Test fixtures look like real GitHub tokens** (`ghp_AbC123…`), so they may trip secret scanners. | `tests/test_bug_report.py:30,49` | Noted |

### Checked and fine (secure by default)

- **Sharing is off until you turn it on.** Defaults: `sharing.receiving: "off"` (LocalSend) and `sharing.quickShare: "off"`; the Quick Share service starts invisible. *Tested.*
- **Nothing is saved before you say yes.**
  - Quick Share: the transfer frames are read only after ACCEPT, and data sent early is refused. LocalSend: no upload token exists until you accept.
  - The incoming card takes the keyboard only when clicked (`OnDemand`), so a stray Enter can't accept.
  - Received links open only on your click, and only `https?://`.
  - Wi-Fi credentials are only offered as "Copy password".
  - *Tested.*
- **Received files can't go anywhere else or replace anything.**
  - Quick Share: partial files are hidden, 0600 and exclusively created; finished files are 0644 (never executable); the free name is published with a hard link, so nothing is replaced; path parts and control characters are dropped.
  - Quick Share size limits: 8 MB frames, 1 MB byte payloads, 1000 items, free space checked.
  - *Tested.*
- **Quick Share crypto is sound.**
  - UKEY2 uses P-256 and SHA-512 with commitment checks.
  - Messages are AES-256-CBC with HMAC-SHA256 (encrypt-then-MAC), compared with `hmac.compare_digest`.
  - Strict sequence numbers stop replay and reordering.
  - Peers aren't authenticated (the "Everyone" mode, as on Android); the PIN on both screens is the check against a man in the middle.
- **LocalSend.**
  - TLS 1.2 or later; the client pins the certificate fingerprint learned in discovery (`compare_digest`).
  - Its key is 0600 in a 0700 folder.
  - Request bodies are capped at 1 MB; at most 8 sessions and 16 connections.
  - *Tested.*
- **Link sharing.**
  - The link carries a 128-bit secret (compared in constant time) and stops after 20 wrong guesses.
  - Lifetime is 10 minutes or until the card closes.
  - Pages send a strict CSP, `nosniff`, `no-referrer` and `DENY` framing.
  - The QR file is 0600.
  - *Tested.*
- **Root helper.**
  - Inputs are read as the calling user (`seteuid`), never through a FIFO or device.
  - Writes use a temporary file and rename, into root-owned folders.
  - Linked folders are refused.
  - The login look is validated against an exact schema.
  - Fixes are a fixed table, and nothing free-form runs.
  - The update broker runs only `/usr/bin/pacman -S/-U/-R/-D`. It refuses `--config`, `--dbpath`, `--root`, `--hookdir`, `--gpgdir`, `--cachedir` and `--logfile`, their abbreviations, and `-b`/`-r`.
  - Package names can't start with `-`; package files must come from a cache or `https://archive.archlinux.org`.
  - *Tested.*
- **Polkit.** Every action defaults to `auth_admin`. Only `wallpaper`, `login-look` and `fix` skip the password, and only for an active, local `wheel` session with the exact command line. *Tested.*
- **Backups.**
  - No keys, tokens, passwords, accounts, displays or sharing name leave the computer.
  - The file is 0600, and calendar subscriptions aren't included.
  - Restore refuses tar members that aren't iRice's.
  - A new test also fails if a new setting ends in key, token, secret or password and the scrubber wouldn't catch it.
  - *Tested.*
- **Wi-Fi QR.** The password comes from NetworkManager and goes to `qrencode` on stdin, never on a command line. The code file is 0600 in a 0700 runtime folder, and `forget` removes only its own codes. *Tested.*
- **Private stores.** The AI keys fallback, chats, local calendar events, clipboard pins and shared-text drops are written with `umask 077`.
- **Clipboard history.** It is cliphist's own database. Password-manager copies are skipped by cliphist and wl-clipboard (`CLIPBOARD_STATE=sensitive`); this needs cliphist 0.5 or later.
- **Shell commands.** There are 139 `sh -c` sites. 133 are literal scripts that take their data as `"$1"`. The other 6 were checked by hand and are allow-listed in the test.
- **Logs.** `debuglog.py` logs frames by kind and size, and redacts the Wi-Fi passphrase. It logs device names (not file names, keys or PINs), which is consistent with its docstring.
- **Secrets in the repository.** None: no OAuth client secrets, API keys or private keys. Google uses GNOME Online Accounts or your own `client_secret.json`. Fast Pair carries no Google key material.

---

## 2. What was fixed (commit `8625535`)

1. **The Wi-Fi helper's DHCP lease is checked** (`qs-wifi-helper.py`: `lease_network`, `networks_in_use`, `Links._lease_address`). H1.
2. **Outside text is shown as plain text.** 86 `Text` items gained `textFormat: Text.PlainText`, plus the text components `Marquee`, `HoverTip`, `ListRow` and the clipboard `Heading`. M1.
3. **Notification markup is sanitised and links are allow-listed.** New `Notifications.safeMarkup()` and `plainBody()`, and new `Launch.linkAllowed()` and `openLink()`. Every `onLinkActivated`, AUR homepage, booru page and received link now uses `openLink`. M2.
4. **`--` before outside text** for `trans` (screen and typed text) and for `notify-send` (calendar and reminder titles). M3, L6.
5. **LocalSend file names** are cleaned with `clean_name()`. L1.
6. **Link sharing** checks free space (`KEEP_FREE`, uploads in flight counted), publishes with `publish()` so nothing is replaced, accepts only RFC 1918 addresses, and opens partial files with `O_NOFOLLOW`. L2.
7. **Cast** takes only http(s), puts `--` before the link for `yt-dlp`, and binds the file server to one address. L3.
8. **The Quick Share debug log** uses only a private runtime folder, with `O_NOFOLLOW`. L4.
9. **Received files open with `Files.open(path)`.** L5.
10. **`tests/test_security_defaults.py`**, 46 tests:
    - defaults (sharing off, invisible service);
    - asking before saving;
    - cleaned names;
    - received files are never executable;
    - link sharing: private address only, secret required, guessing stops it, the disk can't be filled, no overwrite, QR is 0600;
    - Cast;
    - root helper: polkit (silent set is exactly three; every action `auth_admin`), the fixed fixes, the firewall fix's ports (it fails if they widen), wallpaper (FIFO, non-image, linked folder), the login-look schema, the trust rule's user name, the camera path, pacman, update arguments, downgrade files (links out of the cache), `read_tree` skipping links;
    - Wi-Fi helper lease and request checks;
    - private file modes (debug log, LocalSend key, Wi-Fi QR);
    - backups (no secrets; every secret-looking setting is scrubbed);
    - QML scans: `sh -c` with an allow-list, `trans --`, `onLinkActivated` → `openLink`, no `file://`+path opens, notification bodies sanitised, outside text plain;
    - the `safeMarkup`, `linkAllowed` and `plainBody` logic itself, run in Node.

**What I didn't change.** The other ~1,350 `Text` items still use AutoText. They show the shell's own strings, or data not on the scanner's list (search results, file names in pickers). Defaulting every `Text` to PlainText would close this class completely, but it's a large mechanical diff, so I left it to you (R9).

---

## 3. How the root helper is exposed (for reference)

| Verb | Password? | Who | Input checks |
|---|---|---|---|
| `wallpaper <path>` | no | active, local wheel session | absolute path; read as you; regular file; JPEG, PNG or WebP magic; 64 MB or less; fixed destination |
| `login-look <json>` | no | same | exact keys and values, 2 KB or less |
| `fix <id>` | no | same | one of 8 fixed scripts (firewall: see H2) |
| `wifi-helper` | yes, or no with "Fast transfers without asking" for that user | that user, active and local | JSON verbs host, join, stop; SSID, passphrase, channel and ifname checked; lease checked (new) |
| `fingerprint`, `face`, `phonecam`, `bootmenu`, `quickshare-trust`, `setup`, `update` | yes | admin | fixed values; camera path regex; package names and files checked; pacman broker allow-list |

---

## 4. Network listeners

| Listener | Port and bind | When | Accepts without asking | Limits |
|---|---|---|---|---|
| Quick Share | TCP 53318 on 0.0.0.0, mDNS, BLE advertising and GATT, L2CAP | only while visible (off by default) | the handshake only; files, text and Wi-Fi after Accept | 3 at once, 8 MB frames, 1000 items, disk space checked |
| Quick Share upgrade | TCP 53319 (`wifi.Listener`) | during an accepted transfer | the same session's Nearby re-introduction | per transfer |
| Wi-Fi helper (root) | DHCP 67 on its own group or AP interface; WPA2 with a random 12-character passphrase | during an accepted transfer | DHCP from the phone | interface-bound |
| LocalSend | UDP 53317 multicast; HTTPS 53317 on 0.0.0.0 | always running; receiving off by default | `/info`, `/register` (L7) | 1 MB bodies, 8 sessions, 16 connections |
| Link sharing | HTTP 53320 or 53319 on **one RFC 1918 address** | while the card shows, 10 minutes at most | uploads with the secret link (receive mode: that's the feature) | 4 GB per file, 200 files, 512 MB kept free, 20 wrong guesses |
| Cast file server | HTTP 53320, now on one address | while casting a file | `GET /<128-bit secret>/<name>` | read-only, one file |

**Firewall changes.**

- `fix firewall` opens 53317–53320/tcp, 53317/udp, 5353/udp and 67/udp, from anywhere, permanently (H2).
- The Wi-Fi helper adds a runtime accept-everything for its own link interface, removed afterwards (M5).

---

## 5. Your decisions (recommendations; nothing below was changed)

**R1. The firewall fix.**

- Scope the rules to local networks: `ufw allow from 192.168.0.0/16 … (and 10/8, 172.16/12) to any port …`; firewalld's `home` zone instead of the default; nft `ip saddr` for private ranges.
- Drop 53320 when only LocalSend and Quick Share need opening.
- Consider "runtime only, until the next restart" as the default, with "Keep after restart" as an explicit choice.

**R2. Stop the silent, automatic firewall change.**

- Remove `allow_hotspot()`'s automatic run from Quick Share (`service.py:414-418`). Instead, show "Allow Quick Share through the firewall" as a button in the transfer.
- Remove `org.irice.root.fix` from the passwordless set (`49-irice.rules`). At least split `firewall` out of it: system services can stay silent, firewall changes ask once (`auth_admin_keep`).

**R3. Fast transfers without asking.**

- Keep the feature, but narrow the link's firewall opening to UDP 67 and the transfer port (53319) on that interface, instead of trusting the whole interface.
- Drop the pkexec-python fallback (L11) once the installed helper is everywhere.

**R4. AUR safety.**

- Show PKGBUILD diffs for anything not already reviewed, or pin iRice's AUR set to known-good versions or a vetted list.
- Don't run `yay -Sua` over all of the user's AUR packages inside iRice's update. Update only iRice's own extras, and say plainly that AUR packages are unreviewed.

**R5. Supply chain.**

- Sign release tags (or commits) and have `irice update` verify them (`git verify-tag`) before running `setup`.
- Publish a SHA-256 for the `get` script and for `irice-config.tar.gz`, and check it in `get` before running `import.sh`. Better still, ship `import.sh` in the repo, so only data is downloaded.

**R6. Google Lens.** Change the label to say the selected area is uploaded to a public file host, or ask the first time it's used. Better, use Lens's own upload endpoint if one becomes available.

**R7. LocalSend while receiving is off.** Answer `/register` and `/info` only while receiving is on (discovery for sending can use the multicast path), or don't start the HTTPS server until it's needed.

**R8. `calendars.json`.** Write it with the same `umask 077` writer as `statePath`, so it is never briefly 0644.

**R9. Plain text by default.** Make PlainText the default for every `Text`, through a `Text` wrapper or a one-off sweep, and opt into StyledText only where it's intended (6 places). This closes the AutoText class completely.

**R10. Quick Share bidi-safe names.** Quick Share already strips Unicode category C (controls and bidi format characters). The island now shows names as plain text; consider also showing the real extension separately on the incoming card.

---

## 6. Licence checklist

**Status (3 Nov 2026):** iRice is now GPL-3.0-or-later (`LICENSE`), with `THIRD_PARTY_NOTICES.md` and the licence texts in `LICENSES/`; the MaterialColor.js path and the Material Symbols modification notice are fixed. The Material sound pack's source is still unknown (PENDING.md).

| Item | Licence | In the repo | What it needs | Status |
|---|---|---|---|---|
| **iRice itself** | none | no `LICENSE` at the root; the README states none | choose a licence (GPL-3.0, to match end4 and Hyprland plugins, or MIT/BSD) | **Missing (L2)** |
| Third-party notices file | — | none | one `THIRD-PARTY-NOTICES` covering everything below | **Missing** |
| Phosphor icons (`shell/assets/fonts/Phosphor*.ttf`, `site/film/hero/assets`) | MIT | `LICENSE-phosphor` next to both | keep the notice | OK |
| Google Sans Flex, renamed "iRice Sans Flex" (`material/assets/fonts/GoogleSansFlex.ttf`) | SIL OFL 1.1 | `LICENSE-google-sans-flex` (OFL and TRADEMARKS.md) | **No Reserved Font Name is declared**, so renaming isn't required by OFL §3 but is allowed. The rename fits Google's TRADEMARKS.md ("Google Sans" may not name a product). Keep the OFL text and copyright (both kept in the name table). | OK. Tidy-ups: the named-instance PostScript names still read `GoogleSansFlex-*`, and the file name too; the `gen-material.sh:7` comment still says "unchanged". |
| Material Symbols Rounded, subset and renamed "iRice Symbols" | Apache-2.0 | `LICENSE-material-symbols` | §4(b): modified files must say they were changed. Add a line such as "subset to 404 icons and renamed by iRice". The licence text is there, and Apache-2.0 has no NOTICE file for this font. | Needs a modification notice |
| material-color-utilities 0.4.0 (`shell/common/MaterialColor.js`) | Apache-2.0 | header points to `shell/assets/fonts/LICENSE-material-symbols`, **a path that doesn't exist** (the file is under `material/`); built with `--legal-comments=none` | fix the header path, or put the Apache text under `shell/`; keep upstream's copyright line | **Fix the path** |
| Google Nearby and UKEY2 `.proto` files (`shell/assets/quickshare/proto/`) | Apache-2.0, © Google | `NOTICE` links the licence URL; headers kept; generated `*_pb2.py` committed | include the Apache-2.0 text (a URL is weaker than the text); list in third-party notices | Mostly OK |
| Emoji data (`shell/assets/data/emoji.json`, `shell/common/emoji.js`): Unicode `emoji-test.txt`, CLDR 49, emojibase-data 18 | Unicode License v3; MIT | none | both ask for their notice to be included | **Missing** |
| Film engine (`site/film/engine`) | MIT | `LICENSE` (mentions `references/analyses/*.md`, which isn't here) | fine; trim the stale line | OK |
| Geist and Geist Mono (`site/film/engine/fonts`) | OFL 1.1 | `OFL-Geist.txt` | — | OK |
| Website fonts (Inter, JetBrains Mono) | OFL | loaded from Google Fonts, not shipped | — | OK |
| Hyprland plugin (`hypr/plugin/irice-glass`) | iRice's (none stated) | includes Hyprland's BSD-3 headers only; built locally | add the project licence header | Follows the project licence |
| end4 keybind and gesture layout (`gen-keymap.py`, `keybinds.lua`) | GPL-3.0 upstream | a data layout, not code | credit end4 in notices (already "follows end4" in comments) | Low risk |
| RQuickShare's 24-byte `WAKE_DATA` (`bluez.py:59-64`) | GPL-3.0 upstream | an interoperability constant | credit it in notices | Low risk |
| Wallpapers `tahoe-*.jpg` (Glass), `material-*.jpg` | original (generators in `tools/`) | — | rename away from "Tahoe" (trademark, section 7) | OK (name aside) |
| Sounds `shell/assets/sounds/*` | original (`tools/make-sounds.py`) | — | — | OK |
| **Sounds `material/assets/sounds/material/*.ogg` (17 files)** | **unknown**: no generator, no credit | — | record their source and licence, or regenerate | **Unknown: check** |
| Device pictures (`shell/assets/devices/*.svg`) | original (`tools/device-pictures.py`) | — | — | OK |
| Site background photos (`site/film/hero/assets/*.jpg`) | Unsplash licence ("photos from Unsplash" in the footer) | per-photo source not recorded | Unsplash needs no credit, but keep a source list | OK; record the sources |
| Apple macOS wallpapers (downloaded by the installer) | **Apple, all rights reserved** (unofficial mirror) | not shipped, but downloaded by default and set as the default | stop defaulting to them (H3) | **Risk** |
| `ttf-apple-emoji` (offered in Settings and the installer, AUR) | Apple's proprietary font, repackaged | not shipped | consider not offering it, or warn about its licence | **Risk (medium)** |
| Other AUR and pacman packages (themes, cursors, yay, whisper models, Quickshell, hyprbars) | their own | not shipped, built locally | — | OK |

---

## 7. Trademarks

**Status (3 Nov 2026):** the features below are renamed (mapping in CHANGELOG.md), and `tests/test_trademarks.py` keeps the old names out of user-facing text.

**The rule you set.** The repo documents "no Google in look names" (`docs/material-look.md:17-18`) and "no company logos in the menu icon picker" (`CHANGELOG.md:103`). A general "no company trademarks in UI names" rule isn't written down anywhere; consider adding it to `docs/`.

**Logos.** No Apple, Google or Microsoft logos are used. Only Phosphor's `github-logo` (a link to GitHub) and `linux-logo` appear.

**How to read the table.** Using a mark to say what iRice works with is generally fine ("works with Android's Quick Share", "Sign in to Google", "AirPlay speakers"). Calling iRice's own feature by another company's product name is the risky case.

### iRice features named with another company's mark (rename suggested; I renamed nothing)

| Mark (owner) | Where (examples) | Suggested neutral name |
|---|---|---|
| **Spotlight** (Apple) | `shell.qml:136`; `modules/spotlight/Spotlight.qml:169`; `Keymap.qml:91`; settings footers; site 442, 507, 517-520 | **Search** |
| **Mission Control** (Apple) | `shell.qml:139`; `Keymap.qml:94`; `Search.qml:216`; `settings/pages/desktop.qml:73`; `appearance-windows.qml:68,147,170`; `trackpad.qml:238`; site 545 | **Overview** or "All windows" |
| **App Exposé** (Apple) | `shell.qml:152`; `Keymap.qml:95` | "App windows" |
| **App Library** (Apple) | `shell.qml:147`; `Keymap.qml:96`; `dock/DockMenu.qml:24`; `settings/pages/dock.qml:17,42`; site 545 | "Apps" or "App drawer" |
| **Stage Manager** (Apple) | `controlcenter/ModuleDefs.qml:46`; `appearance-windows.qml:54,57,143`; `windowrules.qml:662`; `SettingsMap.js:40,58`; site 544; README:3 | "Stage" or "Window stage" |
| **StandBy** (Apple) | `settings/pages/lock.qml:42,45,105,151,163-164` | "Bedside clock" or "Charging clock" |
| **Smart Stack** (Apple) | `desktop/WidgetDefs.qml:23`; `WidgetOptions.qml:49,122`; site 552 | "Widget stack" |
| **Quick Look** (Apple) | `desktop/DesktopIcons.qml:166`; `spotlight/ResultRow.qml:91` | "Preview" |
| **App Store** (Apple) | `appstore/AppStoreWindow.qml:20`; `services/Apps.qml:54`; `Search.qml:214,367`; `appstore.py:87` (User-Agent "iRice App Store"); site 441-444 | "Software" or "Get apps" |
| **Control Center** (Apple) | `settings/pages/menubar.qml:76,195`; `SettingsMap.js:39,56`; `controlcenter.qml:14`; README:41 | "Controls" (lower risk; widely generic) |
| **Dynamic Island** (Apple) | `power-cooldown.qml:25` | "Island" (already used elsewhere) |
| **Finder** (Apple) | `appearance-windows.qml:118,122` (tab style); `DesktopIcons.qml:153` (`notify-send -a Finder`) | "Tabs with icons"; app name "Files" |
| **Liquid Glass** (Apple) | `lookswitch/shell.qml:171`; `appearance.qml:25`; `update.qml:139`; `health-check.py:437-441`; `install.sh:460,465,840,845,922`; site 316 | **Glass** ("glass plugin") |
| **Tahoe** (Apple's macOS 26 name) | `wallpaper.qml:124-149` ("Built-in Tahoe set"); `Welcome.qml:115,320`; `controlcenter.qml:17`; `install.sh:429,436,918,933` (banner "iRice  macOS Tahoe for Hyprland"); file names `tahoe-*.jpg`; **site title and `<h1>` "Tahoe, on Hyprland."** (lines 8, 315) | "Built-in Dawn/Day/Dusk/Night"; files `default-*.jpg`; tagline "A glass desktop for Hyprland" |
| **macOS** as option names (Apple) | `appearance-windows.qml:45,75` (cursor and overview style "macOS") | "Classic pointer"; "Strip" |
| **Apple / Android / Windows** as emoji style names | `appearance.qml:228-229`; `EmojiFont.qml:14-16` | name the fonts: "Noto", "Apple Color Emoji", "Fluent" |
| **Material You / Material 3 Expressive** (Google) | `lookswitch/shell.qml:171`; `Welcome.qml:320`; `appearance.qml:25`; `material/.../appearance.qml:17,168` | "Material (experimental)", "Colours from your wallpaper". Lower risk: these describe Google's design system. |
| **Google Lens** as the notification sender | `lens-search.sh:8,13,16` (`notify-send -a "Google Lens"`) | sender "Screen search". The action label "Search an area with Google Lens" is nominative and fine. |
| **Quick Share** as a section or caption | `sharing.qml:133` ("Android Quick Share"); `SharingPage.qml:48`; `Search.qml:187`; `QuickShare.qml:280` | "Android phones (Quick Share)", "Receive from Android phones". Borderline: compatibility wording is fine. |
| **Fast Pair** (Google) as a site heading | site 423 ("Fast Pair, with every battery.") | "Earbuds in one click (works with Google Fast Pair)" |

### Nominative use (fine as written; add an attribution line)

- Quick Share as the protocol you work with (`sharing.qml:89,135`, `ShareSheet.qml`, `file-manager-share.py:197`);
- AirPlay speakers (`sound.qml`);
- Chromecast, Google TV and Miracast (`CastPage.qml`, `displays.qml`);
- Google Fast Pair (`bluetooth.qml:108`);
- the Google, iCloud and Microsoft 365 account pages;
- "Receive from iPhone";
- LocalSend;
- "like macOS" and "like iOS 26" comparisons.

**For the site and README**, add: *"macOS, AirPlay, iCloud and iPhone are trademarks of Apple Inc.; Android, Quick Share, Fast Pair, Chromecast and Material You are trademarks of Google LLC. iRice is not affiliated with or endorsed by them."* The footer (`index.html:642`) already says "Not affiliated with Apple, Google or any device maker".

---

## 8. Reverse-engineered protocols

- **Quick Share / Nearby.**
  - It is a clean-room Python implementation. Message formats come from Google's own Apache-2.0 `.proto` files, and behaviour is cited from Google's open-source `google/nearby`.
  - No Google SDK or service is used, so no Google terms are accepted.
  - Interoperability reimplementation is generally lawful in the US (*Sega v. Accolade*, *Google v. Oracle*'s fair-use reasoning) and expressly permitted in the EU (Software Directive Art. 6).
  - Risks: Google could change the protocol, and the "Quick Share" name (section 7).
  - One 24-byte constant comes from RQuickShare (GPL-3.0); credit it.
- **LocalSend** follows a published open protocol (MIT-licensed reference app); no code is vendored.
- **Google Cast.**
  - CASTV2 is implemented directly with the public Default Media Receiver ID `CC1AD845`, without the Cast SDK, so the SDK's terms aren't accepted.
  - Device TLS is `CERT_NONE` because Cast devices present Google-chained device certificates that are not checked here. A host on the network could impersonate a TV; the impact is limited to what you cast.
- **AirPlay** is send-only, through PipeWire's RAOP module; there is no receiver.
- **Fast Pair** only parses advertisements and pairs through BlueZ. No anti-spoofing keys or model database are embedded.
- **Privacy note.** Google Lens uploads to uguu.se (M7).
