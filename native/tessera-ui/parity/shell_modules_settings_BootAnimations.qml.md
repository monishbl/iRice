# shell/modules/settings/BootAnimations.qml ↔ surfaces/settings (src/pages/look/lock.rs 154–217, ui/settings-look.slint `BootCards` 634–718)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Lock & Login Screen → Starting up → the boot animation cards (both looks).
Choices: "System (<OS>'s)", each BootSplash design (thumbnail
assets/plymouth/thumbnails/<id>.png), "None (text)". Picking one →
BootSplash.set (password, then about a minute's rebuild).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Choices: system (name with SystemInfo.osName), BootSplash.designs, none (15–17) | lock.rs 160–169 (`os_release`, feed `look.boot.designs`) | same |
| 2 | Flow at pad 20 (M) / 14 (G), spacing 12, cards 136 wide; height = cards + 2·pad (11–24) | BootCards: cols = floor((w − 2·pad + 12) / 148), x/y per index, height from rows | same |
| 3 | isChosen = (busy ? pending : chosen) == id; usable = blocked == "" && !busy; opacity 1 when usable or chosen, else 0.45 (32–36) | 161–164, `.on/.off`; slint 640–642 | same |
| 4 | Thumb: 136 × round(136·0.625)=85, radius 16 (M) / control, #0A0A0B, 1 px separator border, clipped (44–52) | slint 656–663 | same |
| 5 | Design: thumbnail at 1 px margins, PreserveAspectCrop, sourceSize 320×200, async (53–61) | 174–177 `cx.picture(..,320,200)` (decoded off the UI thread); slint `image-fit: cover` | same |
| 6 | System: the distribution's logo (IconLookup osLogo ‖ distributor-logo, fallback computer) 36×36 centred (62–71) | 170 `theme_icon([os_logo, "distributor-logo", "computer"])`; slint 672–677 | same |
| 7 | None: "Starting…\n[  OK  ] Network\n[  OK  ] Bluetooth", #94949D, mono 9, lineHeight 1.2, left 10, vertically centred (72–80) | slint 679–685 | same at a glance (line height 1.0 in Slint) |
| 8 | Being set (pending == id): black 50 % over the thumb + **Spinner 24 px, #EDEDEF**, centred (81–91) | slint 687–690 `Busy {}` | **same (fixed before 17 Nov: 24 px #EDEDEF spinner glyph)** [12 Nov: **different**: core Busy is 40 px in Material and its Glass glyph is Theme.tertiary, not #EDEDEF; use a 24 px spinner in #EDEDEF (e.g. Busy with a size/colour input, or the SG.notch glyph rotating as SButton's busy)] |
| 9 | Hover: white at 6 % over the thumb (usable, not chosen), Theme.motion.hover fade (92–98) | slint 691–695 `animate background { SM.hover }` | same |
| 10 | Chosen ring: outside the thumb, −5 (M) / −4 (G), radius thumb − margin, 3 px m3 primary / 2 px accent (100–109) | slint 645–653 | same |
| 11 | **Keyboard: each card is in the Tab order (activeFocusOnTab), Space / Return → BootSplash.set; FocusRing around the thumb (radius + 4); a click also focuses the card** (37–42, 110, 126) | — | **same (fixed before 17 Nov: BootCards: a FocusScope per card (Space/Return → set), SFocusRing round the thumb, a click focuses)** [12 Nov: **missing**: no focus, no keys, no focus ring on the cards] |
| 12 | Accessible radio buttons: name, checked (38–40) | slint 643–646 | same |
| 13 | Name under the thumb (+8): centred, elided, chosen → label colour weight Medium, else secondary Normal; 14 (M) / small (G) (111–119) | slint 699–708 | same |
| 14 | MouseArea: hover, pointing hand only when usable; click → set (even when not usable: BootSplash refuses) (120–129) | slint 709–712 | same |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open lock`, scroll to
"Starting up" (`run 'P move 1100 600; P scroll 0 20'`), `shot boot`. Read the
card positions from the shot. Never finish a real change of the boot animation:
the arena has no root helper; if a password prompt appears, cancel it.

B1 Cards: PASS: System card with the distro logo, one card per design with
its still, None with the three text lines, names under them, the chosen one
ringed; sizes/gaps 136/12, both shells.

B2 Hover: hover a design card that isn't chosen and shot. PASS: the still a
little lighter (6 % white), pointer hand.

B3 Keys: click the System card (cancel any prompt), then `run 'key -k Tab'`.
PASS (QML): focus ring moves to the next card; `key -k space` starts setting
it (spinner over that card). Rust must do the same.

B4 Busy look: while a change is pending (the prompt is up), shot the card.
PASS (QML): dark veil + a 24 px light spinner; compare Rust's size and colour.
Cancel the prompt: the veil goes, the old choice ringed again.

## Fixed in batch 3 (fixer)

- Cards: a Tab stop each with a ring on the picture, Space/Return choose; the spinner 24 px, #EDEDEF.
- (look.bootRefreshed starts over with the page.)

## Test results

(tester)
