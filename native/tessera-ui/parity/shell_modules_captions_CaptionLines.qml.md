# shell/modules/captions/CaptionLines.qml → surfaces/captions/src/lib.rs `lines_markup`, ui/captions.slint (video lines)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Hint when there's nothing to show: the error, "Live captions need setting up: Settings → Accessibility", or "Captions appear here when something speaks", in ink at 60 % | 9-13, 27 | `lines_markup` | same |
| 2 | The sentence before at 60 %, then what's being said, escaped | 27-29 | `lines_markup`, `escape` | same |
| 3 | At most `lines` lines (3 for video), newest kept: bottom-anchored and clipped | 17-25 | `height: min(text, 3 × line)`, y from the bottom | same |
| 4 | Centred (video), word wrap | 22-23 | `horizontal-alignment: center` | same |
| 5 | Medium weight, line height 1.35 | 25-31 | StyledText | known difference: Slint's StyledText has no weight or line-height (regular weight, the font's line height) |

## Test plan (tester)
Captions on, placement video (`use_config` captions placement "video"), a window focused.
1. `inbox 'qs ipc call captions demo "The sentence before." "What is being said now, long enough to wrap onto more than one line of the subtitles"'` → over the window's bottom: first sentence dimmer (60 %), second white; never more than 3 lines; the newest words always shown.
2. Captions not set up (no model in the arena) → the setup hint at 60 %.
