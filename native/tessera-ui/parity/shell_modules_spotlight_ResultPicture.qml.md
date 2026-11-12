# shell/modules/spotlight/ResultPicture.qml → ui/spotlight.slint (row picture), lib.rs `request_pictures`

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | A screenshot found by its words in place of the icon: cropped (PreserveAspectCrop), read at 96×96, async | 9-22 | `request_pictures` (resize_to_fill 96, on a thread) | same |
| 2 | Rounded: Material round (w/2), Glass r 6; fill colour behind | 12-13 | row picture clip | same |
| 3 | Arrives into the row already shown | — | `update_images` sets that row only | same |

## Test plan (tester)
Needs a screenshot with indexed words in the arena (else mark "not testable here").
1. Type a word from it: the row's leading shows the picture, square-cropped, r 6 (Glass) / round (Material); the rest of the list doesn't blink when it arrives.
