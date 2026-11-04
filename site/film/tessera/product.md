# Product: Tessera

**What it is, in its own words:** a desktop shell for Hyprland on Linux, with two looks: Glass (after macOS) and Material 3 Expressive (after Android), built from one codebase. It covers the menu bar, dock, island, Quick Settings, Search, App Drawer, widgets, lock and login screens.
**Who it's for:** Linux users on Hyprland who want a finished, coherent desktop, not a pile of dotfiles; people who like macOS or Android's polish but want Linux.
**What only it does:**
- two complete looks of one desktop, switched in a second;
- a black island in the menu bar with live activities and alerts that spring open;
- its own Quick Share (phones send files to the laptop, no shared Wi-Fi needed);
- circle to search over anything on screen;
- local Live Text in screenshots;
- AI in a sidebar, with your own models;
- a matching lock screen, login screen and boot animation.

**What's new (4 Nov):**
- the new name (iRice → Tessera) and logo (the Badge);
- chat history in the AI sidebar;
- one Rust process instead of ~30;
- Glass and Material at parity: the A–Z App Drawer, the Quick Settings footer;
- a quiet start;
- usernames typed at login.

## Brand and tokens
- **Logo:** the Badge, Phosphor `circle-wavy` (filled), a scalloped seal. Its edge echoes Material's "cookie" shape and a glass medal, so it suits both looks. Font: shell/assets/fonts/Phosphor-Fill.ttf.
- **Glass:**
  - Type: Inter (the system's; OFL).
  - Accent: the user's choice (default system blue #0A84FF dark / #007AFF light; swatches in shell/common/Theme.qml:26) or the wallpaper's colour.
  - Surfaces: translucent, with blur drawn by the compositor plugin. Radii: continuous corners.
- **Material:**
  - Type: Google Sans Flex, shipped as "iRice Sans Flex" (material/assets/fonts, OFL 1.1: fine for video).
  - Colour: tonal palettes from a seed (Material Color Utilities, shell/common/MaterialColor.js).
  - Motion: MaterialSpring (fast/default/slow), shape morphs (cookie, clover, burst).
- **Icons:** Phosphor (Glass), Material Symbols Rounded (Material).
- **Motion tokens:** Theme.motion (hover 150 ms, press 100 ms, springs).

## Surfaces
| Surface | Where in the code | Notes |
|---|---|---|
| Menu bar + island | shell/modules/menubar | the island has `ipc island demo <kind>` and `post` |
| Dock | shell/modules/dock, material/modules/dock | badges, Downloads stack, magnification (Glass) |
| Quick Settings / Quick Settings | shell/modules/controlcenter | Edit Controls, tiles, sliders, footer |
| Search (Search) | shell/modules/spotlight | apps, actions, quick answers, Ask AI |
| App Drawer | shell/modules/launchpad | categories, A–Z with letter sets |
| Overview / Spaces | shell/modules/overview | Grid and Strip, drag windows between spaces |
| Sidebar (AI, translator) | shell/modules/sidebar | Markdown answers, pictures, chat history |
| Circle to search | screen tools overlay | circle or box → answer in the sidebar |
| Screenshots + Live Text | shell/modules/screentools, quicklook | select text in a shot |
| Quick Share / share sheet | shell/modules/share | phone ↔ laptop |
| Desktop + widgets | shell/modules/desktop | widgets on a snapping grid |
| Lock, login, boot | shell/modules/lock, greeter, Plymouth themes | login intro (`ipc loginIntro play`) |
| Look switch | Settings → Appearance | Glass ↔ Material, with its switch screen |
| Notifications | shell/modules/notifications | banners, Notification Center |

## Hero moments (each changes on screen in 1–3 s)
| # | Moment | What changes | Unique? | Source | Capture notes |
|---|---|---|---|---|---|
| 1 | **The look switch** | the same desktop turns from Glass into Material | yes | footage | the switch screen; record both directions |
| 2 | **The island comes alive** | earbuds connect: the pill springs open, battery rings fill | yes | footage | `ipc island demo headphones` |
| 3 | **Circle to search** | a scribble around a picture → an answer in the sidebar | yes | footage | needs a canned answer (mock model) |
| 4 | **Quick Share** | a phone sends a photo, the laptop's card counts up and opens it | yes | footage + rebuilt phone | fake transfer; the phone drawn |
| 5 | **Live Text** | drag across a screenshot's words, Copy | partly | footage | a generated image with text |
| 6 | **Quick Settings** | the panel opens, tiles bounce (Material), a slider drags | partly | footage | |
| 7 | **Overview** | a tap of Super, the spaces spread out, a window dragged to another space | partly | footage | |
| 8 | **App Drawer A–Z** | a letter picked: its whole set lights up | yes | footage | test apps, as in the sandbox check |
| 9 | **AI sidebar** | a question, a Markdown answer streams in, past chats | partly | footage | canned stream |
| 10 | **Login intro** | the wallpaper comes into focus, the bar and dock settle in | yes | footage | `ipc loginIntro play` |
| 11 | **Search** | a typed sum or app, the answer at once | partly | footage | |
| 12 | **Desktop widgets** | widgets snap into a grid | partly | footage | |
| 13 | **Dynamic colour** | a new wallpaper, and Material's colours follow it | yes (in Material) | footage | two wallpapers |
| 14 | **The Badge, made of tiles** | small tiles (tesserae) gather into the wavy seal, which becomes the menu bar's logo | yes | abstract → footage | the opener and the loop's end |
| 15 | **Lock screen** | the clock, Now Playing; unlock with the fingerprint | partly | footage | fake player |

## Running it safely
- **Sandbox:** `IRICE_ARENA=<n> ./dev.sh up`. It's hidden and silent: a private virtual display and audio, so nothing shows on the user's screen. Capture with `./dev.sh shot` (stills of IRICE-TEST) and `./dev.sh run wf-recorder -g "<region>"` (clips; record a region for full frame rate, expand limited range).
- **Hooks:** `./dev.sh ipc …` (panels open/close, island demo/post, loginIntro play, lock lock/unlock, screen circle), `./dev.sh pointer` (virtual pointer), `./dev.sh run wtype` (keys), sandbox/tools/fakeplayer.py (Now Playing), notify-send inside the sandbox.
- **Clock:** the sandbox clock is real. Pick a calm time: set the shell's clock override if one exists; otherwise film the clock rebuilt or record at a chosen time.

## Never on screen
| What | Where it shows | Replace with |
|---|---|---|
| Wi-Fi network name | Quick Settings, Wi-Fi menu (the sandbox reads the real system bus) | a public placeholder ("Studio") via a fake, or crop it out |
| User name "user", avatar | lock, login, Quick Settings footer | "Maya" with a stock avatar in the arena |
| Bluetooth devices | menus, island | only demo devices |
| Weather location | widgets, lock | a public city |
| Calendar events, to-dos | Today, lock smartspace | seeded demo items |
| Notifications, recent files, chats | Notification Center, sidebar | seeded demo content |
