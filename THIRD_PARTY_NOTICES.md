# Third-party notices

iRice is licensed under the GNU General Public License, version 3 or (at your option) any later version (`GPL-3.0-or-later`; see [LICENSE](LICENSE)). The parts below come from others and keep their own licences. The licence texts they require are in [LICENSES/](LICENSES) or next to the files.

| Part | Licence | Licence text |
|---|---|---|
| Phosphor Icons | MIT | `shell/assets/fonts/LICENSE-phosphor` |
| iRice Sans Flex (Google Sans Flex, renamed) | OFL-1.1 | `material/assets/fonts/LICENSE-google-sans-flex`, [LICENSES/OFL-1.1.txt](LICENSES/OFL-1.1.txt) |
| iRice Symbols (Material Symbols Rounded, subset and renamed) | Apache-2.0 | `material/assets/fonts/LICENSE-material-symbols`, [LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt) |
| material-color-utilities 0.4.0 | Apache-2.0 | [LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt) |
| Google Nearby and UKEY2 protocol definitions | Apache-2.0 | [LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt) |
| Unicode emoji data and CLDR annotations | Unicode License v3 | [LICENSES/Unicode-3.0.txt](LICENSES/Unicode-3.0.txt) |
| emojibase-data shortcodes | MIT | below |
| Geist and Geist Mono (website film only) | OFL-1.1 | `site/film/engine/fonts/OFL-Geist.txt` |
| Film engine (website only) | MIT | `site/film/engine/LICENSE` |
| Material sound pack | **source unrecorded** | none yet |

## Fonts and icons

**Phosphor Icons.** Copyright (c) 2020-2021 Phosphor Icons. MIT License.
Files: `shell/assets/fonts/Phosphor.ttf`, `Phosphor-Bold.ttf`, `Phosphor-Fill.ttf`, and the same fonts in `site/film/hero/assets/`. The licence and copyright notice are in `LICENSE-phosphor` next to each copy. Upstream: https://github.com/phosphor-icons/homepage

