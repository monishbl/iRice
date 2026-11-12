# shell/modules/settings/PagePart.qml ↔ settings.slint BlockView kind "heading" (+ Flash); model.rs Page::part; lib.rs:219 (page → part)

Another page's settings shown inside this one under a heading (merged pages: Accounts, Mouse & Trackpad, Menu Bar & Dock…).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Heading: top padding 16 / 12, left 0 / 4, label colour, 22 px 400 / Theme.font.title 600, none without a title (18–24) | BlockView heading | same |
| 2 | Heading to the part 12 / 10 (16) | Page::part sets the part's first block `above` 12 / 10 | same |
| 3 | The part is the other page's own content (28–31) | Page::part runs that page's builder | same |
| 4 | Search finds the part's rows here; an older link to that page scrolls to the heading (5–7, 12) | lib.rs:219 maps the old page id to (page, part); Flash on the heading | same |
| 5 | Accessible role Heading (25–26) | `accessible-role: AccessibleRole.text` | same at a glance (Slint has no heading role) |

## Test plan

P1 `open accounts` (Material, Glass, both shells), `shot p1-…`. PASS: part headings' size, weight and spacing match the QML.
P2 `open trackpad` (merged into Mouse). PASS: Mouse opens scrolled to the "Trackpad" heading, which flashes, as the QML.

## Test results

(tester)
