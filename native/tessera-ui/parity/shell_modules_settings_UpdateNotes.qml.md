# shell/modules/settings/UpdateNotes.qml ↔ surfaces/settings (src/pages/about/update.rs `notes`/`plain`, ui/settings-about.slint `notes`)

Update notes in Software Update ("What's new in Tessera", "What's new"):
CHANGELOG entries newest first, the newest only until "Show all notes (N)";
commit subjects as pointers without a changelog.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Column spacing 16; shown = all ? entries : first (11–19) | update.rs 55–69; slint 204–209 | same |
| 2 | Entry: title 16 DemiBold + date (small, secondary, tnum) on the title's baseline, 8 apart; spacing 6 (23–44) | slint 210–228 (vertical-alignment bottom) | same at a glance (no tnum) |
| 3 | Body as **Markdown** (bold sub-headings, lists, links in accent, `code`), 14 px, wrapped; links clickable (Launch.openLink) with pointing cursor (45–57) | `plain()` 73–102: marks stripped, links as their text, bullets "•  " | **different**: plain text — no bold headings, links not clickable (documented in settings-about.slint) |
| 4 | No entries: fallback commits as a Markdown list (escaped) (61–69) | 60: "•  line" joined | same at a glance |
| 5 | "Show all notes (N)" / "Show only the latest" (entries > 1), accentInk, small, Medium, pointer → toggles (70–80) | slint 242–257 (local `about.update.notesAll` / `resultAll`) | same |

## Test plan

Shown only after a check finds a Tessera update (or right after one). Setups
as in pages_menubar; `open update`; click "Check for updates" (safe: package
lists only); never press Update.

N1 If "What's new in Tessera" shows: `shot notes`. PASS: newest entry only,
title + date; "Show all notes (N)" when N > 1. Note the Markdown difference
(QML bold sub-headings and accent links, Rust plain).

N2 Click "Show all notes (N)" → all entries, link text "Show only the latest";
click again → one entry.

N3 Hover a link in the QML body: pointing cursor; in Rust none (expected
different).

## Test results

(tester)