**iRice Sans Flex (Google Sans Flex).** Copyright 2015 The Google Sans Flex Authors (https://github.com/googlefonts/googlesans-flex). SIL Open Font License 1.1.
File: `material/assets/fonts/GoogleSansFlex.ttf`.
- **Changed by iRice:** the family is renamed "iRice Sans Flex" in the font's name table (`tools/gen-material.sh`), so another installed copy can't take its place. The outlines are unchanged.
- **Reserved Font Name:** Google Sans Flex declares none, so OFL section 3 doesn't require the rename. The rename also keeps "Google Sans" from naming a product, as Google's trademark note (in the licence file) asks.
- The font, renamed, is still under the OFL 1.1 and is not relicensed under the GPL. The copyright and licence are kept in the name table and in `LICENSE-google-sans-flex`, which also holds Google's TRADEMARKS.md.
- "Google", "Google Sans" and "Google Sans Flex" are trademarks of Google LLC. They are named here only to credit the source.

**iRice Symbols (Material Symbols Rounded).** Copyright Google LLC. Apache License 2.0.
File: `material/assets/fonts/MaterialSymbolsRounded.ttf`. Upstream: https://github.com/google/material-design-icons
- **Changed by iRice** (Apache-2.0 section 4(b)): subset to the icons listed in `tools/material-symbols-map.txt` (about 400), and renamed "iRice Symbols" in the name table. The glyphs themselves are unchanged.
- This notice and `material/assets/fonts/NOTICE` record the change. The upstream licence is in `LICENSE-material-symbols`. Material Symbols ships no NOTICE file.

**Geist and Geist Mono** (used only by the website's film engine). Copyright (c) 2023 Vercel, in collaboration with basement.studio. SIL Open Font License 1.1.
Files: `site/film/engine/fonts/*.woff2`. Licence: `site/film/engine/fonts/OFL-Geist.txt`.

**Website fonts.** Inter and JetBrains Mono (SIL Open Font License 1.1) are loaded from Google Fonts by the website. They are not in this repository.

## Code

**material-color-utilities 0.4.0.** Copyright Google LLC. Apache License 2.0. Upstream: https://github.com/material-foundation/material-color-utilities
- File: `shell/common/MaterialColor.js`.
- **Changed by iRice:** bundled and minified from the npm package `@material/material-color-utilities@0.4.0` by `tools/gen-material.sh`, with a small entry point, `scheme()`, that iRice wrote. The file's header says so. Upstream's code is otherwise unchanged.

**Film engine** (website only). Copyright (c) 2026 the product-film-kit contributors. MIT License.
- Files: `site/film/engine/`.
- Licence: `site/film/engine/LICENSE`.

## Protocol definitions

**Google Nearby and UKEY2 `.proto` files.** Copyright Google LLC (each file's header gives the year). Apache License 2.0.
- Unchanged copies:
  - `shell/assets/quickshare/proto/nearby/`, from https://github.com/google/nearby
  - `shell/assets/quickshare/proto/ukey2/`, from https://github.com/google/ukey2
- Generated code: `shell/assets/scripts/quickshare/nearby/pb/`, generated from these files by `tools/gen-quickshare-protos.sh`.
- Each `.proto` file keeps its licence header. `shell/assets/quickshare/proto/NOTICE` explains their use.
- iRice uses them to speak the Quick Share protocol with Android phones. It contains no Google SDK.

**RQuickShare** (https://github.com/Martichou/rquickshare, GPL-3.0). One 24-byte constant, `WAKE_DATA` in `shell/assets/scripts/quickshare/nearby/bluez.py`, is the fixed Bluetooth advertisement RQuickShare sends (`core_lib/src/hdl/blea.rs`). It is used for interoperability and credited here. GPL-3.0 is compatible with iRice's licence.

**end4's illogical-impulse** (https://github.com/end-4/dots-hyprland, GPL-3.0). iRice's default keys and gestures follow end4's layout wherever both have the same action (`tools/gen-keymap.py`, `hypr/irice/keybinds.lua`). The layout is followed; no code is copied.

## Data

**Unicode emoji data and CLDR annotations.** Copyright © Unicode, Inc. Unicode License v3.
- Files: `shell/assets/data/emoji.json` (built by `tools/gen-emoji-data.py`) and `shell/common/emoji.js`.
- Sources: Unicode's `emoji-test.txt` (the emoji, their names, order, groups and skin tones) and CLDR 49 annotations (search keywords).
- The licence's copyright and permission notice is in [LICENSES/Unicode-3.0.txt](LICENSES/Unicode-3.0.txt), as the licence requires.

**emojibase-data 18** (https://github.com/milesj/emojibase). Copyright (c) Miles Johnson. MIT License. Its GitHub and CLDR shortcode lists supply the `:shortcode:` names in `shell/assets/data/emoji.json`. The notice:

> Copyright (c) Miles Johnson
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Pictures and sounds

| Files | Source | Licence |
|---|---|---|
| `shell/assets/wallpapers/*.jpg` (Glass's built-in set; the `tahoe-` file names are kept so existing settings still find them) | made by iRice, `tools/gen-wallpapers.sh` | iRice's (GPL-3.0-or-later) |
| `material/assets/wallpapers/*.jpg` | made by iRice, `tools/make-material-wallpapers.py` | iRice's |
| `shell/assets/sounds/` | made by iRice, `tools/make-sounds.py` and `tools/make-directional-sounds.sh` | iRice's |
| `shell/assets/devices/*.svg` | made by iRice, `tools/device-pictures.py` | iRice's |
| `material/assets/sounds/material/*.ogg` (21 files) | **source unrecorded**: no generator and no credit in the repository | **unknown** until the source is found (see PENDING.md) |
| `site/film/hero/assets/*.jpg` (website backgrounds) | photos from Unsplash (per-photo sources not recorded) | Unsplash License |

## Not shipped with iRice

The installer can fetch these if you choose them. They are not part of iRice and iRice's licence doesn't cover them:

- **Apple's macOS wallpapers**, downloaded from a community mirror by `tools/get-wallpapers.sh`. © Apple Inc., all rights reserved.
- **ttf-apple-emoji** (AUR), the Apple emoji style. Apple's font, repackaged by the AUR package; check its terms before you install it.
- Hyprland, Quickshell, the Bibata pointer, icon themes and other packages, each under its own licence. The glass plugin (`hypr/plugin/irice-glass`) is built on your computer against Hyprland's headers (BSD-3-Clause).

## Trademarks

macOS, AirPlay, iCloud and iPhone are trademarks of Apple Inc.; Android, Quick Share, Fast Pair, Chromecast, Google Lens and Material You are trademarks of Google LLC; other names belong to their owners. iRice names them only to say what it works with or resembles. iRice is not affiliated with or endorsed by any of them.
